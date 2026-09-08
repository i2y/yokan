// Read the engine's element table (crates/pixie-capi/elements.toml) and
// write the two files that have to agree about it: the Go an app calls
// (elements.go: one type per element, one method per keyword, the
// numbers both sides count with) and the interpreter's view of the
// package (internal/symbols/symbols.go). Nothing in either is written
// by hand, so an element cannot mean one thing in Go and another in the
// engine.
//
//	go run ./tools/gen            write the files
//	go run ./tools/gen --check    fail if what is on disk is not what this
//	                              would write (the sweep runs this)
//
// The TOML this reads is the TOML the table is written in: array-of-
// tables headers, scalars, and arrays of one-line inline tables. The
// Rust side parses the same file with a real parser, and its test fails
// if a key here never reached an arm — so a mistake in the reader
// below cannot pass quietly.
package main

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/printer"
	"go/token"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// --- the TOML this file speaks ------------------------------------------------

type table map[string]any

func tomlValue(s string) any {
	switch {
	case strings.HasPrefix(s, `"`):
		v, err := strconv.Unquote(s)
		if err != nil {
			panic(err)
		}
		return v
	case s == "true":
		return true
	case s == "false":
		return false
	case s == "[]":
		return []any{}
	}
	if i, err := strconv.ParseInt(s, 10, 64); err == nil {
		return i
	}
	f, err := strconv.ParseFloat(s, 64)
	if err != nil {
		panic("not a TOML value: " + s)
	}
	return f
}

var inlineRe = regexp.MustCompile(`(\w+)\s*=\s*("(?:[^"\\]|\\.)*"|\[\]|[^,}\s]+)`)

func tomlInline(s string) table {
	t := table{}
	for _, m := range inlineRe.FindAllStringSubmatch(s, -1) {
		t[m[1]] = tomlValue(m[2])
	}
	return t
}

var headerRe = regexp.MustCompile(`^\[\[(\w+)\]\]$`)

func parseTOML(text string) map[string][]table {
	doc := map[string][]table{}
	var cur table
	var key string
	var arr []any
	inArr := false
	for _, raw := range strings.Split(text, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if inArr {
			if strings.HasPrefix(line, "]") {
				cur[key] = arr
				inArr = false
			} else if strings.HasPrefix(line, "{") {
				arr = append(arr, tomlInline(line))
			}
			continue
		}
		if m := headerRe.FindStringSubmatch(line); m != nil {
			cur = table{}
			doc[m[1]] = append(doc[m[1]], cur)
			continue
		}
		k, v, _ := strings.Cut(line, "=")
		k, v = strings.TrimSpace(k), strings.TrimSpace(v)
		if v == "[" {
			key, arr, inArr = k, []any{}, true
			continue
		}
		cur[k] = tomlValue(v)
	}
	return doc
}

func (t table) str(k string) string {
	if v, ok := t[k].(string); ok {
		return v
	}
	return ""
}

func (t table) is(k string) bool { v, _ := t[k].(bool); return v }

func (t table) has(k string) bool { _, ok := t[k]; return ok }

func (t table) props() []table {
	var out []table
	if vs, ok := t["props"].([]any); ok {
		for _, v := range vs {
			out = append(out, v.(table))
		}
	}
	return out
}

// --- the table, and the numbers -----------------------------------------------

var (
	riders   []table
	elements []table
	ops      []table
	keyID    = map[string]int{}
	keyOrder []string
	kindID   = map[string]int{}
)

// A keyword is numbered once, by its name, however many elements take
// it: `width` is one key whether a button owns it or a box rides on it.
func key(name string) int {
	if id, ok := keyID[name]; ok {
		return id
	}
	keyID[name] = len(keyID) + 1
	keyOrder = append(keyOrder, name)
	return keyID[name]
}

func load(path string) {
	text, err := os.ReadFile(path)
	if err != nil {
		fail(err.Error())
	}
	doc := parseTOML(string(text))
	riders, elements, ops = doc["rider"], doc["element"], doc["op"]
	for _, r := range riders {
		key(r.str("name"))
	}
	for i, e := range elements {
		kindID[e.str("name")] = i + 1
		for _, p := range e.props() {
			key(p.str("name"))
		}
	}
}

// --- spelling -----------------------------------------------------------------------

// camel is the Go spelling of a table name: `text_field` is TextField,
// `a11y_label` is A11yLabel, `h_scroll_view` is HScrollView.
func camel(name string) string {
	var b strings.Builder
	for _, part := range strings.Split(name, "_") {
		if part == "" {
			continue
		}
		b.WriteString(strings.ToUpper(part[:1]) + part[1:])
	}
	return b.String()
}

