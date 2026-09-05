# Yokan task runner. Run `just` with no arguments for the list.
#
# Most common flows:
#   just gate demo/counter.py "click:+1"   # one app through both runs
#   just sweep                             # every demo, both runs
#   just dev-so                            # rebuild the importable module
#   just publish 0.1.3                     # release to PyPI and GitHub
#
# Every recipe exports CARGO_TARGET_DIR, because one shared target dir
# across the workspace and every generated app is what keeps builds
# fast — a plain `cargo` invocation without it rebuilds gpui.
#
# `just --list` prints only the LAST comment line above a recipe, so
# that line is a whole sentence naming what the recipe does; anything
# else worth saying goes above it, separated by a bare `#`.

export CARGO_TARGET_DIR := env_var('HOME') / '.cache/pixie/target'

pkg := 'crates/yokan'
wheels := env_var('HOME') / '.cache/pixie/target/wheels'

# Print this list.
default:
    @just --list

# ---- develop ---------------------------------------------------------------

# `--features extension-module` is not optional: without it the build
# links a system libpython and the module aborts at import under uv's
# CPython. The ad-hoc signature keeps macOS from killing it.
#
# Rebuild the importable extension module (crates/yokan/yokan.so).
dev-so:
    cargo build --release -p yokan --features extension-module
    cp "$CARGO_TARGET_DIR/release/libyokan.dylib" {{pkg}}/yokan.so
    codesign -f -s - {{pkg}}/yokan.so

# Prints the first refusal in file:line:col form, and nothing at all
# when the app is inside the dialect.
#
# Check one app against the dialect, without starting a compiler.
check app:
    cd {{pkg}} && uv run yokan_gate.py check {{app}}

# The fast half of the loop: no compiler, no window, about a second.
# `--frames` writes a PNG of each step's canvas.
#
# Run one app and print the screen it draws.
show app script='' frames='':
    cd {{pkg}} && uv run yokan_gate.py show {{app}} \
        {{ if script == '' { '' } else { '--script "' + script + '"' } }} \
        {{ if frames == '' { '' } else { '--frames ' + frames } }}

# Run one app headless through both runs and byte-compare the screens.
gate app script='':
    cd {{pkg}} && uv run yokan_gate.py gate {{app}} {{ if script == '' { '' } else { '--script "' + script + '"' } }}

# The gate for anything that touches the translator, the runtime or
# the standard library.
#
# Every demo, both runs.
sweep:
    cd {{pkg}} && ./tools/gate_all.sh

# Out of the sweep because it fetches a Whisper model and builds a
# bundle with mlx in it, which the other demos have no reason to wait
# for. The recording is spoken by the machine's own voice, so nothing
# has to be carried in the repository and the two runs still hear the
# same audio.
#
# Gate the transcription port, fixture and all.
transcribe-gate:
    say -v Samantha -r 165 -o /tmp/yokan-transcribe.wav --data-format=LEI16@16000 \
        "Yokan is a compiler for a statically typed subset of Python. It builds native desktop applications. Every build is checked by running the program twice and comparing the screens."
    cd {{pkg}} && env -u VIRTUAL_ENV uv run --quiet --with mlx-whisper --with static-ffmpeg \
        python3 yokan_gate.py gate demo/transcribe/app.py --bundle \
        --script "drop:/tmp/yokan-transcribe.wav,click:transcribe,file:/tmp/yokan-transcribe.srt,click:SRT,dump"

# Run this and tier-gate when a crates/pixie-* change is in the diff.
#
# The substrate's own test suites.
test:
    cargo test --workspace

# The substrate's two-tier gate over the pixie examples.
tier-gate:
    cargo test -p pixie-cli -- --ignored

# Type-check the demos against the shipped stubs (must stay at zero).
pyright:
    cd {{pkg}} && uv run --with pyright --with numpy pyright demo demo/opsboard

# Run it when a module's case set grows, or when Python moves — and
# read the diff, because a table is only true of the version that
# printed it.
#
# Print what CPython answers into crates/yokan-stdlib/tests/expected/.
expected *modules:
    cd {{pkg}} && uv run tools/gen_expected.py {{modules}}

