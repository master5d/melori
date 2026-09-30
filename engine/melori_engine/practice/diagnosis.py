"""Small language-agnostic warning dictionary for note output."""
from __future__ import annotations
import re

_PATTERNS = (
    re.compile(r"\b(?:диагноз\w*|диагност\w*|расстройств\w*|синдром\w*|disorders?|syndromes?|diagnos\w*)\b", re.I),
    # a qualified condition ("стресс-индуцированная бессонница", "хроническая бессонница") is a formulation;
    # the bare word is often the client's own ("у меня бессонница") and stays unmarked
    re.compile(r"\b[\w-]+(?:ая|ой|ую|ый|ое)\s+бессонниц\w*", re.I),
    re.compile(r"\bдепресси\w*", re.I),
    re.compile(r"\b(?:ПТСР|PTSD|СДВГ|ADHD)\b", re.I),
    re.compile(r"\bF\d{2}(?:\.\d)?\b", re.I),
)

def find_diagnostic_phrases(fields: dict[str, str]) -> list[dict]:
    found = []
    for section, value in fields.items():
        matches = [m for pattern in _PATTERNS for m in pattern.finditer(value)]
        for match in sorted(matches, key=lambda m: m.start()):
            item = {"section": section, "phrase": match.group(0)}
            if item not in found:
                found.append(item)
    return found
