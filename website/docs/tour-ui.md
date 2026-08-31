# Components and style

The [tour](tour.md) continues: components with slots, named styles, themes, animation and the window.

## Components

A view fragment you want to reuse becomes a **component** (`@component`).
Per-instance state lives in `ui.local` (independent per call site, and it survives re-renders).

```python
@component
def counter(label: str, step: int):
    n: State[int] = local(0)
    with ui.row(spacing=6):
        ui.text(f"{label}: {n()}")
        ui.button(f"+{step}", on_click=lambda: n.set(n() + step))
```

A component that takes children declares `slots=True`, and the children land at `ui.slot()`.
The caller passes them with `with`.

```python
@component(slots=True)
def card(title: str):
    with ui.column(border_width=1.0, border_color="accent", padding=8):
        ui.text(title, size=18)
        ui.slot()

with card("counters"):
    counter("a", 1)
    counter("b", 10)
```

`ui.local` identity is call-site based.
Reorder the calls and the states swap along with them.

## Styles and themes

A style is a named dict, splatted onto an element with `**` (one per element).
Compose them with `|`.

```python
chip = ui.style(size=18, color="accent")
key = ui.style(background="surface", hover_background="surfaceHover")
hot = key | ui.style(background="#fab387")

ui.text(f"n={n()}", **chip)
```

Colors take hex literals or **theme tokens**.
`windowBg`, `panel`, `surface`, `surfaceHover`, `border`, `text`, `textDim`, `accent` and the rest resolve to the color the theme in effect dictates.

A theme is applied to a subtree with `theme=`.
The value can be a literal or a state read, so an app can own its palette as state.

```python
mode: State[str] = State("dark")

def flip():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")

with ui.column(background="windowBg", grow=1.0, theme=mode()):
    ...
    ui.button("theme", on_click=flip)
```

An app that themes the root of its tree follows that palette down to the window's ground color.

## Animation

Give an element `animate=` (milliseconds) and changes to that element interpolate.
`easing=` picks from `"linear"`, `"in"`, `"out"`, `"inOut"`, and `enter=True` / `exit=True` extend the animation to appearing and disappearing.

```python
ui.text("OUTAGE — api is down", animate=140, easing="out", **pill_crit)
```

## The window

The app declares its title and size in `ui.run`.

```python
ui.run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
```

`width` / `height` are logical pixels, given as a pair (omitted, the engine default applies).
The declaration is baked into the compiled binary as well.
`on_start` is a handler that runs once right after mount, and a failure prints and continues (use it for loading startup data or seeding the RNG).

