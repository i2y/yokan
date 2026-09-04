# Components and style

The [tour](tour.md) continues: components with slots, named styles, themes, animation and the window.

## Components

A view fragment you want to reuse becomes a **component** (`@component`).
Per-instance state lives in `local` (independent per call site, and it survives re-renders).

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with row(spacing=6):
        text(f"{label}: {n()}")
        button(f"+{step}", on_click=lambda: n.set(n() + step))
```

A component that takes children declares `slots=True`, and the children land at `slot()`.
The caller passes them with `with`.

```python
@component(slots=True)
def card(title: str):
    with column(border_width=1.0, border_color="accent", padding=8):
        text(title, size=18)
        slot()

with card("counters"):
    counter("a", 1)
    counter("b", 10)
```

A component can also take a callback or a `State` cell, which is how a child talks back to the caller.

```python
@component
def field(label: str, cell: State[str]):
    with row(spacing=6):
        text(label)
        text_field(cell(), on_change=cell.set)

field("name", name)
field("city", city)
```

A handler and a cell live in the caller, so a component that takes one becomes a view per call site — two calls that pass the same thing share one.

`local` identity is call-site based.
Reorder the calls and the states swap along with them.

## Shared properties

Every element also takes these shared properties, under the same names and with the same meaning:

- **`tooltip="…"`**: shows a line when the pointer rests there, and it is in the dump either way, so a verification script sees it.
- **`role=` / `a11y_label=`**: `role=` overrides the role an element derives (a screen reader's "button", "heading", "list" and so on), and `a11y_label=` is the name it is read by; a headless script's `a11y` step prints that tree (`demo/labels.py`). A `checkbox`, a `switch` and a `progress` are named by their own label, so they take no `a11y_label=`.
- **`disabled=True`**: dims an element and makes it inert. The window does not press it, a script step aimed at it is accepted and does nothing, and the dump shows the state.
- **`width=` / `height=` / `min_width=` / `max_width=`**: size it. An element with its own `width=` / `height=` (`button`, `image`, `svg`, `text`, the charts, `progress`) keeps those.
- **`theme=`, `animate=` / `easing=` / `enter=` / `exit=`, `col_span=` / `row_span=`**: covered under [Styles and themes](#styles-and-themes), [Animation](#animation) and `grid` respectively (`demo/shared.py`).

## Styles and themes

A style is a named dict, splatted onto an element with `**` (one per element).
Compose them with `|`.

```python
chip = style(size=18, color="accent")
key = style(background="surface", hover_background="surfaceHover")
hot = key | style(background="#fab387")

text(f"n={n()}", **chip)
```

Colors take hex literals or **theme tokens**.
`windowBg`, `panel`, `surface`, `surfaceHover`, `border`, `text`, `textDim`, `accent` and the rest resolve to the color the theme in effect dictates.

A style value can come from state as well as from a literal: `size=zoom()`, `color=Look.tone`, `padding=Look.pad * 2`.
The view re-reads it after every event, like everything else it shows.

A theme is applied to a subtree with `theme=`.
The value can be a literal or a state read, so an app can own its palette as state.

```python
mode: State[str] = State("dark")


def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")

with column(background="windowBg", grow=1.0, theme=mode()):
    ...
    button("theme", on_click=flip)
```

An app that themes the root of its tree follows that palette down to the window's ground color.

## Animation

Give an element `animate=` (milliseconds) and changes to that element interpolate.
`easing=` picks from `"linear"`, `"in"`, `"out"`, `"inOut"`, and `enter=True` / `exit=True` extend the animation to appearing and disappearing.

```python
text("OUTAGE — api is down", animate=140, easing="out", **pill_crit)
```

## The window

The app declares its title and size in `run`.

```python
run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
```

`width` / `height` are logical pixels, given as a pair (omitted, the engine default applies).
The declaration is baked into the compiled binary as well.
`on_start` is a handler that runs once right after mount, and a failure prints and continues (use it for loading startup data or seeding the RNG).
It is also the only place for startup work: module level holds declarations, and a statement there (`count.set(5)`, say, or `fs.write_text(...)`) is refused, because the compiled app reads the module and never executes it.