// lower is the same with a small first letter, for a field.
func lower(name string) string {
	c := camel(name)
	return strings.ToLower(c[:1]) + c[1:]
}

func typeName(el table) string { return camel(el.str("name")) + "El" }

// goType is what a prop's value is in Go.
func goType(p table) string {
	if p.has("handler") {
		switch p.str("handler") {
		case "none":
			return "func()"
		case "text":
			return "func(string)"
		case "bool":
			return "func(bool)"
		case "int":
			return "func(int)"
		case "float":
			return "func(float64)"
		}
	}
	switch p.str("type") {
	case "str":
		return "string"
	case "num":
		return "float64"
	case "int":
		return "int"
	case "bool":
		return "bool"
	case "strs":
		return "[]string"
	case "nums":
		return "[]float64"
	case "nums2":
		return "[][]float64"
	case "rows":
		return "func(int) Element"
	}
	fail("no Go type for " + p.str("name"))
	return ""
}

// goDefault is the literal a keyword's field starts at.
func goDefault(p table) string {
	d, ok := p["default"]
	if !ok {
		return ""
	}
	switch v := d.(type) {
	case string:
		return strconv.Quote(v)
	case bool:
		return strconv.FormatBool(v)
	case int64:
		return strconv.FormatInt(v, 10)
	case float64:
		return strconv.FormatFloat(v, 'f', -1, 64)
	}
	return ""
}

// zero says whether a keyword's default is Go's zero value for its
// field, so the constructor needs no line for it.
func zero(p table) bool {
	switch p.str("type") {
	case "strs", "nums", "nums2":
		return true
	}
	d := goDefault(p)
	return d == "" || d == `""` || d == "false" || d == "0"
}

func doc(text string) string {
	var out strings.Builder
	line := "//"
	for _, w := range strings.Fields(text) {
		if len(line)+len(w)+1 > 76 {
			out.WriteString(line + "\n")
			line = "//"
		}
		line += " " + w
	}
	out.WriteString(line + "\n")
	return out.String()
}

// --- elements.go ----------------------------------------------------------------------

const banner = "// Code generated by tools/gen from crates/pixie-capi/elements.toml. DO NOT EDIT.\n" +
	"// Edit the table and run `go run ./tools/gen`.\n"

const stdBanner = "// Code generated by tools/gen from crates/yokan-stdlib/stdlib.toml. DO NOT EDIT.\n" +
	"// Edit the manifest and run `go run ./tools/gen`.\n"

func doorCall(p table, key string, v string) string {
	switch p.str("type") {
	case "str":
		return fmt.Sprintf("door.Str(el, %s, %s)", key, v)
	case "num":
		return fmt.Sprintf("door.Num(el, %s, %s)", key, v)
	case "int":
		return fmt.Sprintf("door.Int(el, %s, int64(%s))", key, v)
	case "bool":
		return fmt.Sprintf("door.Bool(el, %s, %s)", key, v)
	}
	fail("no door call for " + p.str("name"))
	return ""
}

// The lines that write one prop, in emit.
func propLines(p table) string {
	name := p.str("name")
	k := "k" + camel(name)
	f := "e." + lower(name)
	if p.has("handler") {
		var call string
		switch p.str("handler") {
		case "none":
			call = "f"
		case "text":
			call = "func() { f(door.EventText()) }"
		case "bool":
			call = "func() { f(door.EventInt() != 0) }"
		case "int":
			call = "func() { f(int(door.EventInt())) }"
		case "float":
			call = "func() { f(door.EventNum()) }"
		}
		return fmt.Sprintf("\tif f := %s; f != nil {\n\t\tdoor.Handler(el, %s, %s)\n\t}\n", f, k, call)
	}
	switch p.str("type") {
	case "strs":
		return fmt.Sprintf("\tfor _, v := range %s {\n\t\tdoor.PushStr(el, %s, v)\n\t}\n", f, k)
	case "nums":
		return fmt.Sprintf("\tfor _, v := range %s {\n\t\tdoor.PushNum(el, %s, v)\n\t}\n", f, k)
	case "nums2":
		return fmt.Sprintf("\tfor _, inner := range %s {\n\t\tdoor.ListBreak(el, %s)\n\t\tfor _, v := range inner {\n\t\t\tdoor.PushNum(el, %s, v)\n\t\t}\n\t}\n", f, k, k)
	case "rows":
		return fmt.Sprintf("\tif row := %s; row != nil {\n\t\tdoor.RowBuilder(el, %s, func(i int64) int64 {\n\t\t\tr := row(int(i))\n\t\t\tif r == nil {\n\t\t\t\treturn 0\n\t\t\t}\n\t\t\treturn r.emit()\n\t\t})\n\t}\n", f, k)
	}
	call := doorCall(p, k, f)
	// A positional argument is always written, so it always crosses; a
	// keyword only crosses when it differs from what the engine would
	// have used anyway.
	if p.is("pos") {
		return "\t" + call + "\n"
	}
	var cond string
	switch p.str("type") {
	case "bool":
		if goDefault(p) == "true" {
			cond = "!" + f
		} else {
			cond = f
		}
	default:
		cond = f + " != " + goDefault(p)
	}
	return fmt.Sprintf("\tif %s {\n\t\t%s\n\t}\n", cond, call)
}

