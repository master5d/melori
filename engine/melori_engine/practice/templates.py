"""Note templates and their on-disk registry."""
from __future__ import annotations

import json
import re
import secrets
from dataclasses import dataclass, asdict
from pathlib import Path


@dataclass(frozen=True)
class Section:
    key: str
    title: str
    guidance: str


@dataclass(frozen=True)
class Template:
    id: str
    name: str
    builtin: bool
    sections: tuple[Section, ...]


class TemplateError(ValueError):
    pass


class TemplateInUse(TemplateError):
    def __init__(self, n: int):
        self.n = n
        super().__init__(f"template is used by {n} client(s)")


_LOCALIZED = {
    "soap": [("subjective", "Субъективно", "Subjective", "что клиент рассказал о себе, своими словами", "what the client told about themselves, in their own words"),
             ("objective", "Объективно", "Objective", "что наблюдалось в сессии (поведение, речь), без интерпретаций", "what was observed in the session (behavior, speech), without interpretations"),
             ("assessment", "Оценка", "Assessment", "рабочая формулировка практика по сказанному, без диагнозов", "the practitioner's working formulation from what was said, without diagnoses"),
             ("plan", "План", "Plan", "договорённости и следующие шаги", "agreements and next steps")],
    "dap": [("data", "Данные", "Data", "наблюдения и факты сессии", "session observations and facts"),
            ("assessment", "Оценка", "Assessment", "рабочая формулировка практика по сказанному, без диагнозов", "the practitioner's working formulation from what was said, without diagnoses"),
            ("plan", "План", "Plan", "договорённости и следующие шаги", "agreements and next steps")],
    "grow": [("goal", "Цель", "Goal", "чего клиент хочет достичь", "what the client wants to achieve"),
             ("reality", "Реальность", "Reality", "что происходит сейчас", "what is happening now"),
             ("options", "Варианты", "Options", "возможные пути и решения", "possible paths and solutions"),
             ("will", "Воля", "Will", "выбранные действия и обязательства", "chosen actions and commitments")],
    "free": [("text", "Заметка", "Note", "", "")],
}


def _make_builtin(template_id: str, language: str) -> Template:
    ru = language == "ru"
    return Template(template_id, template_id.upper() if template_id != "free" else ("Заметка" if ru else "Note"), True,
                    tuple(Section(row[0], row[1 if ru else 2], row[3 if ru else 4]) for row in _LOCALIZED[template_id]))


BUILTIN: dict[str, Template] = {key: _make_builtin(key, "en") for key in _LOCALIZED}


def builtin_localized(t: Template, language: str) -> Template:
    return _make_builtin(t.id, language) if t.id in BUILTIN else t


def validate(name: str, sections: list[dict]) -> tuple[Section, ...]:
    if not isinstance(name, str) or not 1 <= len(name.strip()) <= 60:
        raise TemplateError("name must be 1-60 characters")
    if not 1 <= len(sections) <= 12:
        raise TemplateError("sections must contain 1-12 items")
    out, seen = [], set()
    for i, item in enumerate(sections):
        if not isinstance(item, dict):
            raise TemplateError(f"section {i}: must be an object")
        key, title, guidance = item.get("key"), item.get("title"), item.get("guidance", "")
        if not isinstance(key, str) or not re.fullmatch(r"[a-z][a-z0-9_]{0,31}", key):
            raise TemplateError(f"section {i}: invalid key")
        if key in seen:
            raise TemplateError(f"section {i}: duplicate key")
        if not isinstance(title, str) or not 1 <= len(title.strip()) <= 60:
            raise TemplateError(f"section {i}: title must be 1-60 characters")
        if not isinstance(guidance, str) or len(guidance) > 400:
            raise TemplateError(f"section {i}: guidance must be at most 400 characters")
        seen.add(key)
        out.append(Section(key, title, guidance))
    return tuple(out)


def _template_dir(root) -> Path:
    d = Path(root) / "templates"
    d.mkdir(parents=True, exist_ok=True)
    return d


def _read(path: Path) -> Template:
    raw = json.loads(path.read_text(encoding="utf-8"))
    return Template(str(raw["id"]), str(raw["name"]), False, validate(raw["name"], raw["sections"]))


def list_templates(root) -> list[Template]:
    out = [BUILTIN[k] for k in ("soap", "dap", "grow")]
    d = Path(root) / "templates"
    if d.is_dir():
        out.extend(_read(p) for p in sorted(d.glob("*.json")))
    return out


_CUSTOM_ID = re.compile(r"t-[0-9a-f]{8}")


def get_template(root, template_id: str) -> Template | None:
    if template_id in BUILTIN:
        return BUILTIN[template_id]
    if not isinstance(template_id, str) or not _CUSTOM_ID.fullmatch(template_id):
        return None
    p = Path(root) / "templates" / f"{template_id}.json"
    return _read(p) if p.is_file() else None


def _write(root, t: Template) -> None:
    from . import store
    payload = {"id": t.id, "name": t.name, "builtin": False, "sections": [asdict(s) for s in t.sections]}
    store._atomic_write(_template_dir(root) / f"{t.id}.json", json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def create_template(root, name: str, sections: list[dict]) -> Template:
    t = Template("t-" + secrets.token_hex(4), name.strip(), False, validate(name, sections))
    _write(root, t)
    return t


def update_template(root, id: str, name: str, sections: list[dict]) -> Template:
    if id in BUILTIN:
        raise TemplateError("builtin")
    if get_template(root, id) is None:
        raise TemplateError("not found")
    t = Template(id, name.strip(), False, validate(name, sections))
    _write(root, t)
    return t


def delete_template(root, id: str, *, clients_root, force: bool = False) -> int:
    if id in BUILTIN:
        raise TemplateError("builtin")
    if get_template(root, id) is None:
        raise TemplateError("not found")
    path = Path(root) / "templates" / f"{id}.json"
    from . import store
    clients = [c for c in store.list_clients(clients_root) if c.template_id == id]
    if clients and not force:
        raise TemplateInUse(len(clients))
    for c in clients:
        store.set_client_template(clients_root, c.id, "soap")
    path.unlink()
    return len(clients)
