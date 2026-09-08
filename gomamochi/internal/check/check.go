// Package check is what an app writes that Gomamochi cannot take, named
// with the line and what to write instead, and the one rewrite the
// interpreted run needs.
//
// Every rule here stands for something the interpreted run gets wrong:
// a shape the interpreter stops on, or, worse, runs and then quietly
// answers differently from the compiled one. The gate catches those
// too, but only after a build; a refusal catches them while the app is
// still being written, and says what to write.
package check

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"sort"
	"strings"
)

// Refusal is one thing the app wrote that cannot be taken.
type Refusal struct {
	File      string
	Line, Col int
	Message   string
}

// Run reads the app and answers what it cannot take. A file Go's own
// parser rejects is answered with that error: gc is the parser of
// record, and nothing here second-guesses it.
func Run(path string) ([]Refusal, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
	if err != nil {
		return nil, err
	}
	c := &checker{fset: fset, path: path}
	c.file(f)
	return c.refusals, nil
}

type checker struct {
	fset     *token.FileSet
	path     string
	refusals []Refusal
}

func (c *checker) refuse(pos token.Pos, msg string) {
	p := c.fset.Position(pos)
	c.refusals = append(c.refusals, Refusal{File: c.path, Line: p.Line, Col: p.Column, Message: msg})
}

func (c *checker) file(f *ast.File) {
	if f.Name.Name != "main" {
		c.refuse(f.Name.Pos(), "an app is a `package main` whose `main` hands the app to `Run`")
	}
	hasMain := false
	for _, d := range f.Decls {
		if fn, ok := d.(*ast.FuncDecl); ok && fn.Recv == nil && fn.Name.Name == "main" {
			hasMain = true
		}
	}
	if !hasMain {
		c.refuse(f.Package, "there is no `main`. Write `func main() { Run(&App{}, Title(\"…\")) }`")
	}
	ast.Inspect(f, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.CallExpr:
			// The app handed to Run straight from a call. The interpreter
			// hands a call's result over without the wrapper that makes an
			// interpreted type satisfy a compiled interface, and Run then
			// cannot take it.
			if id, ok := x.Fun.(*ast.Ident); ok && id.Name == "Run" && len(x.Args) > 0 {
				if _, call := x.Args[0].(*ast.CallExpr); call {
					c.refuse(x.Args[0].Pos(), "the app is handed to `Run` straight from a call, and the interpreted run "+
						"cannot take it that way. Give it a name first: `app := newApp()`, then `Run(app, …)`")
				}
			}
			// Go 1.21's builtins: gc knows them, the interpreter
			// does not, and an app that used them would run one way
			// and not the other.
			if id, ok := x.Fun.(*ast.Ident); ok && id.Obj == nil && (id.Name == "min" || id.Name == "max") {
				c.refuse(id.Pos(), fmt.Sprintf("`%s` is Go 1.21's, and the interpreted run does not know it. "+
					"Write the comparison out (`if a < b { … }`), or a small function of your own", id.Name))
			}
		case *ast.RangeStmt:
			// Go 1.22's range over a number: the interpreter stops
			// on it, rather than answering something else.
			if lit, ok := x.X.(*ast.BasicLit); ok && lit.Kind == token.INT {
				c.refuse(x.X.Pos(), "`range` over a number is Go 1.22's, and the interpreted run stops on it. "+
					"Write `for i := 0; i < n; i++`")
			}
		}
		return true
	})
}

