# /// script
# requires-python = ">=3.14"
# dependencies = ["mlx-whisper", "static-ffmpeg"]
# ///
"""Transcribe — Buzz's screen and flow, ported.

Buzz (MIT, github.com/chidiwilliams/buzz) is a desktop app that turns
recordings into text with Whisper, offline. What is ported here is
its screen and its flow: drop a file, pick a model, a language and
whether to transcribe or translate, watch it work, read the segments,
export TXT / SRT / VTT. The transcription itself is not Buzz's code
and not this app's either — it is mlx-whisper, called from a `@py`
escape. That is the point of the pair: the model stays real Python,
and everything around it — the window, the table, the timestamps, the
three exports — is compiled.

The escape runs inside a `task`, so a recording that takes a minute
leaves the window drawing, and `report(fraction, note)` from in there
moves the bar as the audio goes by.

    uv run demo/transcribe/app.py

The first run downloads the model from Hugging Face; after that it is
offline. ffmpeg reads the audio: the system's is used when there is
one, and `static-ffmpeg` fetches a copy when there is not. With no
recording to hand, this machine can speak one:

    say -v Samantha -o /tmp/hello.wav --data-format=LEI16@16000 \
        "It builds native desktop applications."
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

from yokan import (  # noqa: E402
    State,
    button,
    column,
    data_table,
    on_file_drop,
    progress,
    py,
    row,
    run,
    scroll_view,
    segmented,
    select,
    spacer,
    task,
    text,
    value,
)
from yokan import fs, strings  # noqa: E402


@value
class Seg:
    start: float
    end: float
    text: str


@py
def transcribe_file(path: str, repo: str, language: str, job: str) -> list[str]:
    """One call to mlx-whisper, with the bar wired to the window.

    `transcribe` has no progress callback: it counts audio frames
    through a tqdm bar, so the bar is what this replaces — every
    window it decodes becomes a `report`. If a future mlx-whisper
    counts differently the transcription is unaffected; only the bar
    would stop moving.
    """
    import shutil

    from yokan import report

    # The three imports a type checker cannot resolve without the
    # model stack installed. They are the escape's own dependencies,
    # declared in the block at the top of this file; `uv run` fetches
    # them, and pyright is not asked to.
    if shutil.which("ffmpeg") is None:
        import static_ffmpeg  # pyright: ignore[reportMissingImports]

        static_ffmpeg.add_paths()

    import mlx_whisper  # pyright: ignore[reportMissingImports]
    import mlx_whisper.transcribe as engine  # pyright: ignore[reportMissingImports]

    class Bar:
        def __init__(self, total=0, **kw):
            self.total = max(1, total)
            self.n = 0

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

        def update(self, k):
            self.n += k
            secs = self.n // 100
            report(min(1.0, self.n / self.total), f"{secs // 60}:{secs % 60:02d}")

    class Counter:
        tqdm = Bar

    report(0.0, "loading the model")
    engine.tqdm = Counter
    r = mlx_whisper.transcribe(
        path,
        path_or_hf_repo=repo,
        language=language or None,
        task=job,
        temperature=0.0,
        verbose=None,
    )
    return [
        f"{s['start']:.3f}\t{s['end']:.3f}\t{s['text'].strip()}"
        for s in r["segments"]
    ]


# What the chooser shows, and what Hugging Face calls it. The lists
# are held in State rather than written as module constants because
# that is what `select(options=…)` reads, and what an index into one
# can be looked up in.
models: State[list[str]] = State(["tiny", "base", "small", "medium", "large-v3"])
repos: State[list[str]] = State(
    [
        "mlx-community/whisper-tiny",
        "mlx-community/whisper-base-mlx",
        "mlx-community/whisper-small-mlx",
        "mlx-community/whisper-medium-mlx",
        "mlx-community/whisper-large-v3-mlx",
    ]
)
langs: State[list[str]] = State(
    ["detect", "English", "Japanese", "German", "French", "Spanish"]
)
codes: State[list[str]] = State(["", "en", "ja", "de", "fr", "es"])
jobs: State[list[str]] = State(["transcribe", "translate"])

path: State[str] = State("")
model_ix: State[int] = State(0)
lang_ix: State[int] = State(0)
job_ix: State[int] = State(0)
segs: State[list[Seg]] = State([])
pct: State[float] = State(0.0)
note: State[str] = State("")
busy: State[bool] = State(False)
saved: State[str] = State("")


def name_of(p: str) -> str:
    if p == "":
        return "(nothing yet)"
    cut = p.rfind("/")
    return p[cut + 1 :]


def stamp(t: float, comma: bool) -> str:
    ms = int(t * 1000)
    sep = "."
    if comma:
        sep = ","
    return f"{ms // 3600000:02d}:{(ms // 60000) % 60:02d}:{(ms // 1000) % 60:02d}{sep}{ms % 1000:03d}"


def pick_model(i: int):
    model_ix.set(i)


def pick_lang(i: int):
    lang_ix.set(i)


def pick_job(i: int):
    job_ix.set(i)


def took(p: str):
    if p != "":
        path.set(p)
        segs.set([])
        saved.set("")


def open_one():
    task(lambda: fs.open_dialog("Choose a recording"), on_done=took)


def to_seg(line: str) -> Seg:
    part: list[str] = line.split("\t")
    return Seg(
        strings.to_float(part[0], 0.0),
        strings.to_float(part[1], 0.0),
        part[2],
    )


def landed(lines: list[str]):
    segs.set([to_seg(line) for line in lines])
    busy.set(False)
    pct.set(1.0)
    note.set(f"{len(lines)} segments")


def moved(fraction: float, mark: str):
    pct.set(fraction)
    note.set(mark)


def start():
    if path() == "":
        return
    audio = path()
    repo = repos()[model_ix()]
    code = codes()[lang_ix()]
    job = jobs()[job_ix()]
    busy.set(True)
    pct.set(0.0)
    task(
        lambda: transcribe_file(audio, repo, code, job),
        on_done=landed,
        on_progress=moved,
    )


def wrote_txt(p: str):
    if p != "":
        body = ""
        for s in segs():
            body = body + s.text + "\n"
        fs.write_text(p, body)
        saved.set(f"wrote {name_of(p)}")


def wrote_srt(p: str):
    if p != "":
        body = ""
        n = 0
        for s in segs():
            n = n + 1
            head = f"{stamp(s.start, True)} --> {stamp(s.end, True)}"
            body = body + f"{n}\n{head}\n{s.text}\n\n"
        fs.write_text(p, body)
        saved.set(f"wrote {name_of(p)}")


def wrote_vtt(p: str):
    if p != "":
        body = "WEBVTT\n\n"
        for s in segs():
            head = f"{stamp(s.start, False)} --> {stamp(s.end, False)}"
            body = body + f"{head}\n{s.text}\n\n"
        fs.write_text(p, body)
        saved.set(f"wrote {name_of(p)}")


def export_txt():
    task(lambda: fs.save_dialog("transcript.txt"), on_done=wrote_txt)


def export_srt():
    task(lambda: fs.save_dialog("transcript.srt"), on_done=wrote_srt)


def export_vtt():
    task(lambda: fs.save_dialog("transcript.vtt"), on_done=wrote_vtt)


on_file_drop(took)


def view():
    with column(spacing=10, padding=14):
        text("Transcribe — drop a recording, or open one", size=13, color="#8a8f98")
        with row(spacing=8):
            button("open…", on_click=open_one)
            text(f"{name_of(path())}", size=14)
        with row(spacing=8):
            select(options=models(), selected=model_ix(), on_change=pick_model, width=110.0)
            select(options=langs(), selected=lang_ix(), on_change=pick_lang, width=130.0)
            segmented(options=jobs(), selected=job_ix(), on_change=pick_job)
            spacer()
            button("transcribe", on_click=start, disabled=busy())
        progress(pct(), label=note())
        with scroll_view(height=260.0):
            with data_table():
                # The two time columns are given a WIDTH rather than a
                # share: a share is what is left after the content, so
                # a longer timestamp in one row would move that row's
                # text and the column would stop being a column.
                with row(spacing=8):
                    text("start", width=56.0, align="right")
                    text("end", width=56.0, align="right")
                    text("text", grow=1.0)
                for s in segs():
                    with row(spacing=8):
                        text(f"{s.start:.2f}", width=56.0, align="right")
                        text(f"{s.end:.2f}", width=56.0, align="right")
                        text(s.text, grow=1.0)
        with row(spacing=8):
            text("export", size=13, color="#8a8f98")
            button("TXT", on_click=export_txt)
            button("SRT", on_click=export_srt)
            button("VTT", on_click=export_vtt)
            spacer()
            text(f"{saved()}", size=13, color="#8a8f98")


if __name__ == "__main__":
    run(view, title="transcribe", width=900, height=560)
