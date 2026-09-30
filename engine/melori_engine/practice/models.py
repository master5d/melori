# Vendored from wellbeing practice/ @ bd50668 (2026-09-27); diverges: scoped consent, delete, retain.
"""Frozen data models for clients and sessions. No behavior beyond derived properties."""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

PERMISSIONS: tuple[str, ...] = ("transcript", "video", "council", "retain")


@dataclass(frozen=True)
class Consent:
    permissions: frozenset[str]
    date: str | None
    retain_days: int | None = None
    template_version: str | None = None
    signed_sha256: str | None = None
    revoked: str | None = None

    @property
    def active(self) -> bool:
        return bool(self.permissions) and self.revoked is None


@dataclass(frozen=True)
class Client:
    id: str
    alias: str
    tags: list[str]
    consent: Consent
    created: str
    intake: str = ""
    template_id: str | None = None


@dataclass(frozen=True)
class CouncilRun:
    index: int
    timestamp: str
    mode: str
    specialist_ids: list[str]
    result: dict


@dataclass(frozen=True)
class Segment:
    i: int
    source: str
    start_ms: int
    end_ms: int
    text: str


@dataclass(frozen=True)
class NoteVersion:
    n: int
    created: str
    author: str
    template_id: str
    fields: dict[str, str]
    parent: int | None
    provenance: dict | None = None
    sections: tuple[tuple[str, str], ...] = ()
    warnings: tuple[dict, ...] = ()


@dataclass(frozen=True)
class Session:
    id: str
    client: str
    date: str
    created: str
    status: Literal["open", "closed", "unmigrated"]
    meeting_type: str
    note: str
    runs: list[CouncilRun] = field(default_factory=list)
    template_id: str | None = None
    duration_ms: int = 0
    language: str | None = None
    transcript: list[Segment] = field(default_factory=list)
    notes: list[NoteVersion] = field(default_factory=list)
    asks: list[dict] = field(default_factory=list)
    migrated: bool = False
