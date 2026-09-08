package main

// Shipping: the binary and the engine's library as one application.
// The engine rides beside the executable in every shape — the door
// looks there first — so nothing is installed and nothing is looked
// up. macOS gets an application bundle, Linux an AppDir that
// appimagetool packs into one file; each names itself off its
// platform rather than producing a directory nothing there opens.

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"

	"github.com/i2y/yokan/gomamochi/internal/door"
)

// The window's title, read off the app's own `Run` line: it names the
// bundle and shows in the menu bar.
func windowTitle(app string) string {
	src, _ := os.ReadFile(app)
	m := regexp.MustCompile(`Title\("((?:[^"\\]|\\.)*)"\)`).FindSubmatch(src)
	if m == nil {
		return stem(app)
	}
	return string(m[1])
}

func safeName(title, fallback string, spaces bool) string {
	var keep string
	if spaces {
		keep = regexp.MustCompile(`[^A-Za-z0-9 ._-]`).ReplaceAllString(title, "")
	} else {
		keep = regexp.MustCompile(`[^A-Za-z0-9._-]+`).ReplaceAllString(title, "_")
	}
	keep = strings.Trim(keep, " _")
	if keep == "" {
		return fallback
	}
	return keep
}

func copyFile(src, dst string, mode os.FileMode) {
	data, err := os.ReadFile(src)
	if err != nil {
		die(err.Error())
	}
	if err := os.WriteFile(dst, data, mode); err != nil {
		die(err.Error())
	}
}

func dirSize(root string) string {
	var total int64
	filepath.Walk(root, func(_ string, st os.FileInfo, err error) error {
		if err == nil && !st.IsDir() {
			total += st.Size()
		}
		return nil
	})
	return fmt.Sprintf("%.1f MB", float64(total)/1_000_000)
}

func quietly(name string, args ...string) bool {
	cmd := exec.Command(name, args...)
	return cmd.Run() == nil
}

// --- macOS ----------------------------------------------------------------------

// An icon rides along when <stem>.icns or <stem>.png sits beside the
// app. A PNG goes through the only pipeline macOS offers: sips writes
// every size into an iconset and iconutil packs it. If either balks
// the bundle simply has no icon, which is not worth failing a build
// over.
func installIcon(app, res string) string {
	dir := filepath.Dir(absPath(app))
	dest := filepath.Join(res, "icon.icns")
	icns := filepath.Join(dir, stem(app)+".icns")
	png := filepath.Join(dir, stem(app)+".png")
	switch {
	case exists(icns):
		copyFile(icns, dest, 0o644)
	case exists(png):
		if !pngToIcns(png, dest) {
			return ""
		}
	default:
		return ""
	}
	return "  <key>CFBundleIconFile</key>\n  <string>icon</string>\n"
}

func exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func pngToIcns(png, dest string) bool {
	td, err := os.MkdirTemp("", "gomamochi-icon")
	if err != nil {
		return false
	}
	defer os.RemoveAll(td)
	iconset := filepath.Join(td, "app.iconset")
	os.MkdirAll(iconset, 0o755)
	for _, base := range []int{16, 32, 128, 256, 512} {
		for _, scale := range []int{1, 2} {
			px := fmt.Sprint(base * scale)
			name := fmt.Sprintf("icon_%dx%d.png", base, base)
			if scale == 2 {
				name = fmt.Sprintf("icon_%dx%d@2x.png", base, base)
			}
			if !quietly("sips", "-z", px, px, png, "--out", filepath.Join(iconset, name)) {
				return false
			}
		}
	}
	return quietly("iconutil", "-c", "icns", iconset, "-o", dest)
}

