# /// script
# requires-python = ">=3.14"
# ///
"""OpsBoard — a fleet-operations dashboard, fully compiled.

Three modules, two stores, a sum-typed health model matched in the
view, seeded mock telemetry, charts, a virtualized alert feed with
severity filters, slotted cards, themed styling with a live palette
flip — and the shipped artifact is one native binary with no Python
in it. Every behavior below is gate-checked against CPython.

Run:   uv run demo/opsboard/app.py
Gate:  yokan gate demo/opsboard/app.py --script "click:reset,click:tick,..."
Ship:  yokan build demo/opsboard/app.py --release
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from yokan import (
    bar_chart,
    button,
    column,
    fs,
    line_chart,
    list_view,
    row,
    run,
    text,
)

from state import Alerts, Degraded, Healthy, Metrics, Outage, health, mode  # noqa: E402
from widgets import btn, btn_hot, card, h1, h2, kpi, panel, pill_crit, pill_ok, pill_warn, svc_row  # noqa: E402


def reset():
    Metrics.reset()
    Alerts.reset()
    health.set(Healthy())


def tick():
    Metrics.tick()
    Alerts.emit_tick(Metrics.ticks)
    if Alerts.crit_n > 1:
        health.set(Outage("api"))
    elif Alerts.crit_n > 0:
        health.set(Degraded(1))
    else:
        health.set(Healthy())


def boot():
    """Open onto history, not zeros: reset then replay six minutes
    of telemetry — deterministic, so both tiers boot identically."""
    Metrics.reset()
    Alerts.reset()
    for i in range(6):
        Metrics.tick()
        Alerts.emit_tick(Metrics.ticks)
    if Alerts.crit_n > 1:
        health.set(Outage("api"))
    elif Alerts.crit_n > 0:
        health.set(Degraded(1))
    else:
        health.set(Healthy())


def export():
    fs.write_text(
        "demo/.gate/opsboard-report.txt",
        f"OpsBoard report @ {Metrics.clock} — rps={Metrics.rps} p95={Metrics.p95}ms alerts={Alerts.crit_n}/{Alerts.warn_n}/{Alerts.info_n}",
    )


def flip_theme():
    if mode() == "dark":
        mode.set("light")
    else:
        mode.set("dark")


def alert_row(i):
    return text(Alerts.visible[i], size=12)


def view():
    with column(spacing=10, padding=16, background="windowBg", grow=1.0, theme=mode()):
        # ── header ────────────────────────────────────────────────
        with row(spacing=10):
            text("⬢ OpsBoard", **h1)
            text("fleet telemetry · mock feed", **h2)
            text(f"synced {Metrics.clock}", size=12, color="textDim", grow=1.0, align="right")
            button("◐ theme", on_click=flip_theme, **btn)
        # ── system health (sum type, matched live) ───────────────
        with row(spacing=8):
            match health():
                case Healthy():
                    text("ALL SYSTEMS NOMINAL", animate=140, easing="out", **pill_ok)
                case Degraded(services):
                    text(f"DEGRADED — {services} service(s) impacted", animate=140, easing="out", **pill_warn)
                case Outage(service):
                    text(f"OUTAGE — {service} is down", animate=140, easing="out", **pill_crit)
            text(f"tick #{Metrics.ticks}", size=11, color="textDim", grow=1.0, align="right")
        # ── KPI row ──────────────────────────────────────────────
        with row(spacing=10):
            kpi("REQUESTS", f"{Metrics.rps}", "req/m")
            kpi("ERROR RATE", f"{Metrics.err_pct:.1f}", "%")
            kpi("P95 LATENCY", f"{Metrics.p95}", "ms")
            kpi("UPTIME 30D", f"{Metrics.uptime}", "SLO 99.9")
        # ── charts ───────────────────────────────────────────────
        with row(spacing=10):
            with card("THROUGHPUT — req/m per tick"):
                line_chart(Metrics.rps_trend, height=110.0)
            with card("P95 LATENCY — ms per tick"):
                line_chart(Metrics.p95_trend, height=110.0)
        with row(spacing=10):
            with card("LOAD BY SERVICE"):
                bar_chart(Metrics.svc_reqs, labels=Metrics.svc_names, height=100.0)
            with card("FLEET"):
                svc_row("api-gateway", Metrics.api_r, Metrics.api_s)
                svc_row("web-frontend", Metrics.web_r, Metrics.web_s)
                svc_row("worker-pool", Metrics.worker_r, Metrics.worker_s)
                svc_row("cache-layer", Metrics.cache_r, Metrics.cache_s)
        # ── alert feed ───────────────────────────────────────────
        with card(f"ALERTS — {Alerts.crit_n} crit · {Alerts.warn_n} warn · {Alerts.info_n} info"):
            with row(spacing=6):
                # the highlight follows the ACTIVE filter
                if Alerts.filter == "all":
                    button("all", on_click=lambda: Alerts.set_filter("all"), **btn_hot)
                else:
                    button("all", on_click=lambda: Alerts.set_filter("all"), **btn)
                if Alerts.filter == "crit":
                    button("crit", on_click=lambda: Alerts.set_filter("crit"), **btn_hot)
                else:
                    button("crit", on_click=lambda: Alerts.set_filter("crit"), **btn)
                if Alerts.filter == "warn":
                    button("warn", on_click=lambda: Alerts.set_filter("warn"), **btn_hot)
                else:
                    button("warn", on_click=lambda: Alerts.set_filter("warn"), **btn)
            list_view(len(Alerts.visible), alert_row, item_height=22.0, grow=1.0)
        # ── footer ───────────────────────────────────────────────
        with row(spacing=8):
            button("▶ tick", on_click=tick, **btn)
            button("reset", on_click=reset, **btn)
            button("export report", on_click=export, **btn)
            text("yokan · compiled dashboard · zero python at runtime", size=10, color="textDim", grow=1.0, align="right")


if __name__ == "__main__":
    run(view, title="OpsBoard", width=1100, height=820, on_start=boot)
