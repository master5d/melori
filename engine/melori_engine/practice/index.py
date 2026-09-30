"""Per-client SQLite/FTS index."""
from __future__ import annotations

import hashlib
import math
import re
import sqlite3
import struct
from dataclasses import dataclass
from pathlib import Path

from melori_engine.practice import chunks as chunking, sessions, store

_TOKEN = re.compile(r"[\w]+", re.UNICODE)
_RU_ENDINGS = ("иями", "ями", "ами", "ого", "его", "ому", "ему", "ыми", "ими", "ой", "ей", "ий", "ый", "ая", "яя", "ое", "ее", "ую", "юю", "ом", "ем", "ах", "ях", "ы", "и", "а", "я", "о", "е", "у", "ю", "ь")
_EN_ENDINGS = ("ing", "ed", "es", "s")


def stem(word: str) -> str:
    word = word.lower().replace("ё", "е")
    if not word:
        return ""
    endings = _EN_ENDINGS if re.fullmatch(r"[a-z]+", word) else _RU_ENDINGS
    for ending in endings:
        minimum = 2 if endings is _RU_ENDINGS else 3
        if word.endswith(ending) and len(word) - len(ending) >= minimum:
            word = word[:-len(ending)]
            break
    return word


_CONSONANT = "бвгджзйклмнпрстфхцчшщ"
_FLEETING = re.compile(rf"^(.*[{_CONSONANT}])[ое]([{_CONSONANT}])$")


def stem_variants(word: str) -> list[str]:
    """The stem plus, for a stem ending consonant+о/е+consonant, the same stem without that vowel:
    Russian drops it in other forms (сон/сна, день/дня, отец/отца), so both sides of the index carry it."""
    s = stem(word)
    if not s:
        return []
    m = _FLEETING.match(s)
    return [s, m.group(1) + m.group(2)] if m else [s]


def stem_text(text: str) -> str:
    return " ".join(v for token in _TOKEN.findall(text) for v in stem_variants(token))


def fts_query(q: str) -> str:
    terms = dict.fromkeys(v.replace(chr(34), "") for token in _TOKEN.findall(q) for v in stem_variants(token))
    return " OR ".join(f'"{t}"' for t in terms if t)


@dataclass(frozen=True)
class Hit:
    chunk_id: int
    session_id: str
    kind: str
    label: str
    start_ms: int | None
    text: str
    score: float


@dataclass(frozen=True)
class RefreshReport:
    sessions: int
    chunks: int
    rebuilt: list[str]
    dropped: list[str]
    vectors_missing: int
    meaning_error: str | None


def _pack(values: list[float]) -> bytes:
    return struct.pack(f"<{len(values)}f", *values)


def _unpack(value: bytes) -> list[float]:
    return list(struct.unpack(f"<{len(value) // 4}f", value))


