// Package check is what an app writes that Gomamochi cannot take, named
// with the line and what to write instead, and the one rewrite the
// interpreted run needs.
//
// Every rule here stands for something the interpreted run gets wrong:
// a shape the interpreter stops on, or, worse, runs and then quietly
// answers differently from the compiled one — or a shape neither run
// can be trusted with, such as a view that changes what it draws
// between two builds of the same state. The gate catches the second
// kind too, but only after a build; a refusal catches them while the
// app is still being written, and says what to write.
//
// Two layers. The first reads the file alone, with go/parser, and runs
// everywhere. The second type-checks it with go/types, which needs the
// Go toolchain for the packages it imports; it runs when `go` is on the
// path, which `build` and `gate` need anyway, and says nothing when it
// is not.
package check

import (
	"fmt"
	"go/ast"
	"go/importer"
	"go/parser"
	"go/token"
	"go/types"
	"io"
	"os"
	"os/exec"
	"sort"
	"strconv"
	"strings"
)

// Refusal is one thing the app wrote that cannot be taken.
type Refusal struct {
	File      string
	Line, Col int
	Message   string
}

// Module is the import path of the package an app dot-imports.
const Module = "github.com/i2y/yokan/gomamochi"

// Run reads the app and answers what it cannot take, in the order it
// is written. A file Go's own parser rejects is answered with that
// error: gc is the parser of record, and nothing here second-guesses
// it. `root` is the module the typed layer runs in; empty skips it.
func Run(path, root string) ([]Refusal, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
	if err != nil {
		return nil, err
	}
	c := &checker{fset: fset, path: path}
	c.file(f)
	_, loops := loopEdits(fset, path, f)
	c.refusals = append(c.refusals, loops...)
	if root != "" && len(c.refusals) == 0 {
		if err := c.typed(f, root); err != nil {
			return c.refusals, err
		}
	}
	sort.SliceStable(c.refusals, func(i, j int) bool {
		a, b := c.refusals[i], c.refusals[j]
		return a.Line < b.Line || (a.Line == b.Line && a.Col < b.Col)
	})
	return c.refusals, nil
}

type checker struct {
	fset     *token.FileSet
	path     string
	refusals []Refusal
	// The local name of each import, and what it names.
	imports map[string]string
	// Whether the package is dot-imported, so its names are bare.
	bare bool
}

func (c *checker) refuse(pos token.Pos, msg string) {
	p := c.fset.Position(pos)
	c.refusals = append(c.refusals, Refusal{File: c.path, Line: p.Line, Col: p.Column, Message: msg})
}

// --- the first layer: the file alone ------------------------------------------------

// What a view may not call. A view is built again from the same state
// and must draw the same thing each time; anything below answers
// differently from one build to the next, or does work that belongs in
// a handler or a timer.
var (
	impurePackages = map[string]string{
		"os": "the environment or a file", "io": "a stream", "bufio": "a stream",
		"net": "the network", "net/http": "the network",
		"math/rand": "a random number", "math/rand/v2": "a random number", "crypto/rand": "a random number",
	}
	impureTime = map[string]bool{"Now": true, "Since": true, "Until": true, "Sleep": true, "After": true, "Tick": true}
	// The package's own calls that belong in a handler or a timer.
	impureOwn = map[string]string{
		"Task": "starts work", "Every": "declares a timer", "Quit": "closes the window",
		"KeyDown": "reads the keyboard", "KeyPressed": "reads the keyboard", "KeyReleased": "reads the keyboard",
		"AudioPlay": "plays a sound", "AudioStop": "stops a sound",
		"Shortcut": "declares a shortcut", "MenuItem": "declares a menu item",
		"OnKey": "declares a key handler", "OnFileDrop": "declares a drop handler",
	}
)

func (c *checker) file(f *ast.File) {
	if f.Name.Name != "main" {
		c.refuse(f.Name.Pos(), "an app is a `package main` whose `main` hands the app to `Run`")
	}
	c.imports = map[string]string{}
	for _, im := range f.Imports {
		path, _ := strconv.Unquote(im.Path.Value)
		name := path[strings.LastIndex(path, "/")+1:]
		if im.Name != nil {
			name = im.Name.Name
		}
		c.imports[name] = path
		if path == Module && name == "." {
			c.bare = true
		}
		c.importRule(im, path)
	}
	hasMain := false
	for _, d := range f.Decls {
		if fn, ok := d.(*ast.FuncDecl); ok && fn.Recv == nil && fn.Name.Name == "main" {
			hasMain = true
		}
	}
	if !hasMain && f.Name.Name == "main" {
		c.refuse(f.Package, "there is no `main`. Write `func main() { Run(&App{}, Title(\"…\")) }`")
	}

	ast.Inspect(f, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.FuncDecl:
			if c.isView(x.Type, x.Recv) {
				c.view(x.Body, x.Recv)
			}
		case *ast.CallExpr:
			c.call(x)
		case *ast.RangeStmt:
			// Go 1.22's range over a number: the interpreter stops
			// on it, rather than answering something else.
			if lit, ok := x.X.(*ast.BasicLit); ok && lit.Kind == token.INT {
				c.refuse(x.X.Pos(), rangeIntMessage)
			}
		}
		return true
	})
}

