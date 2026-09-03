//! Compile-and-run acceptance: every demo's headless script runs
//! through BOTH execution tiers — compiled `build()` and the rung-2
//! interpreter — and must print identical element trees (§5.11 / R3:
//! a behavioral difference between tiers is a release blocker).
//!
//! Slow (real cargo builds of the generated crates) and dependent on
//! the shared runtime (`pixie install-runtime` once per machine), so
//! opt-in:
//!
//!     cargo test -p pixie-cli -- --ignored
//!
//! This is the merge gate for widget work: run it after every widget
//! lands.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// A tiny in-process HTTP fixture for the http demo — the gate stays
/// hermetic (no network, no external server). One thread accepts,
/// one per connection answers, `Connection: close` keeps ureq's
/// pooling honest about the short-lived server.
fn http_fixture_base() -> &'static str {
    static BASE: OnceLock<String> = OnceLock::new();
    BASE.get_or_init(|| {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fixture");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                std::thread::spawn(move || {
                    let mut buf: Vec<u8> = Vec::new();
                    let mut tmp = [0u8; 2048];
                    let header_end = loop {
                        match s.read(&mut tmp) {
                            Ok(0) => return,
                            Ok(n) => {
                                buf.extend_from_slice(&tmp[..n]);
                                if let Some(p) =
                                    buf.windows(4).position(|w| w == b"\r\n\r\n")
                                {
                                    break p + 4;
                                }
                            }
                            Err(_) => return,
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
                    let mut lines = head.lines();
                    let request = lines.next().unwrap_or_default().to_string();
                    let mut content_length = 0usize;
                    let mut x_pixie = String::from("none");
                    for l in lines {
                        let lower = l.to_ascii_lowercase();
                        if let Some(v) = lower.strip_prefix("content-length:") {
                            content_length = v.trim().parse().unwrap_or(0);
                        }
                        if let Some(v) = l
                            .strip_prefix("X-Pixie:")
                            .or_else(|| l.strip_prefix("x-pixie:"))
                        {
                            x_pixie = v.trim().to_string();
                        }
                    }
                    while buf.len() < header_end + content_length {
                        match s.read(&mut tmp) {
                            Ok(0) => break,
                            Ok(n) => buf.extend_from_slice(&tmp[..n]),
                            Err(_) => return,
                        }
                    }
                    let body_in = &buf[header_end..(header_end + content_length).min(buf.len())];
                    let (status, body): (&str, Vec<u8>) =
                        if request.starts_with("GET /hello ") {
                            ("200 OK", b"hello from fixture".to_vec())
                        } else if request.starts_with("GET /blob ") {
                            ("200 OK", (0u8..7).collect())
                        } else if request.starts_with("GET /tag ") {
                            ("200 OK", format!("tag={x_pixie}").into_bytes())
                        } else if request.starts_with("POST /echo ") {
                            let mut v = b"echo:".to_vec();
                            v.extend_from_slice(body_in);
                            ("200 OK", v)
                        } else {
                            ("404 Not Found", b"nope".to_vec())
                        };
                    let _ = write!(
                        s,
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(&body);
                });
            }
        });
        format!("http://127.0.0.1:{port}")
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn shared_bin(stem: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
        .join(".cache/pixie/target/debug")
        .join(stem)
}

/// Build one demo through the real CLI, then run its script in both
/// tiers and return (compiled stdout, interp stdout, interp stderr).
fn run_both(rel: &str, script: &str) -> (String, String, String) {
    let pix = repo_root().join(rel);
    let status = Command::new(env!("CARGO_BIN_EXE_pixie"))
        .arg("build")
        .arg(&pix)
        .status()
        .expect("pixie build runs");
    assert!(status.success(), "pixie build failed for {rel}");
    let stem = pix.file_stem().unwrap().to_str().unwrap().replace('-', "_");
    let bin = shared_bin(&stem);

    let compiled = Command::new(&bin)
        .env("PIXIE_SCRIPT", script)
        .env("PIXIE_HTTP_BASE", http_fixture_base())
        .env_remove("PIXIE_TIER")
        .output()
        .expect("compiled-tier run");
    assert!(compiled.status.success(), "compiled tier failed for {rel}");

    let interp = Command::new(&bin)
        .env("PIXIE_SCRIPT", script)
        .env("PIXIE_HTTP_BASE", http_fixture_base())
        .env("PIXIE_TIER", "interp")
        .output()
        .expect("interp-tier run");
    assert!(
        interp.status.success(),
        "interp tier failed for {rel}:\n{}",
        String::from_utf8_lossy(&interp.stderr)
    );

    (
        String::from_utf8_lossy(&compiled.stdout).into_owned(),
        String::from_utf8_lossy(&interp.stdout).into_owned(),
        String::from_utf8_lossy(&interp.stderr).into_owned(),
    )
}

