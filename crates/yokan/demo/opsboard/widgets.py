# /// script
# requires-python = ">=3.14"
# ///
"""OpsBoard widgets: named styles and reusable components — the
slotted panel, KPI cards, status pills — all compiled to native
views with pixie's Slot{} splice.

Colors are theme TOKENS (panel/border/surface/textDim/accent…) so
the whole board re-skins on a palette flip; only the status colors
are fixed hex, picked to read on both palettes.
"""
from yokan import column, component, row, slot, style, text  # noqa: E402

panel = style(background="panel", border_radius=10, border_width=1.0, border_color="border", padding=12, spacing=6, grow=1.0)
kpi_label = style(size=11, color="textDim")
kpi_value = style(size=26, color="accent")
kpi_unit = style(size=11, color="textDim")
h1 = style(size=20, color="accent")
h2 = style(size=13, color="textDim")
pill_ok = style(size=11, color="#2fa84f")
pill_warn = style(size=11, color="#d99a1f")
pill_crit = style(size=11, color="#e5484d")
row_text = style(size=12)
btn = style(background="surface", hover_background="surfaceHover")
hot = style(background="#f38ba8", hover_background="#eba0ac")
btn_hot = btn | hot


@component(slots=True)
def card(title: str):
    with column(**panel):
        text(title, **kpi_label)
        slot()


@component
def kpi(label: str, value: str, unit: str):
    with column(**panel):
        text(label, **kpi_label)
        with row(spacing=4):
            text(value, **kpi_value)
            text(unit, **kpi_unit)


@component
def pill(status: str):
    with row():
        if status == "crit":
            text("● CRIT", **pill_crit)
        elif status == "warn":
            text("● WARN", **pill_warn)
        else:
            text("● OK", **pill_ok)


@component
def svc_row(name: str, reqs: int, status: str):
    with row(spacing=10):
        text(name, size=12, grow=1.0)
        text(f"{reqs} req/m", **row_text)
        pill(status)
