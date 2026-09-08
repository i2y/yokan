// The Gomamochi command: one Go app, four things you can do to it.
//
//	gomamochi check demo/counter.go
//	gomamochi run   demo/counter.go
//	gomamochi build demo/counter.go [--release]
//	gomamochi gate  demo/counter.go --script "click:+1,dump"
//
// `gate` is the one that matters. It runs the app twice — under yaegi
// through the door, which opens pixie's C face, and as the binary gc
// built from the same file, which opens the same library — with one
// interaction script, and compares the two transcripts byte for byte.
// Everything else is a step of that round you may want on its own.
//
// The interpreted run lives in this program: yaegi, the standard
// library's symbols and the door are compiled into it, and an app's
// file is read as it is. `run` needs no Go toolchain; `build` and
// `gate` need `go`, and every command builds the engine first so that
// neither run is a version behind.
package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing/fstest"

	"github.com/traefik/yaegi/interp"
	"github.com/traefik/yaegi/stdlib"

	"github.com/i2y/yokan/gomamochi/internal/check"
	"github.com/i2y/yokan/gomamochi/internal/door"
	"github.com/i2y/yokan/gomamochi/internal/symbols"
)

const usage = `usage: gomamochi <command> <app.go> [--script "step,step"] [--fresh <path>]

  check      what the app writes that Gomamochi cannot take
  run        run it interpreted, in a window; a save reloads the file
  build      build the native binary [--release]
  gate       run it both ways headless and compare the two runs

--release drops the symbol table.
--fresh deletes a path before EACH run, so an app that keeps a file or
a database starts both runs from the same nothing. Repeatable.
`

// Where things are: the module (where go build runs), the checkout
// (where the engine builds), and the shared cargo target dir.
var root, repo, target string

func die(msg string) {
	fmt.Fprintln(os.Stderr, "gomamochi:", msg)
	os.Exit(1)
}

func locate(app string) {
	root = os.Getenv("GOMAMOCHI_ROOT")
	if root == "" {
		dir := filepath.Dir(absPath(app))
		for {
			if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
				root = dir
				break
			}
			up := filepath.Dir(dir)
			if up == dir {
				die("no go.mod above " + app + " — run it through bin/gomamochi, or set GOMAMOCHI_ROOT")
			}
			dir = up
		}
	}
	repo = filepath.Dir(root)
	target = os.Getenv("CARGO_TARGET_DIR")
	if target == "" {
		home, _ := os.UserHomeDir()
		target = filepath.Join(home, ".cache", "pixie", "target")
	}
}

func absPath(p string) string {
	a, err := filepath.Abs(p)
	if err != nil {
		die(err.Error())
	}
	return a
}

// --- the engine ---------------------------------------------------------------

var engineDir string

// pixie's C face: one library, opened by both runs. Built here so
// neither run can be a version behind.
func engine() string {
	if engineDir != "" {
		return engineDir
	}
	cmd := exec.Command("cargo", "build", "--release", "-q", "-p", "pixie-capi")
	cmd.Dir = repo
	cmd.Env = append(os.Environ(), "CARGO_TARGET_DIR="+target)
	if out, err := cmd.CombinedOutput(); err != nil {
		os.Stderr.Write(out)
		die("the engine did not build")
	}
	engineDir = filepath.Join(target, "release")
	return engineDir
}

func libPath() string { return filepath.Join(engine(), door.Library()) }

// --- the steps ------------------------------------------------------------------

func stem(app string) string { return strings.TrimSuffix(filepath.Base(app), ".go") }

func gateDir(app string) string {
	dir := filepath.Join(filepath.Dir(absPath(app)), ".gate")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		die(err.Error())
	}
	return dir
}

// What the app writes that Gomamochi cannot take, named with the line.
// Nothing is printed when there is nothing to say.
func refusals(app string) bool {
	rs, err := check.Run(app)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return false
	}
	src, _ := os.ReadFile(app)
	lines := strings.Split(string(src), "\n")
	for _, r := range rs {
		fmt.Fprintln(os.Stderr, check.Render(r, lines))
	}
	return len(rs) == 0
}

