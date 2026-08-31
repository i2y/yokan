# /// script
# requires-python = ">=3.14"
# ///
"""OpsBoard state: the whole data model compiles — sum-typed fleet
health, two stores, seeded mock generation. No CPython at runtime.
"""
from dataclasses import dataclass

import yokan as ui
from yokan import State, store  # noqa: E402
from yokan import random, time

BASE_MS = 1767225600000  # 2026-01-01 00:00 UTC — the mock clock's epoch


@dataclass(frozen=True)
class Healthy:
    pass


@dataclass(frozen=True)
class Degraded:
    services: int


@dataclass(frozen=True)
class Outage:
    service: str


type Health = Healthy | Degraded | Outage

health: State[Health] = State(Healthy())
mode: State[str] = State("dark")


@store
class Metrics:
    ticks: int = 0
    clock: str = "--:--"
    rps: int = 0
    err_pct: float = 0.0
    p95: int = 0
    uptime: str = "99.99%"
    rps_trend: list[float] = []
    p95_trend: list[float] = []
    svc_reqs: list[int] = []
    svc_names: list[str] = ["api", "web", "worker", "cache"]
    api_r: int = 0
    api_s: str = "ok"
    web_r: int = 0
    web_s: str = "ok"
    worker_r: int = 0
    worker_s: str = "ok"
    cache_r: int = 0
    cache_s: str = "ok"

    def reset(self) -> None:
        random.seed(3)
        self.ticks = 0
        self.clock = "--:--"
        self.rps = 0
        self.err_pct = 0.0
        self.p95 = 0
        self.rps_trend = []
        self.p95_trend = []
        self.svc_reqs = []
        self.api_r = 0
        self.web_r = 0
        self.worker_r = 0
        self.cache_r = 0
        self.api_s = "ok"
        self.web_s = "ok"
        self.worker_s = "ok"
        self.cache_s = "ok"

    def tick(self) -> None:
        self.ticks += 1
        self.clock = time.format_ms(1767225600000 + self.ticks * 60000, "%H:%M")
        self.api_r = 900 + random.int(0, 300)
        self.web_r = 600 + random.int(0, 250)
        self.worker_r = 200 + random.int(0, 120)
        self.cache_r = 1500 + random.int(0, 400)
        self.rps = self.api_r + self.web_r + self.worker_r + self.cache_r
        self.err_pct = 0.1 * random.int(1, 28)
        self.p95 = 80 + random.int(0, 220)
        self.rps_trend = self.rps_trend + [1.0 * self.rps]
        self.p95_trend = self.p95_trend + [1.0 * self.p95]
        self.svc_reqs = []
        self.svc_reqs = self.svc_reqs + [self.api_r]
        self.svc_reqs = self.svc_reqs + [self.web_r]
        self.svc_reqs = self.svc_reqs + [self.worker_r]
        self.svc_reqs = self.svc_reqs + [self.cache_r]
        if self.p95 > 260:
            self.api_s = "crit"
        elif self.p95 > 200:
            self.api_s = "warn"
        else:
            self.api_s = "ok"
        if self.err_pct > 2.0:
            self.web_s = "warn"
        else:
            self.web_s = "ok"


@store
class Alerts:
    crit_rows: list[str] = []
    warn_rows: list[str] = []
    info_rows: list[str] = []
    visible: list[str] = []
    filter: str = "all"
    crit_n: int = 0
    warn_n: int = 0
    info_n: int = 0

    def reset(self) -> None:
        self.crit_rows = []
        self.warn_rows = []
        self.info_rows = []
        self.visible = []
        self.filter = "all"
        self.crit_n = 0
        self.warn_n = 0
        self.info_n = 0

    def emit_tick(self, tick_no: int) -> None:
        stamp = time.format_ms(1767225600000 + tick_no * 60000, "%H:%M")
        roll = random.int(0, 9)
        if roll < 2:
            self.crit_rows = self.crit_rows + ["🔴 " + stamp + "  p95 breach on api — circuit breaker armed"]
            self.crit_n += 1
        elif roll < 5:
            self.warn_rows = self.warn_rows + ["🟡 " + stamp + "  error budget burn 2× on web"]
            self.warn_n += 1
        else:
            self.info_rows = self.info_rows + ["🔵 " + stamp + "  deploy worker@" + stamp + " rolled out"]
            self.info_n += 1
        self.rebuild()

    def set_filter(self, f: str) -> None:
        self.filter = f
        self.rebuild()

    def rebuild(self) -> None:
        self.visible = []
        if self.filter == "all":
            for r in self.crit_rows:
                self.visible = self.visible + [r]
            for r in self.warn_rows:
                self.visible = self.visible + [r]
            for r in self.info_rows:
                self.visible = self.visible + [r]
        elif self.filter == "crit":
            for r in self.crit_rows:
                self.visible = self.visible + [r]
        else:
            for r in self.warn_rows:
                self.visible = self.visible + [r]