const rangeIntMessage = "`range` over a number is Go 1.22's, and the interpreted run stops on it. " +
	"Write `for i := 0; i < n; i++`"

func (c *checker) importRule(im *ast.ImportSpec, path string) {
	switch {
	case path == "C":
		c.refuse(im.Pos(), "cgo: the interpreted run cannot call C, and the compiled run is built without it. "+
			"Reach the engine through the package, and anything else through Go")
	case path == "unsafe":
		c.refuse(im.Pos(), "`unsafe`: the interpreted run does not take it. Write the same thing in plain Go")
	case path == "reflect":
		c.refuse(im.Pos(), "`reflect`: the interpreted run names the app's own types differently from the compiled one, "+
			"so what it answers would differ. Write the check as a type switch or a method")
	case path == "embed":
		c.refuse(im.Pos(), "`//go:embed`: the interpreted run cannot embed a file. Read it with `os.ReadFile` "+
			"in a handler, or name it as an element's source")
	case path == Module:
	case strings.Contains(strings.SplitN(path, "/", 2)[0], "."):
		c.refuse(im.Pos(), fmt.Sprintf("`%s` is a module outside the standard library, and the interpreted run "+
			"cannot read it yet. The standard library and this package are what an app imports", path))
	}
}

// The package's own name, as written: bare under a dot import, or
// `gm.Name` under a named one.
func (c *checker) ownCall(x *ast.CallExpr) (string, bool) {
	switch fn := x.Fun.(type) {
	case *ast.Ident:
		if c.bare && fn.Obj == nil {
			return fn.Name, true
		}
	case *ast.SelectorExpr:
		if id, ok := fn.X.(*ast.Ident); ok && c.imports[id.Name] == Module {
			return fn.Sel.Name, true
		}
	}
	return "", false
}

// A call to a standard package, as `pkg.Name`.
func (c *checker) pkgCall(x *ast.CallExpr) (pkg, name string, ok bool) {
	sel, isSel := x.Fun.(*ast.SelectorExpr)
	if !isSel {
		return "", "", false
	}
	id, isId := sel.X.(*ast.Ident)
	if !isId || id.Obj != nil {
		return "", "", false
	}
	path, imported := c.imports[id.Name]
	if !imported {
		return "", "", false
	}
	return path, sel.Sel.Name, true
}

func (c *checker) call(x *ast.CallExpr) {
	// Go 1.21's builtins: gc knows them, the interpreter does not, and
	// an app that used them would run one way and not the other.
	if id, ok := x.Fun.(*ast.Ident); ok && id.Obj == nil && (id.Name == "min" || id.Name == "max") {
		c.refuse(id.Pos(), fmt.Sprintf("`%s` is Go 1.21's, and the interpreted run does not know it. "+
			"Write the comparison out (`if a < b { … }`), or a small function of your own", id.Name))
	}
	// The app handed to Run straight from a call. The interpreter
	// hands a call's result over without the wrapper that makes an
	// interpreted type satisfy a compiled interface, and Run then
	// cannot take it.
	if name, own := c.ownCall(x); own && name == "Run" && len(x.Args) > 0 {
		if _, call := x.Args[0].(*ast.CallExpr); call {
			c.refuse(x.Args[0].Pos(), "the app is handed to `Run` straight from a call, and the interpreted run "+
				"cannot take it that way. Give it a name first: `app := newApp()`, then `Run(app, …)`")
		}
	}
	// `%T` prints the interpreter's name for an app's type, not Go's.
	if pkg, name, ok := c.pkgCall(x); ok && pkg == "fmt" && strings.HasSuffix(name, "f") {
		for _, a := range x.Args {
			if lit, ok := a.(*ast.BasicLit); ok && lit.Kind == token.STRING && strings.Contains(lit.Value, "%T") {
				c.refuse(lit.Pos(), "`%T` names the app's own types differently in the interpreted run. "+
					"Print the value (`%v`), or give the type a `String` method")
			}
		}
	}
}

