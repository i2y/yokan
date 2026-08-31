import yokan as ui
from state import count


def header():
    return ui.text(f"count: {count()}", size=20)


def badge(label: str):
    return ui.text(label, size=12, color="#7aa2f7")