// `--app` on macOS: an application bundle under dist/, beside the app.
// The binary and the engine's library sit together in Contents/MacOS,
// which is where the door looks, and the bundle links nothing but the
// system's own frameworks: it runs on a machine with neither Go nor
// the toolchain installed.
func appBundle(app, bin string) string {
	title := windowTitle(app)
	safe := safeName(title, stem(app), true)
	root := filepath.Join(filepath.Dir(absPath(app)), "dist", safe+".app")
	macos := filepath.Join(root, "Contents", "MacOS")
	res := filepath.Join(root, "Contents", "Resources")
	os.RemoveAll(root)
	for _, d := range []string{macos, res} {
		if err := os.MkdirAll(d, 0o755); err != nil {
			die(err.Error())
		}
	}
	copyFile(bin, filepath.Join(macos, stem(app)), 0o755)
	copyFile(libPath(), filepath.Join(macos, door.Library()), 0o755)
	icon := installIcon(app, res)
	plist := fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>%s</string>
  <key>CFBundleDisplayName</key>
  <string>%s</string>
  <key>CFBundleExecutable</key>
  <string>%s</string>
  <key>CFBundleIdentifier</key>
  <string>com.gomamochi.%s</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
%s</dict>
</plist>
`, title, title, stem(app), stem(app), icon)
	if err := os.WriteFile(filepath.Join(root, "Contents", "Info.plist"), []byte(plist), 0o644); err != nil {
		die(err.Error())
	}
	// Ad-hoc signed, like every other artifact here: unsigned, macOS
	// refuses to open it at all. `--deep` signs the library inside.
	cmd := exec.Command("codesign", "--force", "--deep", "-s", "-", root)
	if out, err := cmd.CombinedOutput(); err != nil {
		os.Stderr.Write(out)
		die("codesign failed")
	}
	return root
}

// --- Linux -------------------------------------------------------------------------

// The C runtime and the loader that started the process. A carried
// copy of one of these does not wait for a different machine to break
// the app; it breaks it on the machine that built it. No flag turns
// them back on. The list is the one Yokan's packer keeps.
var coreLibs = map[string]bool{}

func init() {
	for _, n := range strings.Fields(`ld-linux-aarch64.so.1 ld-linux-x86-64.so.2 ld-linux.so.2
		libBrokenLocale.so.1 libanl.so.1 libc.so.6 libcidn.so.1 libcom_err.so.2 libdl.so.2
		libgcc_s.so.1 libm.so.6 libmvec.so.1 libnss_compat.so.2 libnss_dns.so.2 libnss_files.so.2
		libnss_hesiod.so.2 libnss_nis.so.2 libnss_nisplus.so.2 libpthread.so.0 libresolv.so.2
		librt.so.1 libstdc++.so.6 libthread_db.so.1 libutil.so.1`) {
		coreLibs[n] = true
	}
}

// The shared libraries the engine resolves, less the C runtime: what
// `--carry-libs` copies in. Without the flag a package carries the
// engine alone and leaves the desktop's libraries — the graphics
// stack in particular — to the host, which is the only copy that
// talks to the host's driver; the list of what a Linux machine has
// to provide is in the repository's notes.
func carriedLibs(engine string) []string {
	out, err := exec.Command("ldd", engine).Output()
	if err != nil {
		return nil
	}
	var libs []string
	for _, line := range strings.Split(string(out), "\n") {
		soname, rest, ok := strings.Cut(strings.TrimSpace(line), " => ")
		if !ok {
			continue
		}
		path := strings.TrimSpace(strings.SplitN(rest, " (", 2)[0])
		if path != "" && exists(path) && !coreLibs[strings.TrimSpace(soname)] {
			libs = append(libs, path)
		}
	}
	return libs
}

// A plain square in the engine's accent: appimagetool requires the
// file the .desktop entry names, so an app that brought no icon still
// gets one rather than a refusal.
func writeIcon(path string) {
	quietly("sh", "-c", "true")
	const side = 64
	// A minimal PNG is more code than it is worth here; a 64x64 PPM
	// converted by the tools a Linux desktop has would still be a
	// guess. Write an SVG: appimagetool takes one.
	svg := fmt.Sprintf(`<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d"><rect width="%d" height="%d" rx="12" fill="#89b4fa"/></svg>`, side, side, side, side)
	os.WriteFile(path, []byte(svg), 0o644)
}