// Rewrite answers the source the interpreted run reads. Every loop
// variable a closure captures gets a copy of its own at the top of the
// body, on the same line as the brace, so that an interpreter with the
// older rule sees what gc has given the closure since Go 1.22 and every
// line number stays where it was. A three-clause loop whose body
// assigns to a captured variable is refused instead: the copy would
// hide the assignment from the loop.
func Rewrite(path string, src []byte) ([]byte, []Refusal) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, src, parser.ParseComments)
	if err != nil {
		return src, nil // gc's error, and the interpreter's, will say it
	}
	c := &checker{fset: fset, path: path}
	type edit struct {
		at   int
		text string
	}
	var edits []edit
	ast.Inspect(f, func(n ast.Node) bool {
		var names []string
		var body *ast.BlockStmt
		threeClause := false
		switch s := n.(type) {
		case *ast.RangeStmt:
			if s.Tok != token.DEFINE {
				return true
			}
			names, body = identNames(s.Key, s.Value), s.Body
		case *ast.ForStmt:
			init, ok := s.Init.(*ast.AssignStmt)
			if !ok || init.Tok != token.DEFINE {
				return true
			}
			names, body, threeClause = identNames(init.Lhs...), s.Body, true
		default:
			return true
		}
		var captured []string
		for _, name := range names {
			if capturedIn(body, name) {
				captured = append(captured, name)
			}
		}
		if len(captured) == 0 {
			return true
		}
		if threeClause {
			if pos, hit := assignedIn(body, captured); hit {
				c.refuse(pos, "the loop's own variable is written inside its body and a closure captures it: "+
					"the two runs would disagree about which iteration the closure sees. "+
					"Copy it first (`j := i`) and let the closure use the copy")
				return true
			}
		}
		var parts []string
		for _, name := range captured {
			parts = append(parts, name+" := "+name+";")
		}
		edits = append(edits, edit{at: fset.Position(body.Lbrace).Offset + 1, text: " " + strings.Join(parts, " ")})
		return true
	})
	if len(c.refusals) > 0 {
		return src, c.refusals
	}
	sort.Slice(edits, func(i, j int) bool { return edits[i].at > edits[j].at })
	out := append([]byte(nil), src...)
	for _, e := range edits {
		out = append(out[:e.at], append([]byte(e.text), out[e.at:]...)...)
	}
	return out, nil
}

func identNames(exprs ...ast.Expr) []string {
	var names []string
	for _, e := range exprs {
		if id, ok := e.(*ast.Ident); ok && id != nil && id.Name != "_" {
			names = append(names, id.Name)
		}
	}
	return names
}

// capturedIn says whether a closure inside the body reads `name`.
func capturedIn(body *ast.BlockStmt, name string) bool {
	found := false
	ast.Inspect(body, func(n ast.Node) bool {
		if found {
			return false
		}
		if lit, ok := n.(*ast.FuncLit); ok {
			ast.Inspect(lit.Body, func(m ast.Node) bool {
				if id, ok := m.(*ast.Ident); ok && id.Name == name {
					found = true
				}
				return !found
			})
			return false
		}
		return true
	})
	return found
}

// assignedIn finds an assignment to one of the names in the body
// itself, outside any closure.
func assignedIn(body *ast.BlockStmt, names []string) (token.Pos, bool) {
	is := func(e ast.Expr) bool {
		id, ok := e.(*ast.Ident)
		if !ok {
			return false
		}
		for _, n := range names {
			if id.Name == n {
				return true
			}
		}
		return false
	}
	var pos token.Pos
	ast.Inspect(body, func(n ast.Node) bool {
		if pos.IsValid() {
			return false
		}
		switch s := n.(type) {
		case *ast.FuncLit:
			return false
		case *ast.AssignStmt:
			for _, e := range s.Lhs {
				if is(e) {
					pos = e.Pos()
				}
			}
		case *ast.IncDecStmt:
			if is(s.X) {
				pos = s.X.Pos()
			}
		}
		return true
	})
	return pos, pos.IsValid()
}

// Render is `file:line:col: …`, the line it is on, and a caret under it.
func Render(r Refusal, lines []string) string {
	head := fmt.Sprintf("%s:%d:%d: Gomamochi cannot take this — %s", r.File, r.Line, r.Col, r.Message)
	if r.Line < 1 || r.Line > len(lines) {
		return head
	}
	col := r.Col - 1
	if col < 0 {
		col = 0
	}
	return head + "\n    " + strings.TrimRight(lines[r.Line-1], "\r\n") + "\n    " + strings.Repeat(" ", col) + "^"
}
