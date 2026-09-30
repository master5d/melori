"""The council answers in the language of the session: detected from the source text."""
from __future__ import annotations

import re

NAMES = {"ru": "Russian", "en": "English"}
_CYRILLIC = re.compile(r"[Ѐ-ӿ]")
_LETTER = re.compile(r"[^\W\d_]")


def detect_language(text: str) -> str:
    """'ru' when at least 30% of the letters are Cyrillic, else 'en'.

    A Russian session is full of English loanwords and fillers, so a majority rule would
    misfire; 30% of letters is well above what an English text carries by accident."""
    letters = _LETTER.findall(text or "")
    if not letters:
        return "en"
    cyrillic = sum(1 for ch in letters if _CYRILLIC.match(ch))
    return "ru" if cyrillic / len(letters) >= 0.3 else "en"


def normalize(language: str | None) -> str | None:
    lang = (language or "").strip().lower()[:2]
    return lang if lang in NAMES else None


def with_language(messages: list[dict], language: str, *, synthesis: bool = False) -> list[dict]:
    """Append the answer-language instruction to the last user turn (vendored prompts stay untouched)."""
    name = NAMES.get(language, "English")
    note = f"\n\nWrite your whole answer in {name}, the language of the session."
    if synthesis:
        note += (" Keep the two section markers exactly as written, in English, each on its own line:"
                 " CONVERGENCES: and DIVERGENCES: — everything else in " + name + ".")
    out = [dict(m) for m in messages]
    for m in reversed(out):
        if m.get("role") == "user":
            m["content"] = f"{m['content']}{note}"
            return out
    out.append({"role": "user", "content": note.strip()})
    return out
