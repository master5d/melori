# Vendored from wellbeing doctor/psychcouncil.py @ bd50668 (2026-09-27); psych lenses only, practitioner mode, no EMR/graph.
from __future__ import annotations
import re
from dataclasses import dataclass
from pathlib import Path
from melori_engine.council import prompts
from melori_engine.council import specialists as _sp
from melori_engine.council.language import detect_language, normalize, with_language
from melori_engine.llm import LLMUnavailable

VALID_MODES = ("practitioner",)

class PsychCouncilError(ValueError):
    pass

@dataclass(frozen=True)
class PsychOpinion:
    specialist_id: str
    name: str
    paradigm: str
    text: str
    citations: list[str]

@dataclass(frozen=True)
class PsychSynthesis:
    text: str
    convergences: list[str]
    divergences: list[str]

_CONV_WORDS = ("convergence", "совпаден", "сходств", "конвергенц")
_DIV_WORDS = ("divergence", "расхожден", "различ", "дивергенц", "разноглас")
_BULLET = re.compile(r"^\s*(?:[-*•]|\d+[.)])\s+")


def _heading_kind(line: str) -> str | None:
    """'conv' / 'div' when the line is a section heading (English marker, a Russian name,
    markdown decoration, trailing colon), else None."""
    bare = re.sub(r"[#*_`>]+", "", line).strip().rstrip(":").strip().lower()
    if not bare or len(bare) > 40:
        return None
    if any(bare.startswith(w) for w in _CONV_WORDS):
        return "conv"
    if any(bare.startswith(w) for w in _DIV_WORDS):
        return "div"
    return None


def _parse_synthesis(text: str) -> tuple[list[str], list[str]]:
    conv: list[str] = []
    div: list[str] = []
    bucket: list[str] | None = None
    for line in text.splitlines():
        kind = _heading_kind(line)
        if kind == "conv":
            bucket = conv
            continue
        if kind == "div":
            bucket = div
            continue
        if bucket is not None and _BULLET.match(line):
            item = _BULLET.sub("", line).strip().strip("*_ ").strip()
            if item:
                bucket.append(item)
    return conv, div

def preflight(situation, specialist_ids, mode):
    if not situation or not situation.strip():
        raise PsychCouncilError("situation must be non-empty")
    if mode not in VALID_MODES:
        raise PsychCouncilError(f"mode must be one of {VALID_MODES}")
    ids = specialist_ids or _sp.enabled_ids()
    specs = [s for s in (_sp.PSYCH.get(i) for i in ids) if s is not None]
    if not specs:
        raise PsychCouncilError("no psych specialists selected")
    return specs

# Lenses run side by side on the practitioner's LLM. Measured on the sovereign floor
# (local-floor, 2026-09-28; 11 lens-sized calls): 1 -> 53.7 s, 2 -> 40.7, 3 -> 35.0,
# 4 -> 32.0, 6 -> 28.4, 8 -> 27.5 s, no errors. The knee is 4 (6 is 11% faster but each
# lens waits 36% longer; 8 gains nothing). Override: MELORI_COUNCIL_CONCURRENCY.
MAX_CONCURRENCY = 6
DEFAULT_CONCURRENCY = 4


def clamp_concurrency(value: int) -> int:
    return max(1, min(int(value), MAX_CONCURRENCY))


def ordered_opinions(ids: list[str], opinions: list[PsychOpinion]) -> list[PsychOpinion]:
    """Opinions in the selected lens order, whatever order they finished in."""
    rank = {sid: i for i, sid in enumerate(ids)}
    return sorted(opinions, key=lambda o: rank.get(o.specialist_id, len(rank)))


def _one_lens(spec, situation, llm, corpus_dir, mode, language="en") -> PsychOpinion:
    try:
        excerpts = spec.excerpts(corpus_dir)
        text = llm.complete(with_language(
            prompts.psych_opinion_messages(situation, [], excerpts, spec.system_prompt, mode), language))
        return PsychOpinion(spec.id, spec.name, spec.paradigm, text, [p for p, _ in excerpts])
    except LLMUnavailable:
        raise
    except Exception:
        return PsychOpinion(spec.id, spec.name, spec.paradigm, "(this lens is temporarily unavailable)", [])


def psych_council_stream(situation: str, specialist_ids: list[str] | None, *, llm,
                         corpus_dir: Path | None, mode: str = "practitioner",
                         concurrency: int = 1, language: str | None = None):
    """Yield (kind, payload): plan, lens_start, opinion, lens_done (per lens, as they happen),
    synthesis_start, synthesis. Stage frames are dicts; opinion/synthesis are dataclasses.
    An unavailable LLM cancels the lenses not yet started and re-raises."""
    import queue
    import threading
    import time
    from concurrent.futures import ThreadPoolExecutor

    specs = preflight(situation, specialist_ids, mode)
    ids = [s.id for s in specs]
    limit = clamp_concurrency(concurrency)
    # the session's language: given by the caller (detected from the transcript), else
    # detected from the text the council reads
    lang = normalize(language) or detect_language(situation)
    yield "plan", {"total": len(specs), "concurrency": limit, "specialists": ids, "language": lang}

    events: "queue.Queue[tuple[str, object]]" = queue.Queue()
    stop = threading.Event()

    def run(index: int, spec):
        if stop.is_set():
            events.put(("skipped", spec.id))
            return
        events.put(("lens_start", {"specialist_id": spec.id, "name": spec.name, "index": index}))
        started = time.monotonic()
        try:
            opinion = _one_lens(spec, situation, llm, corpus_dir, mode, lang)
        except LLMUnavailable as exc:
            stop.set()
            events.put(("failed", exc))
            return
        events.put(("opinion", opinion))
        events.put(("lens_done", {"specialist_id": spec.id,
                                  "elapsed_s": round(time.monotonic() - started, 1)}))

    opinions: list[PsychOpinion] = []
    failure: LLMUnavailable | None = None
    with ThreadPoolExecutor(max_workers=limit) as pool:
        for i, spec in enumerate(specs):
            pool.submit(run, i + 1, spec)
        finished = 0
        while finished < len(specs):
            kind, payload = events.get()
            if kind == "failed":
                failure = failure or payload
                finished += 1
            elif kind == "skipped":
                finished += 1
            elif kind == "opinion":
                opinions.append(payload)
                yield kind, payload
            elif kind == "lens_done":
                finished += 1
                yield kind, payload
            else:
                yield kind, payload
    if failure is not None:
        raise failure

    opinions = ordered_opinions(ids, opinions)
    yield "synthesis_start", {"opinions": len(opinions)}
    syn_text = llm.complete(with_language(prompts.psych_synthesis_messages(
        situation, [(o.name, o.paradigm, o.text) for o in opinions], mode), lang, synthesis=True))
    conv, div = _parse_synthesis(syn_text)
    yield "synthesis", PsychSynthesis(syn_text, conv, div)
