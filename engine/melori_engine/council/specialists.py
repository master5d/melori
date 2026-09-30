# Vendored from wellbeing doctor/specialists.py @ bd50668 (2026-09-27); psych lenses only, practitioner mode, no EMR/graph.
from __future__ import annotations
from dataclasses import dataclass
from pathlib import Path
from melori_engine.council import prompts

CORPUS_CHAR_CAP = 3500
MAX_CORPUS_FILES = 8

@dataclass(frozen=True)
class Specialist:
    id: str
    name: str
    paradigm: str
    system_prompt: str
    corpus_subdir: str

    def excerpts(self, corpus_dir: Path | None) -> list[tuple[str, str]]:
        if corpus_dir is None:
            return []
        root = Path(corpus_dir).resolve()
        cdir = (root / self.corpus_subdir).resolve()
        if root not in cdir.parents:
            raise ValueError(f"corpus_subdir escapes corpus_dir: {self.corpus_subdir}")
        if not cdir.is_dir():
            return []
        out = []
        for f in sorted(cdir.glob("*.md")):
            if f.name == "README.md" or f.name.startswith("_"):
                continue
            out.append((f"{self.corpus_subdir}/{f.name}", f.read_text(encoding="utf-8").strip()[:CORPUS_CHAR_CAP]))
            if len(out) >= MAX_CORPUS_FILES:
                break
        return out

CBT = Specialist("cbt", "CBT", "Cognitive-behavioral", prompts.CBT_PERSONA, "cbt")
RO_DBT = Specialist("ro-dbt", "RO-DBT", "Radically-open DBT", prompts.RO_DBT_PERSONA, "ro-dbt")
EMDR = Specialist("emdr", "EMDR", "EMDR", prompts.EMDR_PERSONA, "emdr")
GESTALT = Specialist("gestalt", "Gestalt", "Gestalt", prompts.GESTALT_PERSONA, "gestalt")
JUNGIAN = Specialist("jungian", "Юнгианский анализ", "Jungian", prompts.JUNGIAN_PERSONA, "jungian")
TRANSPERSONAL = Specialist("transpersonal", "Трансперсональный", "Transpersonal", prompts.TRANSPERSONAL_PERSONA, "transpersonal")
ERICKSONIAN = Specialist("ericksonian", "Эриксоновская терапия", "Ericksonian", prompts.ERICKSONIAN_PERSONA, "ericksonian")
SOMATIC_TRAUMA = Specialist("somatic-trauma", "Соматик/Травма", "Somatic / trauma", prompts.SOMATIC_TRAUMA_PERSONA, "somatic-trauma")
SYNERGETIC = Specialist("synergetic", "Синергийная терапия", "Co-experiencing (Vasilyuk)", prompts.SYNERGETIC_PERSONA, "synergetic")
SYMBOLIC_IMAGINAL = Specialist("symbolic-imaginal", "Символдрама", "Symbolic-imaginal", prompts.SYMBOLIC_IMAGINAL_PERSONA, "symbolic-imaginal")
BODYNAMIC = Specialist("bodynamic", "Бодинамика", "Bodynamic (somatic developmental)", prompts.BODYNAMIC_PERSONA, "bodynamic")

PSYCH = {s.id: s for s in (CBT, RO_DBT, EMDR, GESTALT, JUNGIAN, TRANSPERSONAL, ERICKSONIAN,
                            SOMATIC_TRAUMA, SYNERGETIC, SYMBOLIC_IMAGINAL, BODYNAMIC)}

def enabled_ids() -> list[str]:
    return list(PSYCH)