// isView says whether a function answers an element or paints a
// canvas: a method named View, anything whose one result is Element,
// and a closure handed a *Painter.
func (c *checker) isView(ft *ast.FuncType, recv *ast.FieldList) bool {
	if ft.Results != nil && len(ft.Results.List) == 1 && c.isOwnType(ft.Results.List[0].Type, "Element") {
		return true
	}
	if ft.Params != nil && len(ft.Params.List) == 1 {
		if star, ok := ft.Params.List[0].Type.(*ast.StarExpr); ok && c.isOwnType(star.X, "Painter") {
			return true
		}
	}
	return false
}

func (c *checker) isOwnType(t ast.Expr, name string) bool {
	switch t := t.(type) {
	case *ast.Ident:
		return c.bare && t.Name == name
	case *ast.SelectorExpr:
		id, ok := t.X.(*ast.Ident)
		return ok && c.imports[id.Name] == Module && t.Sel.Name == name
	}
	return false
}

// view walks a view's body: what it calls, and what it writes. A
// closure inside it that is not itself a view is a handler, and a
// handler is where a write belongs — so the walk stops there.
func (c *checker) view(body *ast.BlockStmt, recv *ast.FieldList) {
	if body == nil {
		return
	}
	self := ""
	if recv != nil && len(recv.List) == 1 && len(recv.List[0].Names) == 1 {
		self = recv.List[0].Names[0].Name
	}
	var walk func(n ast.Node) bool
	walk = func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.FuncLit:
			if c.isView(x.Type, nil) {
				ast.Inspect(x.Body, walk)
			}
			return false
		case *ast.GoStmt:
			c.refuse(x.Pos(), "a view starts a goroutine, and a view is built again from the same state whenever "+
				"anything changes. Start the work from a handler with `Task`")
		case *ast.AssignStmt:
			for _, lhs := range x.Lhs {
				if c.writesSelf(lhs, self) {
					c.refuse(lhs.Pos(), "a view only reads. Move the write into a handler — the closure on a button, "+
						"or a method the app calls from one")
				}
			}
		case *ast.IncDecStmt:
			if c.writesSelf(x.X, self) {
				c.refuse(x.X.Pos(), "a view only reads. Move the write into a handler — the closure on a button, "+
					"or a method the app calls from one")
			}
		case *ast.CallExpr:
			if name, own := c.ownCall(x); own {
				what, impure := impureOwn[name]
				if !impure {
					what, impure = stdlibNames[name]
				}
				if impure {
					c.refuse(x.Pos(), fmt.Sprintf("`%s` %s, and a view is built again from the same state whenever "+
						"anything changes. Call it from a handler or a timer, and keep what it answers on the app", name, what))
				}
			}
			if pkg, name, ok := c.pkgCall(x); ok {
				if what, impure := impurePackages[pkg]; impure {
					c.refuse(x.Pos(), fmt.Sprintf("`%s.%s` reads %s, and a view may only read the app: it is built "+
						"again from the same state whenever anything changes. Read it in a handler and keep the "+
						"answer on the app", pkg[strings.LastIndex(pkg, "/")+1:], name, what))
				} else if pkg == "time" && impureTime[name] {
					c.refuse(x.Pos(), fmt.Sprintf("`time.%s` reads the clock, and a view may only read the app: it is "+
						"built again from the same state whenever anything changes. Read the clock in a timer and "+
						"keep the answer on the app", name))
				}
			}
		}
		return true
	}
	ast.Inspect(body, walk)
}

// writesSelf says whether an expression is a field of the receiver.
func (c *checker) writesSelf(e ast.Expr, self string) bool {
	if self == "" {
		return false
	}
	for {
		switch x := e.(type) {
		case *ast.SelectorExpr:
			if id, ok := x.X.(*ast.Ident); ok {
				return id.Name == self
			}
			e = x.X
		case *ast.IndexExpr:
			e = x.X
		default:
			return false
		}
	}
}

// --- the second layer: go/types ----------------------------------------------------------