// `--app` on Linux: the AppDir, which is both a runnable directory and
// the thing appimagetool packs. The binary and the engine's library sit
// together in usr/bin, which is where the door looks; `AppRun` puts
// carried libraries ahead of the host's for this process only.
func appDir(app, bin string, carry bool) string {
	title := windowTitle(app)
	safe := safeName(title, stem(app), false)
	root := filepath.Join(filepath.Dir(absPath(app)), "dist", safe+".AppDir")
	os.RemoveAll(root)
	usrBin := filepath.Join(root, "usr", "bin")
	usrLib := filepath.Join(root, "usr", "lib")
	for _, d := range []string{usrBin, usrLib} {
		if err := os.MkdirAll(d, 0o755); err != nil {
			die(err.Error())
		}
	}
	copyFile(bin, filepath.Join(usrBin, stem(app)), 0o755)
	copyFile(libPath(), filepath.Join(usrBin, door.Library()), 0o755)
	if carry {
		for _, lib := range carriedLibs(libPath()) {
			copyFile(lib, filepath.Join(usrLib, filepath.Base(lib)), 0o755)
		}
	}
	icon := filepath.Join(root, stem(app)+".png")
	if src := filepath.Join(filepath.Dir(absPath(app)), stem(app)+".png"); exists(src) {
		copyFile(src, icon, 0o644)
	} else {
		icon = filepath.Join(root, stem(app)+".svg")
		writeIcon(icon)
	}
	desktop := fmt.Sprintf("[Desktop Entry]\nType=Application\nName=%s\nExec=%s\nIcon=%s\nCategories=Utility;\nTerminal=false\n",
		title, stem(app), stem(app))
	os.WriteFile(filepath.Join(root, stem(app)+".desktop"), []byte(desktop), 0o644)
	run := "#!/bin/sh\n# Generated by gomamochi. The carried libraries go ahead of the\n# host's for this process only.\n" +
		"HERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\n" +
		"export LD_LIBRARY_PATH=\"$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}\"\n" +
		fmt.Sprintf("exec \"$HERE/usr/bin/%s\" \"$@\"\n", stem(app))
	os.WriteFile(filepath.Join(root, "AppRun"), []byte(run), 0o755)
	return root
}

func appimageArch() string {
	switch runtime.GOARCH {
	case "arm64":
		return "aarch64"
	case "amd64":
		return "x86_64"
	}
	return runtime.GOARCH
}

// The packer, fetched once into the cache the command already keeps.
func appimagetool() string {
	home, _ := os.UserHomeDir()
	dest := filepath.Join(home, ".cache", "gomamochi", "appimagetool-"+appimageArch())
	if exists(dest) {
		return dest
	}
	url := "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-" + appimageArch() + ".AppImage"
	fmt.Fprintf(os.Stderr, "fetching appimagetool (%s) into %s\n  first --appimage build only; later builds reuse it\n", appimageArch(), dest)
	os.MkdirAll(filepath.Dir(dest), 0o755)
	resp, err := http.Get(url)
	if err != nil {
		die("could not fetch appimagetool from " + url + ": " + err.Error())
	}
	defer resp.Body.Close()
	out, err := os.OpenFile(dest, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o755)
	if err != nil {
		die(err.Error())
	}
	if _, err := io.Copy(out, resp.Body); err != nil {
		die(err.Error())
	}
	out.Close()
	return dest
}

// `--appimage`: the AppDir packed into the one file Linux hands
// around. The tool is itself an AppImage; without FUSE it is asked to
// unpack itself first.
func appImage(root string) string {
	name := strings.TrimSuffix(filepath.Base(root), ".AppDir")
	out := filepath.Join(filepath.Dir(root), name+"-"+appimageArch()+".AppImage")
	cmd := exec.Command(appimagetool(), root, out)
	cmd.Env = append(os.Environ(), "APPIMAGE_EXTRACT_AND_RUN=1", "ARCH="+appimageArch())
	if b, err := cmd.CombinedOutput(); err != nil {
		os.Stderr.Write(b)
		die("appimagetool failed")
	}
	return out
}