func genElements() []byte {
	var b strings.Builder
	b.WriteString(banner)
	b.WriteString("\npackage gomamochi\n\n")
	b.WriteString("import \"github.com/i2y/yokan/gomamochi/internal/door\"\n\n")

	b.WriteString("// The numbers the two sides of the engine's C face count with.\n")
	b.WriteString("const (\n")
	for _, e := range elements {
		fmt.Fprintf(&b, "\tkind%s = %d\n", camel(e.str("name")), kindID[e.str("name")])
	}
	b.WriteString(")\n\n")
	b.WriteString("// The keyword arguments, numbered once across every element.\n")
	b.WriteString("const (\n")
	for _, name := range keyOrder {
		fmt.Fprintf(&b, "\tk%s = %d\n", camel(name), keyID[name])
	}
	b.WriteString(")\n\n")

	// The riders.
	b.WriteString("// The keyword arguments every element takes, under one name and one\n")
	b.WriteString("// meaning. A rider that was written is written to the engine whatever\n")
	b.WriteString("// its value — a width of 0 still makes a box, and one nobody wrote does\n")
	b.WriteString("// not — so each remembers whether it was.\n")
	b.WriteString("type riders struct {\n\tset uint32\n")
	for _, r := range riders {
		fmt.Fprintf(&b, "\t%s %s\n", lower(r.str("name")), goType(r))
	}
	b.WriteString("}\n\n")
	b.WriteString("const (\n")
	for i, r := range riders {
		if i == 0 {
			fmt.Fprintf(&b, "\tr%s uint32 = 1 << iota\n", camel(r.str("name")))
		} else {
			fmt.Fprintf(&b, "\tr%s\n", camel(r.str("name")))
		}
	}
	b.WriteString(")\n\n")
	b.WriteString("func (r *riders) write(el int64) {\n")
	for _, r := range riders {
		name := r.str("name")
		fmt.Fprintf(&b, "\tif r.set&r%s != 0 {\n\t\t%s\n\t}\n", camel(name), doorCall(r, "k"+camel(name), "r."+lower(name)))
	}
	b.WriteString("}\n\n")
	b.WriteString("// box carries the riders and the methods that set them, promoted into\n")
	b.WriteString("// every element; `self` is the element, so a chain keeps its type. An\n")
	b.WriteString("// element whose own keyword has the same name (a button's width) has a\n")
	b.WriteString("// method of its own that wins over the rider's.\n")
	b.WriteString("type box[T any] struct {\n\tself T\n\trd   riders\n}\n\n")
	for _, r := range riders {
		name := r.str("name")
		v := "v"
		if r.str("type") == "int" {
			v = "v"
		}
		fmt.Fprintf(&b, "func (b *box[T]) %s(v %s) T { b.rd.%s, b.rd.set = %s, b.rd.set|r%s; return b.self }\n",
			camel(name), goType(r), lower(name), v, camel(name))
	}
	b.WriteString("\n")

	// The elements.
	for _, e := range elements {
		name := e.str("name")
		tn := typeName(e)
		cn := camel(name)
		props := e.props()
		b.WriteString("// " + tn + ": " + strings.TrimPrefix(doc(e.str("doc")), "// "))
		fmt.Fprintf(&b, "type %s struct {\n\tbox[*%s]\n", tn, tn)
		for _, p := range props {
			fmt.Fprintf(&b, "\t%s %s\n", lower(p.str("name")), goType(p))
		}
		if e.is("children") {
			b.WriteString("\tkids []Element\n")
		}
		if e.is("paints") {
			b.WriteString("\tpaint func(*Painter)\n")
		}
		b.WriteString("}\n\n")

		// The constructor: the positional props in order, then the
		// children; a keyword with a default that is not Go's zero
		// starts there.
		var params, inits []string
		for _, p := range props {
			if p.is("pos") {
				params = append(params, lower(p.str("name"))+" "+goType(p))
				inits = append(inits, lower(p.str("name"))+": "+lower(p.str("name")))
			} else if !zero(p) {
				inits = append(inits, lower(p.str("name"))+": "+goDefault(p))
			}
		}
		if e.is("children") {
			params = append(params, "kids ...Element")
			inits = append(inits, "kids: kids")
		}
		fmt.Fprintf(&b, "// %s is a new %s.\n", cn, name)
		fmt.Fprintf(&b, "func %s(%s) *%s {\n\te := &%s{%s}\n\te.self = e\n\treturn e\n}\n\n",
			cn, strings.Join(params, ", "), tn, tn, strings.Join(inits, ", "))

		// One method per keyword.
		for _, p := range props {
			if p.is("pos") {
				continue
			}
			pn := p.str("name")
			mt := goType(p)
			switch p.str("type") {
			case "strs":
				fmt.Fprintf(&b, "func (e *%s) %s(v ...string) *%s { e.%s = v; return e }\n", tn, camel(pn), tn, lower(pn))
			case "nums":
				fmt.Fprintf(&b, "func (e *%s) %s(v ...float64) *%s { e.%s = v; return e }\n", tn, camel(pn), tn, lower(pn))
			default:
				fmt.Fprintf(&b, "func (e *%s) %s(v %s) *%s { e.%s = v; return e }\n", tn, camel(pn), mt, tn, lower(pn))
			}
		}
		if e.is("paints") {
			fmt.Fprintf(&b, "\n// Paint is what the canvas draws: the commands written on the painter\n// it is handed, in order.\nfunc (e *%s) Paint(f func(*Painter)) *%s { e.paint = f; return e }\n", tn, tn)
		}
		if e.is("owns_label") {
			fmt.Fprintf(&b, "\n// A11yLabel is refused here: this element's own `label` is already the\n// name a screen reader reads.\nfunc (e *%s) A11yLabel(string) *%s {\n\tpanic(\"this element's own `label` is already the name a screen reader reads; there is no second one to give\")\n}\n", tn, tn)
		}
		b.WriteString("\n")

		// emit.
		fmt.Fprintf(&b, "func (e *%s) emit() int64 {\n", tn)
		if e.is("children") {
			b.WriteString("\t// Children first, as an argument list is evaluated in every\n\t// sibling language; the dumps then read the same.\n")
			b.WriteString("\tids := make([]int64, 0, len(e.kids))\n\tfor _, k := range e.kids {\n\t\tif k != nil {\n\t\t\tids = append(ids, k.emit())\n\t\t}\n\t}\n")
		}
		fmt.Fprintf(&b, "\tel := door.El(kind%s)\n", cn)
		for _, p := range props {
			b.WriteString(propLines(p))
		}
		if e.is("children") {
			b.WriteString("\tdoor.Children(el, ids)\n")
		}
		if e.is("paints") {
			b.WriteString("\tif e.paint != nil {\n\t\te.paint(&Painter{el: el})\n\t}\n")
		}
		b.WriteString("\te.rd.write(el)\n\treturn door.End(el)\n}\n\n")
	}

	// The drawing commands.
	b.WriteString("// Painter is what a canvas's Paint closure draws with. A command is not\n")
	b.WriteString("// an element: it takes none of the keywords every element takes,\n")
	b.WriteString("// nothing can click it, and it means nothing outside the canvas it was\n")
	b.WriteString("// written in. Every coordinate is a whole virtual pixel and every color\n")
	b.WriteString("// is a NUMBER, the index of a color in the canvas's palette.\n")
	b.WriteString("type Painter struct{ el int64 }\n\n")
	for _, op := range ops {
		name := op.str("name")
		var params, args []string
		for _, a := range strings.Split(op.str("args"), ",") {
			spec, _, _ := strings.Cut(strings.TrimSpace(a), "=")
			// `flip_x=flipX: bool = false`: the name before `=` is
			// ours, the rest is the .pix side's.
			spec = strings.TrimSpace(spec)
			an, at, _ := strings.Cut(spec, ":")
			an, at = strings.TrimSpace(an), strings.TrimSpace(at)
			if at == "" {
				// The type came after the .pix name: `flip_x=flipX: bool`.
				full := strings.TrimSpace(a)
				_, rest, _ := strings.Cut(full, ":")
				at, _, _ = strings.Cut(strings.TrimSpace(rest), "=")
				at = strings.TrimSpace(at)
			}
			var gt, conv string
			switch at {
			case "int":
				gt, conv = "int", "int64(%s)"
			case "str":
				gt, conv = "string", "%s"
			case "bool":
				gt, conv = "bool", "%s"
			default:
				fail("no type for op arg " + a)
			}
			params = append(params, lower(an)+" "+gt)
			args = append(args, fmt.Sprintf(conv, lower(an)))
		}
		fmt.Fprintf(&b, "func (p *Painter) %s(%s) { door.Op%s(p.el, %s) }\n",
			camel(name), strings.Join(params, ", "), camel(name), strings.Join(args, ", "))
	}
	return mustFormat(b.String())
}

