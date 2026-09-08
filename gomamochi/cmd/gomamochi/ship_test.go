package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// The AppDir is a Linux shape, but its layout can be checked on any
// machine: the binary and the engine's library side by side under
// usr/bin, the .desktop entry, the icon it names, and an AppRun that
// runs the binary. What cannot be checked here is that it runs there.
func TestAppDirLayout(t *testing.T) {
	root, _ = filepath.Abs("../..")
	repo = filepath.Dir(root)
	if target = os.Getenv("CARGO_TARGET_DIR"); target == "" {
		home, _ := os.UserHomeDir()
		target = filepath.Join(home, ".cache", "pixie", "target")
	}
	if _, err := os.Stat(libPath()); err != nil {
		t.Skip("no engine built:", err)
	}
	dir := t.TempDir()
	app := filepath.Join(dir, "counter.go")
	src, err := os.ReadFile(filepath.Join(root, "demo", "counter.go"))
	if err != nil {
		t.Fatal(err)
	}
	os.WriteFile(app, src, 0o644)
	bin := filepath.Join(dir, "counter")
	os.WriteFile(bin, []byte("#!/bin/sh\necho stand-in\n"), 0o755)

	out := appDir(app, bin, false)
	if filepath.Base(out) != "counter.AppDir" {
		t.Fatalf("named %s, want counter.AppDir", filepath.Base(out))
	}
	for _, rel := range []string{"usr/bin/counter", "usr/bin/" + filepath.Base(libPath()), "counter.desktop", "AppRun", "counter.svg"} {
		if _, err := os.Stat(filepath.Join(out, rel)); err != nil {
			t.Errorf("missing %s: %v", rel, err)
		}
	}
	desktop, _ := os.ReadFile(filepath.Join(out, "counter.desktop"))
	if !strings.Contains(string(desktop), "Name=counter\n") || !strings.Contains(string(desktop), "Exec=counter\n") {
		t.Errorf("desktop entry:\n%s", desktop)
	}
	run, _ := os.ReadFile(filepath.Join(out, "AppRun"))
	if !strings.Contains(string(run), `exec "$HERE/usr/bin/counter" "$@"`) {
		t.Errorf("AppRun:\n%s", run)
	}
	if st, _ := os.Stat(filepath.Join(out, "AppRun")); st.Mode()&0o111 == 0 {
		t.Error("AppRun is not executable")
	}
}

// A window's title comes off the app's own Run line, and a name a
// shell can hold is made from it.
func TestWindowTitle(t *testing.T) {
	dir := t.TempDir()
	app := filepath.Join(dir, "x.go")
	os.WriteFile(app, []byte(`package main
func main() { Run(&A{}, Title("Pyxel Jump"), Size(1, 2)) }
`), 0o644)
	if got := windowTitle(app); got != "Pyxel Jump" {
		t.Errorf("title %q", got)
	}
	if got := safeName("Pyxel Jump", "x", false); got != "Pyxel_Jump" {
		t.Errorf("safe name %q", got)
	}
	if got := safeName("Pyxel Jump", "x", true); got != "Pyxel Jump" {
		t.Errorf("bundle name %q", got)
	}
}