# Read off the manifest; the tour's library section is written from it.
#
# How far each standard-library module reaches into Python's.
coverage *modules:
    cd {{pkg}} && uv run tools/stdlib_coverage.py {{modules}}

# ---- documentation site ----------------------------------------------------

# From the manifest, the translator's own tables, and a probe of every
# builtin — nothing on the page is typed by hand.
#
# Regenerate the documentation site's coverage page (both languages).
coverage-page:
    cd {{pkg}} && uv run tools/stdlib_coverage.py -o ../../website/docs/support.md
    cd {{pkg}} && uv run tools/stdlib_coverage.py --lang ja -o ../../website/docs-ja/support.md

# Both languages, in the order build.sh enforces: English first, since
# it cleans the build directory the Japanese one writes into.
#
# Build the documentation site.
site:
    cd website && ./build.sh

# The pages carry absolute links (site_url ends in /yokan/), so the
# build has to sit under that path or every link 404s — hence the
# symlinked docroot.
#
# Build the documentation site and read it at localhost:8001/yokan/.
site-serve: site
    @mkdir -p website/.serve && ln -sfn ../build website/.serve/yokan
    @echo "http://localhost:8001/yokan/  ·  http://localhost:8001/yokan/ja/"
    python3 -m http.server 8001 -d website/.serve

# ---- release ---------------------------------------------------------------

# Build the release wheel from the current version in pyproject.toml.
wheel:
    cd {{pkg}} && uvx maturin build --release

# The gate before an upload: it proves the ARTIFACT works, not the
# checkout — the difference that has bitten every release so far (a
# wheel whose kernel predates the docs).
#
# Install the newest built wheel into a throwaway venv and drive an app.
smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    whl=$(ls -t {{wheels}}/yokan-*.whl | head -1)
    tmp=$(mktemp -d)
    echo "smoking $(basename "$whl") in $tmp"
    cat > "$tmp/app.py" <<'PY'
    from yokan import State, button, column, run, text
    n: State[int] = State(0)
    def view():
        with column(spacing=8, padding=10):
            text(f"n: {n()}")
            button("+1", on_click=lambda: n.set(n() + 1))
    if __name__ == "__main__":
        run(view)
    PY
    uv venv -q -p 3.14 "$tmp/venv"
    uv pip install -q -p "$tmp/venv/bin/python" "$whl"
    out=$(cd "$tmp" && PIXIE_SCRIPT='click:+1,dump' ./venv/bin/python app.py)
    echo "$out"
    echo "$out" | grep -q 'Text(n: 1)' || { echo "smoke FAILED: the app did not react"; exit 1; }
    "$tmp/venv/bin/yokan" translate "$tmp/app.py" > /dev/null
    echo "smoke OK"

# Stops before the upload if the smoke run fails. The upload is the
# one irreversible step — PyPI never takes a version back.
#
# Release VERSION: bump, build, smoke, upload, tag, publish notes.
publish version:
    #!/usr/bin/env bash
    set -euo pipefail
    test -z "$(git status --porcelain)" || { echo "working tree is dirty"; exit 1; }
    python3 - <<PY
    import re, pathlib
    p = pathlib.Path("{{pkg}}/pyproject.toml")
    s = p.read_text()
    s2 = re.sub(r'(?m)^version = "[^"]+"', 'version = "{{version}}"', s, count=1)
    assert s2 != s, "version line not found"
    p.write_text(s2)
    PY
    just wheel
    just smoke
    read -r -p "upload yokan {{version}} to PyPI? [y/N] " ok
    [ "$ok" = "y" ] || { echo "aborted (version bump left in the tree)"; exit 1; }
    uvx twine upload {{wheels}}/yokan-{{version}}-*.whl
    git add {{pkg}}/pyproject.toml
    git commit -m "chore: release {{version}}"
    git push origin main
    git tag v{{version}}
    git push origin v{{version}}
    gh release create v{{version}} --latest --title "Yokan {{version}}" \
        --notes "See the commits since the previous tag." \
        {{wheels}}/yokan-{{version}}-*.whl
    echo "released {{version}}"