// --- internal/symbols/symbols.go -----------------------------------------------------

// The interpreter's view of the package: every exported name of the
// package as a reflect value, in the table yaegi's `extract` writes,
// plus a wrapper for each interface an interpreted type may implement.
// It is read off the package's own source, elements.go included, so
// nothing an app can call is missing from what the interpreted run can
// call.
func genSymbols(elementsSrc, stdlibSrc []byte) []byte {
	fset := token.NewFileSet()
	files := map[string]*ast.File{}
	entries, err := os.ReadDir(".")
	if err != nil {
		fail(err.Error())
	}
	// The two generated files are read from what was just generated,
	// whether or not they are on disk yet; the rest from the directory.
	sources := map[string]any{"elements.go": elementsSrc, "stdlib.go": stdlibSrc}
	for _, en := range entries {
		n := en.Name()
		if en.IsDir() || !strings.HasSuffix(n, ".go") || strings.HasSuffix(n, "_test.go") {
			continue
		}
		if _, generated := sources[n]; !generated {
			sources[n] = nil
		}
	}
	for n, src := range sources {
		f, err := parser.ParseFile(fset, n, src, parser.ParseComments)
		if err != nil {
			fail(err.Error())
		}
		files[n] = f
	}
	var names []string
	kinds := map[string]string{}
	ifaces := map[string]*ast.InterfaceType{}
	typeNames := map[string]bool{}
	for _, f := range files {
		for _, d := range f.Decls {
			switch d := d.(type) {
			case *ast.FuncDecl:
				if d.Recv != nil || !d.Name.IsExported() || d.Type.TypeParams != nil {
					continue
				}
				names = append(names, d.Name.Name)
				kinds[d.Name.Name] = "func"
			case *ast.GenDecl:
				for _, s := range d.Specs {
					switch s := s.(type) {
					case *ast.TypeSpec:
						if !s.Name.IsExported() || s.TypeParams != nil {
							continue
						}
						names = append(names, s.Name.Name)
						kinds[s.Name.Name] = "type"
						typeNames[s.Name.Name] = true
						if it, ok := s.Type.(*ast.InterfaceType); ok {
							ifaces[s.Name.Name] = it
						}
					case *ast.ValueSpec:
						for _, n := range s.Names {
							if n.IsExported() && d.Tok == token.VAR {
								names = append(names, n.Name)
								kinds[n.Name] = "var"
							}
						}
					}
				}
			}
		}
	}
	sort.Strings(names)

	var b strings.Builder
	b.WriteString(banner)
	b.WriteString("\n// Package symbols is the interpreter's view of the gomamochi package:\n")
	b.WriteString("// every exported name as a reflect value, and a wrapper for each\n")
	b.WriteString("// interface an interpreted type may implement.\n")
	b.WriteString("package symbols\n\nimport (\n\t\"reflect\"\n\n\tgm \"github.com/i2y/yokan/gomamochi\"\n)\n\n")
	b.WriteString("// Symbols is what the host hands to interp.Use.\n")
	b.WriteString("var Symbols = map[string]map[string]reflect.Value{}\n\n")
	b.WriteString("func init() {\n\tSymbols[\"github.com/i2y/yokan/gomamochi/gomamochi\"] = map[string]reflect.Value{\n")
	for _, n := range names {
		switch kinds[n] {
		case "func", "var":
			fmt.Fprintf(&b, "\t\t%q: reflect.ValueOf(gm.%s),\n", n, n)
		case "type":
			fmt.Fprintf(&b, "\t\t%q: reflect.ValueOf((*gm.%s)(nil)),\n", n, n)
			if it, ok := ifaces[n]; ok && wrappable(it) {
				fmt.Fprintf(&b, "\t\t%q: reflect.ValueOf((*_gomamochi_%s)(nil)),\n", "_"+n, n)
			}
		}
	}
	b.WriteString("\t}\n}\n")

	// The wrappers.
	var inames []string
	for n := range ifaces {
		inames = append(inames, n)
	}
	sort.Strings(inames)
	for _, n := range inames {
		it := ifaces[n]
		if !wrappable(it) {
			continue
		}
		fmt.Fprintf(&b, "\n// _gomamochi_%s is an interface wrapper for %s.\ntype _gomamochi_%s struct {\n\tIValue interface{}\n", n, n, n)
		for _, m := range it.Methods.List {
			for _, mn := range m.Names {
				fmt.Fprintf(&b, "\tW%s %s\n", mn.Name, qualify(fset, m.Type, typeNames))
			}
		}
		b.WriteString("}\n")
		for _, m := range it.Methods.List {
			ft := m.Type.(*ast.FuncType)
			for _, mn := range m.Names {
				var params, args []string
				for i, p := range ft.Params.List {
					pt := qualify(fset, p.Type, typeNames)
					if len(p.Names) == 0 {
						params = append(params, fmt.Sprintf("a%d %s", i, pt))
						args = append(args, fmt.Sprintf("a%d", i))
						continue
					}
					for _, pn := range p.Names {
						params = append(params, pn.Name+" "+pt)
						args = append(args, pn.Name)
					}
				}
				results := ""
				if ft.Results != nil {
					var rs []string
					for _, r := range ft.Results.List {
						rs = append(rs, qualify(fset, r.Type, typeNames))
					}
					results = " " + strings.Join(rs, ", ")
					if len(rs) > 1 {
						results = " (" + strings.Join(rs, ", ") + ")"
					}
				}
				ret := ""
				if ft.Results != nil {
					ret = "return "
				}
				fmt.Fprintf(&b, "\nfunc (W _gomamochi_%s) %s(%s)%s { %sW.W%s(%s) }\n",
					n, mn.Name, strings.Join(params, ", "), results, ret, mn.Name, strings.Join(args, ", "))
			}
		}
	}
	return mustFormat(b.String())
}