// The interpreted run: the file, with its loop variables made
// per-iteration, read by a fresh interpreter that has the standard
// library and the door compiled in.
func interpret(path string) (err error) {
	src, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	out, rs := check.Rewrite(path, src)
	if len(rs) > 0 {
		lines := strings.Split(string(src), "\n")
		for _, r := range rs {
			fmt.Fprintln(os.Stderr, check.Render(r, lines))
		}
		return fmt.Errorf("refused")
	}
	name := filepath.Base(path)
	i := interp.New(interp.Options{
		SourcecodeFilesystem: fstest.MapFS{name: &fstest.MapFile{Data: out}},
		Args:                 []string{path},
		Unrestricted:         true,
	})
	if err := i.Use(stdlib.Symbols); err != nil {
		return err
	}
	if err := i.Use(symbols.Symbols); err != nil {
		return err
	}
	// The interpreter stops on some of Go with a panic rather than an
	// error; a stop is reported like any other.
	defer func() {
		if e := recover(); e != nil {
			err = fmt.Errorf("the interpreter stopped on %s: %v", name, e)
		}
	}()
	_, err = i.EvalPath(name)
	return err
}

// The compiled run: gc, no cgo, the engine linked at run time by the
// door. It rides beside the binary — a link into the shared target dir
// here, a copy in a bundle later.
func build(app string, release bool) string {
	dir := filepath.Join(gateDir(app), stem(app))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		die(err.Error())
	}
	bin := filepath.Join(dir, stem(app))
	args := []string{"build", "-o", bin}
	if release {
		args = append(args, "-ldflags=-s -w")
	}
	args = append(args, absPath(app))
	cmd := exec.Command("go", args...)
	cmd.Dir = root
	cmd.Env = append(os.Environ(), "CGO_ENABLED=0")
	if out, err := cmd.CombinedOutput(); err != nil {
		os.Stderr.Write(out)
		die("go build failed")
	}
	link := filepath.Join(dir, door.Library())
	os.Remove(link)
	if err := os.Symlink(libPath(), link); err != nil {
		die(err.Error())
	}
	return bin
}

// Run a command headless and keep what it printed. A failed run is
// worth reading; a passed one is not.
func capture(env []string, what string, name string, args ...string) string {
	cmd := exec.Command(name, args...)
	cmd.Env = append(os.Environ(), env...)
	var out, errb strings.Builder
	cmd.Stdout, cmd.Stderr = &out, &errb
	if err := cmd.Run(); err != nil {
		if s := strings.TrimSpace(errb.String()); s != "" {
			fmt.Fprintln(os.Stderr, s)
		}
		die(what + " failed")
	}
	return out.String()
}

func wipe(paths []string) {
	for _, p := range paths {
		os.RemoveAll(p)
	}
}

func rel(path string) string {
	if wd, err := os.Getwd(); err == nil {
		if r, err := filepath.Rel(wd, path); err == nil && !strings.HasPrefix(r, "..") {
			return r
		}
	}
	return path
}

func sizeOf(path string) string {
	st, err := os.Stat(path)
	if err != nil {
		return "?"
	}
	return fmt.Sprintf("%.1f MB", float64(st.Size())/1_000_000)
}

// --- the commands -----------------------------------------------------------------

type options struct {
	script  string
	fresh   []string
	release bool
}

func cmdCheck(app string, _ options) {
	if !refusals(app) {
		os.Exit(1)
	}
}

func cmdRun(app string, _ options) {
	if !refusals(app) {
		os.Exit(1)
	}
	if os.Getenv("PIXIE_CAPI") == "" {
		os.Setenv("PIXIE_CAPI", libPath())
	}
	path := absPath(app)
	// Only a windowed run watches the file; under a script there is
	// nobody saving it.
	if os.Getenv("PIXIE_SCRIPT") == "" {
		door.Watch(path, func() bool {
			if err := interpret(path); err != nil {
				fmt.Fprintln(os.Stderr, "gomamochi:", err)
				return false
			}
			return true
		})
	}
	if err := interpret(path); err != nil {
		die(err.Error())
	}
}

