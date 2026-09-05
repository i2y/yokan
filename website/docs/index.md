---
title: "Write Python. Ship native."
hide:
  - navigation
  - toc
---

<div class="yk-hero" markdown>
<img class="yk-hero__mark" src="images/logo.svg#only-dark" alt="">
<img class="yk-hero__mark" src="images/logo-light.svg#only-light" alt="">

# Yokan

<p class="yk-hero__tag">Write Python. Ship native.</p>

<p class="yk-hero__lede">
A compiler that takes a statically typed subset of Python to native
code. What you write is a slice of Python, and inside that slice your
code behaves exactly as Python. While you develop, the whole app runs
on real CPython; when you ship, the same source becomes a machine-code
executable. <strong>Whether the two behave the same is something you
can check</strong>, with <code>yokan gate</code>.
</p>

<div class="yk-hero__cta" markdown>
[Get started](installation.md){ .md-button .md-button--primary }
[Language tour](tour.md){ .md-button }
[Demos](demos.md){ .md-button }
[GitHub](https://github.com/i2y/yokan){ .md-button }
</div>
</div>

## The whole picture

One source, two roads to run it.
Both roads stand on the same Rust foundation — **pixie**, the
substrate language Yokan compiles through; the `.pix` in the middle
of the release road is its readable source, so you can open it and
see what your app compiled to (`yokan translate app.py` prints it).

![How Yokan runs your app: one typed source, the VM's fast loop while you develop, a native binary without the VM at release (CPython bundled only for @py), one shared foundation, one gate](images/architecture.svg#only-dark)

![How Yokan runs your app: one typed source, the VM's fast loop while you develop, a native binary without the VM at release (CPython bundled only for @py), one shared foundation, one gate](images/architecture-light.svg#only-light)

---

## What it looks like

![OpsBoard, a dashboard demo written in Yokan](images/opsboard.png)

*[`demo/opsboard`](https://github.com/i2y/yokan/tree/main/crates/yokan/demo/opsboard)
— a three-module dashboard: two stores, a sum-typed health model
matched in the view, charts, a virtualized alert feed, theme flip.
Written entirely in Python; ships as one native binary.*

---

## Write it, run it, ship it

The smallest complete app:

```python
# /// script
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text

count: State[int] = State(0)


def view():
    with column(spacing=12, padding=16):
        text(f"count: {count()}", size=34)
        button("+1", on_click=lambda: count.set(count() + 1))


if __name__ == "__main__":
    run(view, title="counter")
```

Run it with `uv run app.py` and this window opens (the renderer is
**gpui**, the engine behind the Zed editor):

![The counter app running](images/counter.png)

Edit the running app's source and save — the app updates in
place, views and handler behavior alike, with the state intact. The GIF below is that moment (editing
another small demo); note the tick counter never stops:

![Live reload: the title updates in place while the tick counter keeps counting](images/reload.gif)

Ship it:

```console
$ yokan build app.py --release
```

No `@py` escapes in the app? Then the executable contains no
CPython at all — zero links to Python, 14.7 MB (11.3 MB stripped),
millisecond startup. The person receiving it installs nothing.

---

## "But it worked on my machine"

Hand it a sequence of interactions and it replays them against the
CPython run and the machine-code build, then byte-compares the
resulting screens. Yokan calls it the **gate**:

```console
$ yokan gate app.py --script "click:+1,input:Momo"
GATE OK — 2 dump lines identical in both runs
```

Yokan's own modules — files, SQLite, HTTP, the clipboard — are one
implementation that both runs call, so there is nothing for them to
disagree about. Python's own modules (`math`, `re`, `datetime` and
the rest) are answered by CPython while you develop and by a twin
once compiled, and the gate is what holds those two together. What
Yokan cannot do yet is listed, with reasons, in
[What does not work yet](tour-ship.md#what-does-not-work-yet).

---


## When an agent is writing it

An agent writes a file and reads what comes back, so what comes back
decides how the session goes. The first two commands answer in about a
second, with no compiler and no window: a refusal that names what to
write instead, and the screen as text. The gate is the proof at the
end.

![The loop an agent works in: it writes app.py at the centre of a ring, spins through yokan check and yokan show in about a second each, and leaves the ring for yokan gate, the compile that proves the shipped binary agrees, and then for ship](images/cycle.svg#only-dark)

![The loop an agent works in: it writes app.py at the centre of a ring, spins through yokan check and yokan show in about a second each, and leaves the ring for yokan gate, the compile that proves the shipped binary agrees, and then for ship](images/cycle-light.svg#only-light)

[Building with an agent](agents.md) walks the whole loop, and
[`skills/yokan/SKILL.md`](https://github.com/i2y/yokan/blob/main/skills/yokan/SKILL.md)
is the guide to hand your agent.

---


## What else is in it

<div class="grid cards" markdown>

-   :material-language-python: __The rest of Python stays__

    Mark a function `@py` and it runs on real CPython embedded in
    the executable. numpy, pandas, your existing code — all of it.

-   :material-language-rust: __Rust crates, yours or crates.io's__

    `yokan add app.py deunicode 1` — declare a crates.io version or
    a local path and call it from your Yokan code. The crate side is
    ordinary Rust; nothing has to be written for Yokan.

-   :material-shield-check: __Typed and checked__

    Bundled stubs make pyright/Pylance check Yokan apps clean —
    `@store` singletons bind correctly, `@model`/`@value` carry
    field constructors, `Weak[Node]` reads as `Node | None`.

</div>

---

## Where next

<div class="grid cards" markdown>

-   :material-rocket-launch: __[Installation](installation.md)__

    `uv run` covers development; clone the repo for native builds.
    macOS on Apple silicon today.

-   :material-book-open-variant: __[Language tour](tour.md)__

    One pass over how apps are written — state, views, forms,
    memory, Rust crates, the gate — closing with what does not
    work yet. ([日本語版](https://i2y.github.io/yokan/ja/tour/))

-   :material-view-gallery: __[Demos](demos.md)__

    Every bundled demo, screenshotted — from the smallest
    counter to the OpsBoard dashboard.

-   :material-github: __[Source](https://github.com/i2y/yokan)__

    The compiler, the engine, and the demos.

</div>

---

_The name is the Japanese confection — yokan (羊羹), a dense solid
block made to be sliced and handed out. Which is what your app
becomes._

_"Python" is a trademark of the Python Software Foundation._