// An interface an interpreted type can implement: every method
// exported, none of them generic.
func wrappable(it *ast.InterfaceType) bool {
	if it.Methods == nil || len(it.Methods.List) == 0 {
		return false
	}
	for _, m := range it.Methods.List {
		if len(m.Names) == 0 {
			return false // an embedded interface
		}
		for _, n := range m.Names {
			if !n.IsExported() {
				return false
			}
		}
	}
	return true
}

// qualify prints a type as the symbols package sees it: the package's
// own named types get the `gm.` prefix.
func qualify(fset *token.FileSet, t ast.Expr, typeNames map[string]bool) string {
	var buf bytes.Buffer
	printer.Fprint(&buf, fset, t)
	s := buf.String()
	re := regexp.MustCompile(`\b([A-Z]\w*)\b`)
	return re.ReplaceAllStringFunc(s, func(m string) string {
		if typeNames[m] {
			return "gm." + m
		}
		return m
	})
}

// --- stdlib.go, from the manifest ---------------------------------------------------------

// The framework's own standard library: `crates/yokan-stdlib/stdlib.toml`
// is one manifest that no language owns, and the C face carries its
// framework layer as numbered rows (rakugan/tools/gen_capi.pl writes
// those arms; the numbers are the rows of the `yokan` groups that name
// a Rust function, counted in order). Of those, Go keeps to what the
// engine mediates — a database, the clipboard, the platform's dialogs,
// sound, a notification — and leaves files, the network, JSON and the
// clock to its own standard library, which both runs share anyway.

