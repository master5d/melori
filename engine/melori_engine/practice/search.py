"""Word/vector search composition."""
from __future__ import annotations

import re
from collections import defaultdict

from melori_engine.practice import index as I


def rrf(lists: list[list[I.Hit]], k: int = 60) -> list[I.Hit]:
    scores = defaultdict(float)
    hits = {}
    for items in lists:
        for rank, hit in enumerate(items, 1):
            scores[hit.chunk_id] += 1 / (k + rank)
            hits[hit.chunk_id] = hit
    return [I.Hit(h.chunk_id, h.session_id, h.kind, h.label, h.start_ms, h.text, scores[h.chunk_id])
            for h in sorted(hits.values(), key=lambda h: scores[h.chunk_id], reverse=True)]


def snippet(text: str, q: str, width: int = 240) -> str:
    terms = {v for x in re.findall(r"[\w]+", q, re.UNICODE) for v in I.stem_variants(x)}

    def hits(word: str) -> bool:  # same rule as the index: a shared stem variant
        return not terms.isdisjoint(I.stem_variants(word))

    words = list(re.finditer(r"[\w]+", text, re.UNICODE))
    match = next((word for word in words if hits(word.group(0))), None)
    pos = match.start() if match else 0
    start = max(0, pos - width // 3)
    result = text[start:start + width]
    for word in re.finditer(r"[\w]+", result, re.UNICODE):
        if hits(word.group(0)):
            a, b = word.span()
            result = result[:a] + "[[" + result[a:b] + "]]" + result[b:]
            break
    if len(result) > width:
        result = result[:width]
    return result


def hybrid(index: I.ClientIndex, q: str, mode: str = "both", embed=None, limit: int = 20) -> dict:
    words = index.search_words(q, limit)
    meaning = []
    error = None
    available = False
    if mode in {"meaning", "both"} and embed is not None:
        try:
            vectors = embed([q])
            if vectors:
                meaning = index.search_vectors(vectors[0], limit)
                available = bool(meaning or index.path.exists())
        except Exception as exc:
            error = str(exc) or type(exc).__name__
    if mode == "words":
        results = words
    elif mode == "meaning":
        results = meaning
    else:
        results = rrf([words, meaning]) if meaning else words
    db = index._open() if index.path.exists() else None
    sessions = chunks = missing = 0
    if db is not None:
        sessions = db.execute("SELECT COUNT(*) FROM sessions").fetchone()[0]
        chunks = db.execute("SELECT COUNT(*) FROM chunks").fetchone()[0]
        missing = db.execute("SELECT COUNT(*) FROM chunks WHERE vector IS NULL").fetchone()[0]
    return {"results": [{"chunk_id": h.chunk_id, "session_id": h.session_id, "kind": h.kind,
                          "label": h.label, "start_ms": h.start_ms, "snippet": snippet(h.text, q), "score": h.score}
                         for h in results[:limit]],
            "index": {"sessions": sessions, "chunks": chunks, "vectors_missing": missing,
                      "meaning_available": available, "meaning_error": error}}