func cmdBuild(app string, o options) {
	if !refusals(app) {
		os.Exit(1)
	}
	bin := build(app, o.release)
	fmt.Printf("built: %s (%s)\n", rel(bin), sizeOf(bin))
	fmt.Println("  not gate-checked — `gate` with a script proves the two runs agree")
}

func cmdGate(app string, o options) {
	if !refusals(app) {
		os.Exit(1)
	}
	exe, err := os.Executable()
	if err != nil {
		die(err.Error())
	}
	env := []string{"PIXIE_SCRIPT=" + o.script, "PIXIE_CAPI=" + libPath()}
	wipe(o.fresh)
	a := strings.TrimRight(capture(env, "the interpreted run", exe, "run", app), "\n")
	bin := build(app, false)
	wipe(o.fresh)
	b := strings.TrimRight(capture(env, "the compiled run", bin), "\n")

	if a == b {
		n := len(strings.Split(a, "\n"))
		if a == "" {
			n = 0
		}
		fmt.Printf("GATE OK — %d dump lines identical in both runs\n", n)
		script := o.script
		if script == "" {
			script = "(none — startup dump only)"
		}
		fmt.Printf("  script:   %s\n", script)
		fmt.Printf("  binary:   %s (%s)\n", rel(bin), sizeOf(bin))
		return
	}
	fmt.Println("GATE FAILED — the two runs diverge:")
	al, bl := strings.Split(a, "\n"), strings.Split(b, "\n")
	for i := 0; i < len(al) || i < len(bl); i++ {
		var x, y string
		if i < len(al) {
			x = al[i]
		} else {
			x = "(no line)"
		}
		if i < len(bl) {
			y = bl[i]
		} else {
			y = "(no line)"
		}
		if x == y {
			continue
		}
		fmt.Printf("  yaegi:    %s\n", x)
		fmt.Printf("  compiled: %s\n", y)
	}
	os.Exit(1)
}

// --- the argument line -------------------------------------------------------------

func main() {
	args := os.Args[1:]
	if len(args) == 0 {
		die("no command\n\n" + usage)
	}
	mode := args[0]
	commands := map[string]func(string, options){
		"check": cmdCheck, "run": cmdRun, "build": cmdBuild, "gate": cmdGate,
	}
	run, ok := commands[mode]
	if !ok {
		die("unknown command `" + mode + "`\n\n" + usage)
	}
	var app string
	var o options
	rest := args[1:]
	for len(rest) > 0 {
		arg := rest[0]
		rest = rest[1:]
		switch {
		case arg == "--script":
			if len(rest) == 0 {
				die("--script takes the steps")
			}
			o.script, rest = rest[0], rest[1:]
		case arg == "--fresh":
			if len(rest) == 0 {
				die("--fresh takes a path")
			}
			o.fresh, rest = append(o.fresh, rest[0]), rest[1:]
		case arg == "--release":
			o.release = true
		case strings.HasPrefix(arg, "--"):
			die("unknown option `" + arg + "`\n\n" + usage)
		default:
			if app != "" {
				die("gomamochi " + mode + " takes one app")
			}
			app = arg
		}
	}
	if app == "" {
		die("gomamochi " + mode + " takes an app: gomamochi " + mode + " demo/counter.go")
	}
	if st, err := os.Stat(app); err != nil || st.IsDir() {
		die("no such file: " + app)
	}
	if o.script != "" && mode != "gate" {
		die(mode + " takes no --script — replaying a script is the gate's job")
	}
	if len(o.fresh) > 0 && mode != "gate" {
		die("--fresh belongs to gate: it is there so the two runs start alike")
	}
	if o.release && mode != "build" {
		die("--release belongs to build: it is how an app ships")
	}
	locate(app)
	run(app, o)
}