type stdRow struct {
	id     int
	module string
	py     string
	params []stdParam
	ret    string
}

type stdParam struct{ name, kind string }

var stdKeep = map[string]bool{"sqlite": true, "clipboard": true, "audio": true, "notify": true}
var stdNames = map[string]string{"fs.open_dialog": "OpenDialog", "fs.save_dialog": "SaveDialog"}
var stdWhat = map[string]string{
	"sqlite": "reads or writes a database", "clipboard": "reads or writes the clipboard",
	"audio": "plays a sound", "notify": "sends a notification", "fs": "asks a person",
}

func loadStdlib(path string) []stdRow {
	text, err := os.ReadFile(path)
	if err != nil {
		fail(err.Error())
	}
	doc := parseTOML(string(text))
	var rows []stdRow
	id := 0
	for _, g := range doc["group"] {
		if g.str("layer") != "yokan" {
			continue
		}
		module := g.str("module")
		for _, r := range g.rows() {
			if !r.has("rust") {
				continue
			}
			id++
			row := stdRow{id: id, module: module, py: r.str("py"), ret: r.str("ret")}
			for _, p := range strings.Split(r.str("params"), ",") {
				p = strings.TrimSpace(p)
				if p == "" {
					continue
				}
				name, kind, _ := strings.Cut(p, ":")
				name = strings.TrimSpace(name)
				// A manifest name that is a Go keyword.
				if name == "default" {
					name = "fallback"
				}
				row.params = append(row.params, stdParam{name, strings.TrimSpace(kind)})
			}
			rows = append(rows, row)
		}
	}
	return rows
}

