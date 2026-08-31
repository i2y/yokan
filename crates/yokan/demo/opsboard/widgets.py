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
import yokan as ui
from yokan import component  # noqa: E402

panel = ui.style(background="panel", border_radius=10, border_width=1.0, border_color="border", padding=12, spacing=6, grow=1.0)
kpi_label = ui.style(size=11, color="textDim")
kpi_value = ui.style(size=26, color="accent")
kpi_unit = ui.style(size=11, color="textDim")
h1 = ui.style(size=20, color="accent")
h2 = ui.style(size=13, color="textDim")
pill_ok = ui.style(size=11, color="#2fa84f")
pill_warn = ui.style(size=11, color="#d99a1f")
pill_crit = ui.style(size=11, color="#e5484d")
row_text = ui.style(size=12)
btn = ui.style(background="surface", hover_background="surfaceHover")
hot = ui.style(background="#f38ba8", hover_background="#eba0ac")
btn_hot = btn | hot


@component(slots=True)
def card(title: str):
    with ui.column(**panel):
        ui.text(title, **kpi_label)
        ui.slot()


@component
def kpi(label: str, value: str, unit: str):
    with ui.column(**panel):
        ui.text(label, **kpi_label)
        with ui.row(spacing=4):
            ui.text(value, **kpi_value)
            ui.text(unit, **kpi_unit)


@component
def pill(status: str):
    with ui.row():
        if status == "crit":
            ui.text("● CRIT", **pill_crit)
        elif status == "warn":
            ui.text("● WARN", **pill_warn)
        else:
            ui.text("● OK", **pill_ok)


@component
def svc_row(name: str, reqs: int, status: str):
    with ui.row(spacing=10):
        ui.text(name, size=12, grow=1.0)
        ui.text(f"{reqs} req/m", **row_text)
        pill(status)
