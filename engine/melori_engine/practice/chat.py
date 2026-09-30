"""Chat prompting and per-client retained chat history."""
from __future__ import annotations

import json
import os
import re
from datetime import datetime
from pathlib import Path

from melori_engine.council.language import with_language


NO_RECORDS = {"ru": "в записях этого нет", "en": "this is not in the records"}
_REF = re.compile(r"\[(\d{4}-\d{2}-\d{2})[^\]\n]*\]")


class ChatError(ValueError):
    pass


def normalize_refs(text: str) -> str:
    """The model copies the fragment header (`[2026-09-12 · note · Plan]`); the UI links `[YYYY-MM-DD]` only."""
    return _REF.sub(lambda m: f"[{m.group(1)}]", text)


def build_chat_messages(question: str, fragments: list[dict], history: list[dict], language: str) -> list[dict]:
    lang = (language or "en").lower()[:2]
    system = (
        "Answer only from the supplied client records. After each claim cite the session date exactly as "
        "[YYYY-MM-DD] — the date only, nothing else inside the brackets. If the records do not contain the answer, "
        f"say exactly: {NO_RECORDS.get(lang, NO_RECORDS['en'])}. Do not diagnose, speculate, or invent facts."
    )
    context = "\n\n".join(
        f"[{f.get('date', '')}] ({f.get('kind', '')}{' · ' + f['label'] if f.get('label') else ''}) "
        f"{f.get('text', f.get('snippet', ''))}"
        for f in fragments
    )
    messages = [{"role": "system", "content": system}]
    for turn in history[-4:]:
        if turn.get("q"):
            messages.append({"role": "user", "content": str(turn["q"])})
        if turn.get("a"):
            messages.append({"role": "assistant", "content": str(turn["a"])})
    messages.append({"role": "user", "content": f"Records:\n{context}\n\nQuestion:\n{question}"})
    return with_language(messages, language)


def answer(llm, question: str, fragments: list[dict], history: list[dict], language: str) -> tuple[str, int]:
    messages = build_chat_messages(question, fragments, history, language)
    for attempts in range(1, 4):
        last = llm.complete(messages) or ""
        if str(last).strip():
            return normalize_refs(str(last)), attempts
    # a silent model is not "nothing in the records" — say it failed
    raise ChatError("empty model response")


def _chats_dir(root, cid: str) -> Path:
    return Path(root) / cid / "chats"


def save_turn(root, cid: str, chat_n: int | None, turn: dict) -> int:
    directory = _chats_dir(root, cid)
    directory.mkdir(parents=True, exist_ok=True)
    if chat_n is None:
        n = 1
        while True:
            path = directory / f"{n:03d}.json"
            try:
                fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                os.close(fd)
                break
            except FileExistsError:
                n += 1
        value = {"n": n, "created": datetime.now().isoformat(timespec="seconds"),
                 "title": str(turn.get("q", ""))[:60], "turns": []}
    else:
        n = int(chat_n)
        path = directory / f"{n:03d}.json"
        if not path.exists():
            raise FileNotFoundError(n)
        value = json.loads(path.read_text(encoding="utf-8"))
    value.setdefault("turns", []).append(turn)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return n


def list_chats(root, cid: str) -> list[dict]:
    out = []
    for path in sorted(_chats_dir(root, cid).glob("*.json")):
        value = json.loads(path.read_text(encoding="utf-8"))
        out.append({"n": value.get("n"), "created": value.get("created"), "title": value.get("title"),
                    "turns": [{k: t.get(k) for k in ("q", "at", "refs", "provenance")} for t in value.get("turns", [])]})
    return out


def read_chat(root, cid: str, n: int) -> dict:
    path = _chats_dir(root, cid) / f"{int(n):03d}.json"
    if not path.exists():
        raise FileNotFoundError(n)
    return json.loads(path.read_text(encoding="utf-8"))


def delete_chat(root, cid: str, n: int) -> None:
    path = _chats_dir(root, cid) / f"{int(n):03d}.json"
    if not path.exists():
        raise FileNotFoundError(n)
    path.unlink()