func (t table) rows() []table {
	var out []table
	if vs, ok := t["rows"].([]any); ok {
		for _, v := range vs {
			out = append(out, v.(table))
		}
	}
	return out
}

func stdGoName(r stdRow) string {
	if n, ok := stdNames[r.module+"."+r.py]; ok {
		return n
	}
	return camel(r.module) + camel(r.py)
}

func stdGoType(kind string) string {
	switch kind {
	case "String":
		return "string"
	case "Int":
		return "int"
	case "Float":
		return "float64"
	case "Bool":
		return "bool"
	case "List<String>":
		return "[]string"
	case "List<List<String>>":
		return "[][]string"
	case "":
		return ""
	}
	fail("no Go type for " + kind)
	return ""
}

// The lines that push one argument.
func stdPush(p stdParam) string {
	switch p.kind {
	case "String":
		return fmt.Sprintf("\tdoor.StdArgStr(%s)\n", lower(p.name))
	case "Int":
		return fmt.Sprintf("\tdoor.StdArgInt(int64(%s))\n", lower(p.name))
	case "Float":
		return fmt.Sprintf("\tdoor.StdArgNum(%s)\n", lower(p.name))
	case "List<String>":
		return fmt.Sprintf("\tdoor.StdArgList(%s)\n", lower(p.name))
	}
	fail("no push for " + p.kind)
	return ""
}

// The lines that make the call and keep its answer in `out`.
func stdAnswer(ret string) string {
	switch ret {
	case "Int":
		return "\t\tout = int(door.StdCall(%d))\n"
	case "Bool":
		return "\t\tout = door.StdCall(%d) != 0\n"
	case "Float":
		return "\t\tdoor.StdCall(%d)\n\t\tout = door.StdAnswerNum()\n"
	case "String":
		return "\t\tdoor.StdCall(%d)\n\t\tout = door.StdText()\n"
	case "List<String>":
		return "\t\tdoor.StdCall(%d)\n\t\tout = door.StdList()\n"
	case "List<List<String>>":
		return "\t\tdoor.StdCall(%d)\n\t\tout = door.StdRows()\n"
	case "":
		return "\t\tdoor.StdCall(%d)\n"
	}
	fail("no answer for " + ret)
	return ""
}

