"""Versioned consent text. Old consents stay bound to the version they were signed under."""
from __future__ import annotations

from pathlib import Path

from melori_engine.practice.models import Client

CURRENT_TEMPLATE_VERSION = "v1"
_DIR = Path(__file__).with_name("templates")
_LABELS = {
    "transcript": "Транскрипт разговора (текст) / Conversation transcript",
    "video": "Видео-анализ поведения и выражения лица (числовые ряды) / Video behaviour & facial-expression analysis",
    "council": "Разбор сессии языковой моделью / Session analysis by a language model",
    "retain": "Хранение материалов после сессии / Keeping session material afterwards",
}
_ORDER = ("transcript", "video", "council", "retain")


def render_consent(client: Client, *, version: str = CURRENT_TEMPLATE_VERSION) -> str:
    path = _DIR / f"consent-{version}.md"
    if not path.exists():
        raise KeyError(version)
    perms = client.consent.permissions
    lines = "\n".join(f"- [{'x' if p in perms else ' '}] {_LABELS[p]}" for p in _ORDER)
    if "retain" not in perms:
        retention = "не сохраняется — после сессии материалы удаляются"
    elif client.consent.retain_days is None:
        retention = "до отзыва согласия"
    else:
        retention = f"{client.consent.retain_days} дней после сессии"
    return path.read_text(encoding="utf-8").format(
        alias=client.alias, date=client.consent.date or "", permission_lines=lines,
        retention_line=retention)
