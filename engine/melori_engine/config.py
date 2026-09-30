"""Engine settings from env. Fail-closed: no token -> no engine."""
from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from pathlib import Path


class ConfigError(RuntimeError):
    pass


def default_data_dir() -> Path:
    if sys.platform == "win32":
        return Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming")) / "melori"
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / "melori"
    return Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "melori"


@dataclass(frozen=True)
class Settings:
    token: str
    port: int
    data_dir: Path
    llm_base_url: str
    llm_model: str
    corpus_dir: Path | None
    embed_model: str = ""

    @property
    def clients_root(self) -> Path:
        return self.data_dir / "clients"

    @classmethod
    def from_env(cls) -> "Settings":
        token = os.environ.get("MELORI_ENGINE_TOKEN", "").strip()
        if len(token) < 16:
            raise ConfigError("MELORI_ENGINE_TOKEN missing or shorter than 16 chars")
        corpus = os.environ.get("MELORI_CORPUS_DIR", "").strip()
        return cls(
            token=token,
            port=int(os.environ.get("MELORI_ENGINE_PORT", "8765")),
            data_dir=Path(os.environ["MELORI_DATA_DIR"]) if os.environ.get("MELORI_DATA_DIR")
            else default_data_dir(),
            llm_base_url=os.environ.get("MELORI_LLM_BASE_URL", "http://127.0.0.1:11434/v1"),
            llm_model=os.environ.get("MELORI_LLM_MODEL", "local"),
            embed_model=os.environ.get("MELORI_EMBED_MODEL", "").strip(),
            corpus_dir=Path(corpus) if corpus else None,
        )