// genStdlib writes one Go function per kept row. A row and its twin
// with a trailing `params: List<String>` become one function whose
// last parameter is variadic; which row is called depends on whether
// any were given.
func genStdlib(rows []stdRow) ([]byte, map[string]string) {
	var kept []stdRow
	for _, r := range rows {
		if stdKeep[r.module] || stdNames[r.module+"."+r.py] != "" {
			if r.module == "audio" && r.py == "play" && len(r.params) == 1 {
				continue // `play(path)`: the two-argument row serves both
			}
			kept = append(kept, r)
		}
	}
	names := map[string]string{}
	var b strings.Builder
	b.WriteString(stdBanner)
	b.WriteString("\npackage gomamochi\n\n")
	b.WriteString("// The framework's own standard library: what the engine mediates, one\n")
	b.WriteString("// implementation that both runs reach through the C face, so a run\n")
	b.WriteString("// under a script keeps a dialog answered by `file:` and a sound silent\n")
	b.WriteString("// in both. Files, the network, JSON and the clock are Go's own.\n\n")
	b.WriteString("import \"github.com/i2y/yokan/gomamochi/internal/door\"\n\n")
	done := map[string]bool{}
	for i, r := range kept {
		name := stdGoName(r)
		if done[name] {
			continue
		}
		done[name] = true
		names[name] = stdWhat[r.module]
		// A twin with the trailing list of bound values?
		var with *stdRow
		for j := i + 1; j < len(kept); j++ {
			t := kept[j]
			if stdGoName(t) == name && len(t.params) == len(r.params)+1 && t.params[len(r.params)].kind == "List<String>" {
				with = &kept[j]
				break
			}
		}
		var params []string
		for _, p := range r.params {
			params = append(params, lower(p.name)+" "+stdGoType(p.kind))
		}
		if with != nil {
			params = append(params, lower(with.params[len(r.params)].name)+" ...string")
		}
		ret := stdGoType(r.ret)
		if ret != "" {
			ret = " (out " + ret + ")"
		}
		// The whole call — the arguments, the row, the answer — runs on
		// one thread inside door.Std: the face keeps the pieces in that
		// thread's own state.
		fmt.Fprintf(&b, "// %s is `%s.%s` in the manifest.\n", name, r.module, r.py)
		fmt.Fprintf(&b, "func %s(%s)%s {\n\tdoor.Std(func() {\n\t\tdoor.StdReset()\n", name, strings.Join(params, ", "), ret)
		for _, p := range r.params {
			b.WriteString("\t" + stdPush(p))
		}
		if with != nil {
			last := lower(with.params[len(r.params)].name)
			fmt.Fprintf(&b, "\t\tif len(%s) > 0 {\n\t\t\tdoor.StdArgList(%s)\n", last, last)
			b.WriteString(strings.ReplaceAll(fmt.Sprintf(stdAnswer(r.ret), with.id), "\t\t", "\t\t\t"))
			b.WriteString("\t\t\treturn\n\t\t}\n")
		}
		b.WriteString(fmt.Sprintf(stdAnswer(r.ret), r.id))
		b.WriteString("\t})\n")
		if ret != "" {
			b.WriteString("\treturn\n")
		}
		b.WriteString("}\n\n")
	}
	return mustFormat(b.String()), names
}

// genStdlibNames is what the checker reads: the functions above, and
// why a view may not call them.
func genStdlibNames(names map[string]string) []byte {
	var keys []string
	for k := range names {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var b strings.Builder
	b.WriteString(stdBanner)
	b.WriteString("\npackage check\n\n// The framework's own standard library, and what each call does that\n// a view may not: the same list the package exports, kept here so the\n// checker needs no import of it.\nvar stdlibNames = map[string]string{\n")
	for _, k := range keys {
		fmt.Fprintf(&b, "\t%q: %q,\n", k, names[k])
	}
	b.WriteString("}\n")
	return mustFormat(b.String())
}

// --- write, or check -----------------------------------------------------------------

func mustFormat(src string) []byte {
	out, err := format.Source([]byte(src))
	if err != nil {
		os.Stderr.WriteString(src)
		fail("the generated Go does not parse: " + err.Error())
	}
	return out
}

func fail(msg string) {
	fmt.Fprintln(os.Stderr, "gen:", msg)
	os.Exit(1)
}

func main() {
	check := len(os.Args) > 1 && os.Args[1] == "--check"
	load(filepath.Join("..", "crates", "pixie-capi", "elements.toml"))
	el := genElements()
	std, names := genStdlib(loadStdlib(filepath.Join("..", "crates", "yokan-stdlib", "stdlib.toml")))
	files := []struct {
		path string
		body []byte
	}{
		{"elements.go", el},
		{"stdlib.go", std},
		{filepath.Join("internal", "check", "stdlib_names.go"), genStdlibNames(names)},
		{filepath.Join("internal", "symbols", "symbols.go"), genSymbols(el, std)},
	}
	var stale []string
	for _, f := range files {
		if check {
			on, err := os.ReadFile(f.path)
			if err != nil || !bytes.Equal(on, f.body) {
				stale = append(stale, f.path)
			}
			continue
		}
		if err := os.WriteFile(f.path, f.body, 0o644); err != nil {
			fail(err.Error())
		}
		fmt.Println("wrote", f.path)
	}
	if len(stale) > 0 {
		fmt.Fprintln(os.Stderr, "gen: these are not what elements.toml says — run `go run ./tools/gen`")
		for _, p := range stale {
			fmt.Fprintln(os.Stderr, "  "+p)
		}
		os.Exit(1)
	}
}