/// The cheap half of the same rule (ledger §11.12): the two tiers keep
/// separate container-property allowlists — codegen's `lower_children`
/// and interp's `build_children` — in crates that cannot see each
/// other. This is the only place both are visible, so it is the only
/// place the tables can be compared. Fast, so it is not `#[ignore]`d:
/// the divergence it catches is a silent one.
#[test]
fn container_prop_allowlists_match_across_tiers() {
    for element in [
        "Text",
        "Button",
        "TextField",
        "Column",
        "Row",
        "Grid",
        "ListView",
        "ScrollView",
        "HScrollView",
        "Image",
        "DataTable",
        "Modal",
        "ProgressBar",
        // A leaf with its own props, consumed in its arm — both
        // tables must still say "no container keys".
        "Slider",
        // Not a widget at all — both sides must still say "no keys".
        "Nonesuch",
    ] {
        assert_eq!(
            pixie_codegen::container_prop_keys(element),
            pixie_interp::container_prop_keys(element),
            "container-property allowlists diverge for `{element}`"
        );
    }
    // And the table is not vacuous.
    assert_eq!(
        pixie_codegen::container_prop_keys("ListView"),
        ["virtualized", "itemHeight", "height", "grow"]
    );
    // ScrollView takes the viewport height and nothing else; its
    // horizontal twin clips on width, so it takes no props at all.
    assert_eq!(pixie_codegen::container_prop_keys("ScrollView"), ["height"]);
    // Grid's own table carries its track counts; the placement props a
    // grid ITEM takes live in the second, element-independent table,
    // which both tiers must also agree on.
    assert_eq!(
        pixie_codegen::container_prop_keys("Grid"),
        [
            "columns",
            "rows",
            "spacing",
            "padding",
            "background",
            "grow",
            "borderRadius",
            "borderWidth",
            "borderColor"
        ]
    );
    // §8.79: the box-decoration props ride the container tables rather
    // than a fourth universal one, because the radius has to clip the
    // BACKGROUND — only an element that paints one can take them.
    assert_eq!(
        pixie_codegen::container_prop_keys("Column"),
        [
            "spacing",
            "padding",
            "background",
            "grow",
            "borderRadius",
            "borderWidth",
            "borderColor"
        ]
    );
    assert_eq!(
        pixie_codegen::grid_item_prop_keys(),
        pixie_interp::grid_item_prop_keys(),
        "grid-item placement allowlists diverge"
    );
    assert_eq!(pixie_codegen::grid_item_prop_keys(), ["colSpan", "rowSpan"]);
    // The animation riders (§8.35) are the third universal table and
    // follow the same rule: every element takes them, the `Anim`
    // wrapper consumes them, and the two tiers must name the same set.
    assert_eq!(
        pixie_codegen::anim_prop_keys(),
        pixie_interp::anim_prop_keys(),
        "animation rider allowlists diverge"
    );
    assert_eq!(
        pixie_codegen::anim_prop_keys(),
        ["animate", "easing", "enter", "exit"]
    );
    // The accessibility riders (§8.36) — fourth universal table.
    assert_eq!(
        pixie_codegen::semantic_prop_keys(),
        pixie_interp::semantic_prop_keys(),
        "accessibility rider allowlists diverge"
    );
    assert_eq!(pixie_codegen::semantic_prop_keys(), ["role", "label"]);
    // `role:`'s vocabulary lives in the kernel; codegen keeps a copy
    // because it does not depend on the kernel (the easing table's
    // rule). This is the only place both are visible.
    let kernel_roles: Vec<&str> = pixie_kernel::a11y::Role::ALL
        .iter()
        .map(|r| r.name())
        .collect();
    assert_eq!(
        pixie_codegen::a11y_roles(),
        kernel_roles.as_slice(),
        "the `role:` vocabulary diverges between codegen and the kernel"
    );
    // The theme-scope rider (§8.37) — fifth universal table, and its
    // palette names live in the kernel for the same reason roles do.
    assert_eq!(
        pixie_codegen::theme_prop_keys(),
        pixie_interp::theme_prop_keys(),
        "theme rider allowlists diverge"
    );
    assert_eq!(pixie_codegen::theme_prop_keys(), ["theme"]);
    assert_eq!(
        pixie_codegen::theme_names(),
        pixie_kernel::theme::NAMES,
        "the `theme:` vocabulary diverges between codegen and the kernel"
    );
    assert!(pixie_codegen::container_prop_keys("HScrollView").is_empty());
}