class ClientIndex:
    def __init__(self, root, client_id: str):
        self.root = Path(root)
        self.client_id = client_id
        self.client_root = self.root / client_id
        self.path = self.client_root / "index.sqlite"
        self._db: sqlite3.Connection | None = None

    def _allowed(self) -> bool:
        client = store.get_client(self.root, self.client_id)
        return client is not None and "retain" in client.consent.permissions and client.consent.revoked is None

    def _open(self) -> sqlite3.Connection:
        if self._db is None:
            self.client_root.mkdir(parents=True, exist_ok=True)
            self._db = sqlite3.connect(self.path)
            self._db.row_factory = sqlite3.Row
            self._db.executescript("""
                CREATE TABLE IF NOT EXISTS chunks (id INTEGER PRIMARY KEY, session_id TEXT NOT NULL,
                  kind TEXT NOT NULL CHECK(kind IN ('transcript','note','email')), label TEXT NOT NULL,
                  start_ms INTEGER NULL, text TEXT NOT NULL, stems TEXT NOT NULL, vector BLOB NULL);
                CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(stems, content='chunks', content_rowid='id');
                CREATE TABLE IF NOT EXISTS sessions (session_id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            """)
            self._db.commit()
        return self._db

    def _fingerprint(self, directory: Path) -> str:
        files = [directory / "transcript.jsonl", *sorted((directory / "notes").glob("*.json")), directory / "email.json"]
        parts = []
        for path in files:
            if path.exists():
                stat = path.stat()
                parts.append(f"{path.name}:{stat.st_size}:{stat.st_mtime_ns}")
        return hashlib.sha1("|".join(parts).encode()).hexdigest()

    def refresh(self, embed=None, embed_model: str = "", progress=None) -> RefreshReport:
        if not self._allowed():
            return RefreshReport(0, 0, [], [], 0, None)
        db = self._open()
        current = {s.id: s for s in store.list_sessions(self.root, self.client_id) if s.status != "unmigrated"}
        old = {r["session_id"]: r["fingerprint"] for r in db.execute("SELECT session_id, fingerprint FROM sessions")}
        rebuilt, dropped = [], []
        for sid in sorted(set(old) - set(current)):
            self.drop_session(sid)
            dropped.append(sid)
        for sid, session in current.items():
            directory = sessions.session_dir(self.root, self.client_id, sid)
            fingerprint = self._fingerprint(directory)
            if old.get(sid) == fingerprint:
                continue
            self._delete_rows(db, sid)
            db.execute("DELETE FROM sessions WHERE session_id=?", (sid,))
            for item in chunking.chunks_for_session(directory):
                stems = stem_text(item.text)
                cursor = db.execute("INSERT INTO chunks(session_id,kind,label,start_ms,text,stems) VALUES(?,?,?,?,?,?)",
                                    (sid, item.kind, item.label, item.start_ms, item.text, stems))
                db.execute("INSERT INTO chunks_fts(rowid, stems) VALUES(?,?)", (cursor.lastrowid, stems))
            db.execute("INSERT INTO sessions VALUES(?,?)", (sid, fingerprint))
            rebuilt.append(sid)
            if progress:
                progress(len(rebuilt), len(current))
        db.execute("INSERT OR REPLACE INTO meta VALUES('schema','1')")
        previous_model = db.execute("SELECT value FROM meta WHERE key='embed_model'").fetchone()
        if previous_model is None or previous_model[0] != embed_model:
            db.execute("UPDATE chunks SET vector=NULL")
            db.execute("INSERT OR REPLACE INTO meta VALUES('embed_model',?)", (embed_model,))
            db.execute("DELETE FROM meta WHERE key='embed_dim'")
        error = None
        if embed is not None:
            rows = db.execute("SELECT id,text FROM chunks WHERE vector IS NULL ORDER BY id").fetchall()
            for start in range(0, len(rows), 32):
                batch = rows[start:start + 32]
                try:
                    vectors = embed([r["text"] for r in batch])
                    if len(vectors) != len(batch) or (vectors and not all(isinstance(x, (list, tuple)) for x in vectors)):
                        raise ValueError("invalid embeddings response")
                    dim = len(vectors[0]) if vectors else 0
                    if any(len(v) != dim for v in vectors):
                        raise ValueError("inconsistent embedding dimensions")
                    for row, vector in zip(batch, vectors):
                        db.execute("UPDATE chunks SET vector=? WHERE id=?", (_pack([float(x) for x in vector]), row["id"]))
                    if dim:
                        db.execute("INSERT OR REPLACE INTO meta VALUES('embed_dim',?)", (str(dim),))
                except Exception as exc:
                    error = str(exc) or type(exc).__name__
                    break
        db.commit()
        count = db.execute("SELECT COUNT(*) FROM chunks").fetchone()[0]
        missing = db.execute("SELECT COUNT(*) FROM chunks WHERE vector IS NULL").fetchone()[0]
        return RefreshReport(len(current), count, rebuilt, dropped, missing, error)

    def search_words(self, q: str, limit: int = 20) -> list[Hit]:
        if not self._allowed() or not self.path.exists():
            return []
        query = fts_query(q)
        if not query:
            return []
        db = self._open()
        rows = db.execute("SELECT c.*, bm25(chunks_fts) AS score FROM chunks_fts JOIN chunks c ON c.id=chunks_fts.rowid WHERE chunks_fts MATCH ? ORDER BY score LIMIT ?", (query, limit)).fetchall()
        return [Hit(r["id"], r["session_id"], r["kind"], r["label"], r["start_ms"], r["text"], float(r["score"])) for r in rows]

    def search_vectors(self, qvec: list[float], limit: int = 20) -> list[Hit]:
        if not self._allowed() or not self.path.exists():
            return []
        db = self._open()
        qnorm = math.sqrt(sum(x * x for x in qvec))
        scored = []
        for r in db.execute("SELECT * FROM chunks WHERE vector IS NOT NULL"):
            vec = _unpack(r["vector"])
            norm = math.sqrt(sum(x * x for x in vec))
            score = sum(a * b for a, b in zip(qvec, vec)) / (qnorm * norm) if qnorm and norm else 0.0
            scored.append((score, r))
        scored.sort(key=lambda x: x[0], reverse=True)
        return [Hit(r["id"], r["session_id"], r["kind"], r["label"], r["start_ms"], r["text"], score) for score, r in scored[:limit]]

    def drop_session(self, sid: str, keep_record: bool = False) -> None:
        if not self.path.exists():
            return
        db = self._open()
        self._delete_rows(db, sid)
        if not keep_record:
            db.execute("DELETE FROM sessions WHERE session_id=?", (sid,))
        db.commit()

    @staticmethod
    def _delete_rows(db: sqlite3.Connection, sid: str) -> None:
        ids = [row[0] for row in db.execute("SELECT id FROM chunks WHERE session_id=?", (sid,))]
        for chunk_id in ids:
            db.execute("DELETE FROM chunks_fts WHERE rowid=?", (chunk_id,))
        db.execute("DELETE FROM chunks WHERE session_id=?", (sid,))

    def close(self):
        if self._db is not None:
            self._db.close()
            self._db = None
