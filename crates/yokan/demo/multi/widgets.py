from yokan import text
from state import count


def header():
    return text(f"count: {count()}", size=20)


def badge(label: str):
    return text(label, size=12, color="#7aa2f7")