// typed type-checks the file the way gc does, with the toolchain's own
// export data for what it imports, and adds what only a type can
// tell: a `range` over a map, a number or a function.
func (c *checker) typed(f *ast.File, root string) error {
	if _, err := exec.LookPath("go"); err != nil {
		return nil
	}
	// The toolchain's export data for everything the file imports, and
	// what those import, asked for in one go: go/importer alone knows
	// GOROOT and GOPATH, not a module.
	var paths []string
	for _, im := range f.Imports {
		p, _ := strconv.Unquote(im.Path.Value)
		if p != "C" && p != "unsafe" {
			paths = append(paths, p)
		}
	}
	exports := map[string]string{}
	if len(paths) > 0 {
		cmd := exec.Command("go", append([]string{"list", "-export", "-deps", "-f", "{{.ImportPath}}={{.Export}}"}, paths...)...)
		cmd.Dir = root
		out, err := cmd.Output()
		if err != nil {
			return nil // the toolchain could not answer; gc will say why at build
		}
		for _, line := range strings.Split(string(out), "\n") {
			if p, e, ok := strings.Cut(line, "="); ok && e != "" {
				exports[p] = e
			}
		}
	}
	lookup := func(path string) (io.ReadCloser, error) {
		e, ok := exports[path]
		if !ok {
			return nil, fmt.Errorf("no export data for %s", path)
		}
		return os.Open(e)
	}
	var goErrors []string
	conf := types.Config{
		Importer: importer.ForCompiler(c.fset, "gc", lookup),
		Error: func(err error) {
			if te, ok := err.(types.Error); ok && te.Soft {
				return
			}
			goErrors = append(goErrors, err.Error())
		},
	}
	info := &types.Info{Types: map[ast.Expr]types.TypeAndValue{}}
	conf.Check("main", c.fset, []*ast.File{f}, info)
	if len(goErrors) > 0 {
		return fmt.Errorf("%s", strings.Join(goErrors, "\n"))
	}
	// A map ranged over in a view: the order changes from run to run,
	// so the two runs would draw different screens. A handler may
	// range over one — to collect its keys and sort them, say — and
	// what it keeps on the app is then the same in both.
	ast.Inspect(f, func(n ast.Node) bool {
		var ft *ast.FuncType
		var body *ast.BlockStmt
		var recv *ast.FieldList
		switch x := n.(type) {
		case *ast.FuncDecl:
			ft, body, recv = x.Type, x.Body, x.Recv
		case *ast.FuncLit:
			ft, body = x.Type, x.Body
		default:
			return true
		}
		if !c.isView(ft, recv) || body == nil {
			return true
		}
		var walk func(m ast.Node) bool
		walk = func(m ast.Node) bool {
			switch y := m.(type) {
			case *ast.FuncLit:
				if c.isView(y.Type, nil) {
					ast.Inspect(y.Body, walk)
				}
				return false
			case *ast.RangeStmt:
				if tt := info.TypeOf(y.X); tt != nil {
					if _, isMap := tt.Underlying().(*types.Map); isMap {
						c.refuse(y.X.Pos(), "`range` over a map walks it in a different order every run, and a view "+
							"must draw the same screen from the same state. Keep a sorted list of the keys on the "+
							"app, made in a handler, and range over that")
					}
				}
			}
			return true
		}
		ast.Inspect(body, walk)
		return false
	})
	ast.Inspect(f, func(n ast.Node) bool {
		r, ok := n.(*ast.RangeStmt)
		if !ok {
			return true
		}
		t := info.TypeOf(r.X)
		if t == nil {
			return true
		}
		switch u := t.Underlying().(type) {
		case *types.Basic:
			if u.Info()&types.IsInteger != 0 {
				if _, lit := r.X.(*ast.BasicLit); !lit {
					c.refuse(r.X.Pos(), rangeIntMessage)
				}
			}
		case *types.Signature:
			c.refuse(r.X.Pos(), "`range` over a function is Go 1.23's, and the interpreted run stops on it. "+
				"Call the function and range over what it answers")
		}
		return true
	})
	return nil
}

// --- the rewrite --------------------------------------------------------------------------------

type edit struct {
	at   int
	text string
}

// loopEdits finds every loop variable a closure captures. Each gets a
// copy of its own at the top of the body, on the same line as the
// brace, so that an interpreter with the older rule sees what gc has
// given the closure since Go 1.22 and every line number stays where it
// was. A three-clause loop whose body assigns to a captured variable
// is refused instead: the copy would hide the assignment from the loop.
func loopEdits(fset *token.FileSet, path string, f *ast.File) ([]edit, []Refusal) {
	c := &checker{fset: fset, path: path}
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
	return edits, c.refusals
}

// Rewrite answers the source the interpreted run reads, with the loop
// edits applied. A file that does not parse is answered as it is: gc's
// error, and the interpreter's, will say it.
func Rewrite(path string, src []byte) ([]byte, []Refusal) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, src, parser.ParseComments)
	if err != nil {
		return src, nil
	}
	edits, refusals := loopEdits(fset, path, f)
	if len(refusals) > 0 {
		return src, refusals
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
	src := strings.TrimRight(lines[r.Line-1], "\r\n")
	// The caret sits under the column; a tab in the line stays a tab in
	// the padding, so the two line up however wide a tab is drawn.
	col := r.Col - 1
	if col < 0 {
		col = 0
	}
	if col > len(src) {
		col = len(src)
	}
	pad := []byte(src[:col])
	for i, b := range pad {
		if b != '\t' {
			pad[i] = ' '
		}
	}
	return head + "\n    " + src + "\n    " + string(pad) + "^"
}