#[test]
#[ignore = "compiles the demos; needs the shared pixie runtime (pixie install-runtime)"]
fn tiers_agree_on_every_demo() {
    // Deterministic inputs for the fs-touching scripts.
    std::fs::write("/tmp/pixie-fetch.txt", "tier gate fixture").unwrap();
    let demos = [
        (
            "examples/counter/counter.pix",
            "click:+1,click:+1,click:bump,click:add,click:save,click:load",
        ),
        (
            "examples/greeter/greeter.pix",
            "input:Alice,submit,input@1:memo,click:greet",
        ),
        // A chord is a declaration the way a timer is, and `key:`
        // presses one — so the shortcut path is checked in both tiers
        // rather than only under a human finger.
        (
            "examples/keys/keys.pix",
            "click:+1,click:+1,key:cmd-s,key:x,menu:Save,key:cmd-shift-r",
        ),
        (
            "examples/genfs/genfs.pix",
            "click:round-trip,click:weigh,click:survey,click:clean,click:who",
        ),
        // §8.73: a binding takes a LIST and an OPTIONAL. `resolved`
        // is a path joined from three components on the Rust side,
        // and `status` is an absent `T?` handed back with a fallback.
        (
            "examples/genfs/genfs.pix",
            "click:paths",
        ),
        // §8.74: a Rust enum crosses the binding both ways —
        // `pathKind` returns one and `kindName` takes it back — and
        // `case` matches it on this side. `dir: dir` is both halves.
        (
            "examples/genfs/genfs.pix",
            "click:kind",
        ),
        // §8.77: and so does a STRUCT. `survey` writes two files,
        // `stats` reads a list of `Entry` back — each holding a name,
        // an enum and another struct — sums their sizes with a list
        // of structs going the other way, and hands over one built on
        // this side.
        (
            "examples/genfs/genfs.pix",
            "click:round-trip,click:survey,click:stats,click:clean",
        ),
        // §8.78: a field says its Rust TYPE when reading it back is
        // not enough — `FileStat.len` is a `u64` — and a tuple
        // struct says its POSITION: `Perms` is a newtype pixie
        // reaches as `value` and Rust as `.0`. Both halves are built
        // on this side here, so both directions are in the reading.
        (
            "examples/genfs/genfs.pix",
            "click:perms",
        ),
        ("examples/fetch/fetch.pix", "click:fetch"),
        ("examples/progress/progress.pix", "click:step,click:step"),
        (
            "examples/scroll/scroll.pix",
            "click:fill,click:reseed,click:one,click:clear,click:fill",
        ),
        ("examples/gallery/gallery.pix", "click:+1,click:+1"),
        ("examples/icons/icons.pix", "click:bump,click:bump"),
        ("examples/table/table.pix", "click:add,click:add"),
        (
            "examples/dialog/dialog.pix",
            "click:open,input:hi,click:close",
        ),
        ("examples/layers/layers.pix", "click:bump,click:bump"),
        (
            "examples/biglist/biglist.pix",
            "click:fill,click:fill,click:clear,click:fill",
        ),
        ("examples/charts/charts.pix", "click:load,click:spike"),
        ("examples/styles/styles.pix", "click:7,click:theme"),
        ("examples/pkg/src/main.pix", "click:go"),
        ("examples/http/http.pix", "click:hit"),
        (
            "examples/basket/basket.pix",
            "click:word,click:word,click:num",
        ),
        (
            "examples/cards/cards.pix",
            "click:+1,click:+1,click:+10",
        ),
        (
            "examples/compkit/main.pix",
            "click:a,click:a,click:b",
        ),
        (
            "examples/rows/rows.pix",
            "click:seed,click:ada,click:ada,click:grace,click:more,click:alan,click:pick",
        ),
        // §8.34: the same per-row state one level deeper (a `for`
        // inside a `for`) and inside a VIRTUALIZED list. `widen` grows
        // the inner list after `a1` has been clicked twice — a
        // flattened `outer * inner_len + inner` seat index would hand
        // a1's state to another cell there, so the surviving `a1=2` is
        // the path-keyed seat being tested, not decoration.
        (
            "examples/rowsnest/rowsnest.pix",
            "click:seed,click:a1,click:a1,click:b2,click:widen,click:a3,click:fill,click:i0,click:i2,click:i0",
        ),
        (
            "examples/calc/calc.pix",
            "click:7,click:×,click:6,click:=,click:÷,click:0,click:=,click:C,click:1,click:.,click:5,click:+,click:2,click:=",
        ),
        // §8.35, four readings of one demo. The first proves animation
        // does not change what a script MEANS: no step mentions time,
        // so everything settles and the dump is what it would be with
        // no animation at all. The other three end at an `advance:`
        // and stand inside a frame — a linear tween half way (width
        // 60→200 reads 130, #3355ff→#ff5533 reads #995599), an enter
        // fade at 100/200 ms under `out` easing (0.75), and an EXIT:
        // the `if` is false, the element is gone from the built tree,
        // and it is in the dump anyway at 0.25 because the settle pass
        // retained it. That last line is the invariant this feature
        // exists to break — build no longer decides alone what paints.
        // §8.42–8.44, and the numbers ARE the assertion — `mem`
        // prints the World's live-object count, so what the ledger
        // claims about memory is checked here rather than described.
        //
        //   baseline            4  (three stores/views + the anim store)
        //   churn 5000          4  escape analysis freed every temporary
        //   reload x3         604  the old document went with its root
        //   cycle  x3        1804  a STRONG back-pointer keeps it; the
        //                          same demo with `weak` is the line above
        ("examples/reclaim/reclaim.pix", "mem,click:churn,mem"),
        (
            "examples/reclaim/reclaim.pix",
            "click:reload,mem,click:reload,click:reload,mem",
        ),
        // §8.47: an object a VIEW owns, handed to a store's list and
        // then dropped from it. `live` stays 6 across the purge and
        // `shared` still counts up afterwards — the view's fields are
        // counted edges, so a container's own reference can come and
        // go. Remove that edge table and this exact script panics
        // with "stale handle", which is why it is a reading here.
        (
            "examples/reclaim/reclaim.pix",
            "click:stash,mem,click:purge,mem,click:bump,click:bump",
        ),
        // §8.45: 401 nodes appended THROUGH the parent object, and
        // flat across rebuilds. That shape was a named error until a
        // list property got a `push` of its own.
        (
            "examples/reclaim/reclaim.pix",
            "click:tree,mem,click:tree,click:tree,mem",
        ),
        (
            "examples/reclaim/reclaim.pix",
            "click:cycle,mem,click:cycle,click:cycle,mem",
        ),
        // §8.67: `xs[i] = v`, the write twin of the trapping read.
        // `kids: 10 0 30` is the assertion — two elements written by
        // index and the untouched one still 0.
        (
            "examples/reclaim/reclaim.pix",
            "click:relabel,mem",
        ),
        // §8.60: `deinit` runs for BOTH ways an object is freed. This
        // reading counts them: 5000 escape-reclaimed temporaries plus
        // the 600 nodes the second `reload` drops = `dropped: 5600`,
        // while the live count never moves off 606. A destructor that
        // fired for only one of the two paths would make the escape
        // analysis observable.
        (
            "examples/reclaim/reclaim.pix",
            "click:churn,mem,click:reload,mem,click:reload,mem",
        ),
        // §8.54: arithmetic in an interpolation, format specs
        // (precision, width, zero fill, alignment, centring) and a
        // `static fn`. The interpreted tier has no `format!`, so it
        // applies the same grammar by hand — this reading is what
        // proves the two produce identical bytes. §8.55 rides along:
        // `30.0 * count + 30` mixes Int with Float and `fontSize: 14`
        // puts an Int in a Float slot, neither of which compiled
        // before — the readings are unchanged, which is the point.
        (
            "examples/format/format.pix",
            "click:sample,click:sample,click:sample",
        ),
        // §8.53: a handler body running the same statements a method
        // body runs. `k0..k3` proves the `while`/`if`/`break` and the
        // local counter; `a=3` twice proves an object built IN the
        // handler (2, then bumped) reaching a store. The interpreted
        // tier needs a constructor table to match, so this reading is
        // where that half is checked.
        (
            "examples/handler/handler.pix",
            "click:count,click:chip,click:chip",
        ),
        // §8.62: `return` is an early exit in a handler. `chip` while
        // locked logs and stops before building anything; after
        // `unlock` the same handler runs to the end.
        (
            "examples/handler/handler.pix",
            "click:chip,click:unlock,click:chip",
        ),
        // §8.68: the property types that used to stop at the
        // reflection table. One reading covers a map (iterated by
        // `keys`, subscripted for a `T?`), an optional printed both
        // present and absent, a byte string, struct field defaults
        // (`Row("ada")` fills two), a `T?` struct field, and a
        // repeater over a `List<Struct>` reading three fields a row.
        (
            "examples/shapes/shapes.pix",
            "click:seed,click:pick,click:read",
        ),
        (
            "examples/shapes/shapes.pix",
            "click:seed,click:pick,click:clear",
        ),
        // §8.69: `if let` in a handler, both halves.
        (
            "examples/shapes/shapes.pix",
            "click:seed,click:n,click:pick,click:n",
        ),
        // §8.70: an enum payload that is itself a `T?` — `cell [v]`
        // present, `cell []` absent, the same coercion a property and
        // a struct field get.
        (
            "examples/shapes/shapes.pix",
            "click:fill,click:d,click:clear,click:d",
        ),
        // §11.23 closed: objects refer to objects. §8.58 and §8.61
        // ride along: each `Note` carries a `let id` (init-once), a
        // `var reads` and a DERIVED `heading` that stores nothing, so
        // the row text reads every kind of class member in one line.
        // `shared weight: 3`
        // is the assertion that matters — ONE `Tag` is held by two
        // `Note`s, bumped three times through one of them, and read
        // back through the OTHER. Values cannot express that, which
        // is the reason `class` has to compose at all. `rename` then
        // writes through a chain rooted at a list element.
        (
            "examples/graph/graph.pix",
            "click:build,click:rename",
        ),
        // §8.63: `this` is the receiver. `file` walks the note list
        // calling a method on each ROW — which needed the loop
        // variable to carry its class — and each note hands `this` to
        // the shared tag, which files it on a `weak` back edge.
        (
            "examples/graph/graph.pix",
            "click:build,click:file",
        ),
        // §8.66: `touch` writes NOTHING the store owns directly — only
        // a property of an object the view reaches THROUGH it. The
        // count has to move, and before this section it did not.
        (
            "examples/graph/graph.pix",
            "click:touch,click:touch",
        ),
        // §8.37. The root palette flips (`theme:` step) while two
        // subtrees hold their own — the light panel in a dark window
        // that a process-global theme could not express. The second
        // reading stands halfway through a crossfade: the root's
        // background reads #878892, the midpoint of #1e1e2e and
        // #eff1f5, because a resolved token is just a color now and
        // §8.35 can tween it.
        // The app OWNS its theme here (`theme: App.mode` on the root),
        // so the buttons are the switcher — a `theme:` that only took
        // a literal left a view with no way to offer one.
        ("examples/themed/themed.pix", "click:light"),
        ("examples/themed/themed.pix", "click:light,advance:125"),
        // ... and the process-level root palette still underlies the
        // subtrees that do not claim one.
        ("examples/themed/themed.pix", "theme:light,click:dark"),
        // The pre-existing token user, re-read under both palettes.
        ("examples/styles/styles.pix", "theme:light,click:7"),
        // §8.36. `labels` authors what cannot be derived — an icon's
        // alt text, a heading, a named toolbar — and the second `a11y`
        // reads the field's VALUE back after typing. `dialog` authors
        // nothing at all: the derivation alone has to produce a usable
        // tree, and opening the modal has to add a `dialog` node.
        ("examples/labels/labels.pix", "a11y,input:pixie,a11y"),
        // §8.57: `role:` reads state, so the summary line stops being
        // a heading once `save` runs — the two a11y trees differ in
        // exactly that node.
        ("examples/labels/labels.pix", "a11y,click:save,a11y"),
        ("examples/dialog/dialog.pix", "a11y,click:open,a11y"),
        ("examples/anim/anim.pix", "click:step,click:wide,click:show"),
        ("examples/anim/anim.pix", "click:wide,advance:150"),
        ("examples/anim/anim.pix", "click:show,advance:100"),
        ("examples/anim/anim.pix", "click:show,click:hide,advance:100"),
        // §8.57: the curve and the fades are read from state, so
        // `ease` swaps a running animation's easing and turns the
        // enter/exit fades off. The same frame at the same instant
        // with a different curve is what proves the rider is live
        // rather than baked.
        ("examples/anim/anim.pix", "click:ease,click:wide,advance:75"),
        ("examples/anim/anim.pix", "click:ease,click:show,advance:100"),
        // And it toggles back, so the readout is a switch rather than
        // a one-way door.
        ("examples/anim/anim.pix", "click:ease,click:ease,click:wide,advance:75"),
        // The same calculator, keypad rebuilt as one `Grid` (4x5
        // tracks, `colSpan: 2` on the zero key) — the arithmetic is
        // calc's, so a divergence here is layout lowering, not math.
        (
            "examples/calcgrid/calcgrid.pix",
            "click:7,click:×,click:6,click:=,click:÷,click:0,click:=,click:C,click:1,click:.,click:5,click:+,click:2,click:=",
        ),
        // Checkbox / Switch: `click:<label>` falls through to a toggle
        // when no Button carries the label, and runs `onToggle` with
        // the NEW value — the flipped `checked=` in the dump (and the
        // Text echoing both states) is the implicit-`checked` wiring
        // being proved, not decoration.
        (
            "examples/toggles/toggles.pix",
            "click:Dark mode,click:Wi-Fi",
        ),
        // Slider: `slide:` clamps to [min, max] and snaps to the step
        // grid before running `onChange` with the implicit `value`,
        // and the echoed store prop proves the wiring — 7 and 3 land
        // exactly because both sit on the step-1 grid.
        ("examples/sliders/sliders.pix", "slide:7,slide:3"),
        // The chooser contract (Select / RadioGroup / TabBar): each
        // `select` step targets the nth chooser in tree order, finds
        // the option by exact text and runs `onSelect` with its
        // 0-based index — the Texts echo the three Ints, so the dump
        // proves the implicit `index` reached the store in both tiers.
        (
            "examples/choosers/choosers.pix",
            "select:banana,select@1:cherry,select@2:Settings",
        ),
        // The typed number fields. `input:` COMMITS on them the way
        // `enter` does in a window, and the three readings are the
        // whole contract: a number lands snapped (2.7 → 2.5 on the
        // step-0.5 grid), text that is not a number changes nothing
        // (`abc` leaves qty at 3), and out of range clamps (500 → 99).
        // `submit` is accepted on a number field and does nothing, so
        // the way a person finishes an edit reads the same in a
        // script.
        (
            "examples/numbers/numbers.pix",
            "input@0:3,input@1:2.7,submit@1,dump,input@0:abc,dump,input@0:500",
        ),
    ];
    for (rel, script) in demos {
        let (compiled, interp, interp_err) = run_both(rel, script);
        assert!(
            interp_err.contains("pixie tier: interp"),
            "{rel}: the interp tier did not engage:\n{interp_err}"
        );
        assert_eq!(
            compiled, interp,
            "tier divergence in {rel} (script `{script}`)"
        );
        assert!(!compiled.trim().is_empty(), "{rel} printed nothing");
    }
}
