import pytest

from melori_engine.council import engine as E
from melori_engine.council import specialists as S


class FakeLLM:
    def __init__(self):
        self.n = 0

    def complete(self, messages):
        self.n += 1
        return "CONVERGENCES:\n- shared\nDIVERGENCES:\n- split"


def test_neurodivergent_coach_not_vendored():
    assert all("neuro" not in sid for sid in S.PSYCH)


def test_at_least_ten_psych_lenses():
    assert len(S.enabled_ids()) >= 10


RESULT_KINDS = {"opinion", "synthesis"}


def test_stream_yields_opinions_then_synthesis_without_corpus():
    # contract since 2026-09-28: stage frames (plan / lens_start / lens_done / synthesis_start)
    # are interleaved; the result frames keep their order
    llm = FakeLLM()
    events = list(E.psych_council_stream("client worries about work", ["cbt", "gestalt"], llm=llm, corpus_dir=None))
    assert [k for k, _ in events if k in RESULT_KINDS] == ["opinion", "opinion", "synthesis"]
    assert events[-1][1].convergences == ["shared"]
    assert llm.n == 3


def test_stage_frames_describe_progress():
    events = list(E.psych_council_stream("x", ["cbt", "gestalt", "emdr"], llm=FakeLLM(), corpus_dir=None, concurrency=2))
    kinds = [k for k, _ in events]
    assert kinds[0] == "plan"
    assert {k: v for k, v in events[0][1].items() if k != "language"} == {
        "total": 3, "concurrency": 2, "specialists": ["cbt", "gestalt", "emdr"]}
    assert kinds.count("lens_start") == 3 and kinds.count("lens_done") == 3
    assert kinds.index("synthesis_start") > max(i for i, k in enumerate(kinds) if k == "lens_done")
    assert kinds[-1] == "synthesis"
    starts = [d for k, d in events if k == "lens_start"]
    assert {d["specialist_id"] for d in starts} == {"cbt", "gestalt", "emdr"}
    assert all(d["elapsed_s"] >= 0 for k, d in events if k == "lens_done")


class SlowLLM:
    """Records how many calls overlap."""
    def __init__(self, delay=0.25):
        import threading
        self.delay, self.active, self.peak = delay, 0, 0
        self.lock = threading.Lock()

    def complete(self, messages):
        import time
        with self.lock:
            self.active += 1
            self.peak = max(self.peak, self.active)
        time.sleep(self.delay)
        with self.lock:
            self.active -= 1
        return "CONVERGENCES:\n- a\nDIVERGENCES:\n- b"


def test_lenses_run_in_parallel_up_to_the_limit():
    import time
    ids = ["cbt", "gestalt", "emdr", "jungian"]
    llm = SlowLLM()
    t = time.time()
    list(E.psych_council_stream("x", ids, llm=llm, corpus_dir=None, concurrency=4))
    parallel = time.time() - t
    assert llm.peak == 4
    assert parallel < 4 * llm.delay  # 4 lenses side by side + synthesis, not 4 in a row


def test_concurrency_limit_is_respected():
    llm = SlowLLM(delay=0.1)
    list(E.psych_council_stream("x", ["cbt", "gestalt", "emdr", "jungian", "ericksonian"], llm=llm, corpus_dir=None, concurrency=2))
    assert llm.peak == 2


def test_opinions_keep_the_selected_order_whatever_finishes_first():
    class Uneven(SlowLLM):
        def complete(self, messages):
            import time
            time.sleep(0.3 if "CBT" in str(messages) else 0.01)
            return "CONVERGENCES:\n- a\nDIVERGENCES:\n- b"
    events = list(E.psych_council_stream("x", ["cbt", "gestalt"], llm=Uneven(), corpus_dir=None, concurrency=2))
    streamed = [d.specialist_id for k, d in events if k == "opinion"]
    assert sorted(streamed) == ["cbt", "gestalt"]
    assert E.ordered_opinions(["cbt", "gestalt"], [d for k, d in events if k == "opinion"])[0].specialist_id == "cbt"


def test_an_unavailable_llm_stops_the_council():
    from melori_engine.llm import LLMUnavailable

    class Down:
        def complete(self, messages):
            raise LLMUnavailable("LLM endpoint returned 503")
    with pytest.raises(LLMUnavailable):
        list(E.psych_council_stream("x", ["cbt", "gestalt", "emdr"], llm=Down(), corpus_dir=None, concurrency=2))


def test_concurrency_is_clamped():
    assert E.clamp_concurrency(0) == 1
    assert E.clamp_concurrency(99) == E.MAX_CONCURRENCY


def test_preflight_rejects_empty_and_bad_mode():
    with pytest.raises(E.PsychCouncilError):
        E.preflight("  ", None, "practitioner")
    with pytest.raises(E.PsychCouncilError):
        E.preflight("x", None, "self")


def test_corpus_excerpts_read_from_configured_dir(tmp_path):
    (tmp_path / "cbt").mkdir()
    (tmp_path / "cbt" / "a.md").write_text("thought records", encoding="utf-8")
    (tmp_path / "cbt" / "README.md").write_text("skip", encoding="utf-8")
    assert [t for _, t in S.PSYCH["cbt"].excerpts(tmp_path)] == ["thought records"]


def test_corpus_dir_missing_is_empty_not_error(tmp_path):
    assert S.PSYCH["cbt"].excerpts(tmp_path / "nope") == []


def test_every_lens_carries_the_full_knowledge_status_block():
    # Guard against paraphrased vendoring: the safety floor (toxicology / active drug / delayed care)
    # lives in KNOWLEDGE_STATUS_BLOCK and must reach every persona verbatim.
    from melori_engine.council import prompts as P
    assert "ПОЛ узкий" in P.KNOWLEDGE_STATUS_BLOCK
    for sid, spec in S.PSYCH.items():
        assert spec.system_prompt.endswith(P.KNOWLEDGE_STATUS_BLOCK), sid
    assert P.LEAD_PSYCHOLOGIST_PERSONA.endswith(P.KNOWLEDGE_STATUS_BLOCK)


# ── language of the source transcript drives the council's language ──

def test_language_is_detected_from_the_text():
    from melori_engine.council.language import detect_language
    assert detect_language("Клиент рассказывает о переезде и тревоге на работе.") == "ru"
    assert detect_language("The client talks about moving and stress at work.") == "en"
    assert detect_language("Клиент: ok, sure, yes, fine. Потом долго молчал о работе и сне.") == "ru"
    assert detect_language("") == "en"


class Recorder:
    def __init__(self, reply="CONVERGENCES:\n- a\nDIVERGENCES:\n- b"):
        self.messages, self.reply = [], reply

    def complete(self, messages):
        self.messages.append(messages)
        return self.reply


def test_lenses_and_synthesis_are_asked_to_answer_in_the_transcript_language():
    llm = Recorder()
    list(E.psych_council_stream("x", ["cbt"], llm=llm, corpus_dir=None, language="ru"))
    lens, synthesis = llm.messages
    assert "Russian" in lens[-1]["content"]
    assert "Russian" in synthesis[-1]["content"]
    assert "CONVERGENCES:" in synthesis[-1]["content"] and "DIVERGENCES:" in synthesis[-1]["content"]


def test_language_falls_back_to_the_situation_text():
    llm = Recorder()
    list(E.psych_council_stream("Клиент говорит о сне и тревоге", ["cbt"], llm=llm, corpus_dir=None))
    assert "Russian" in llm.messages[0][-1]["content"]


def test_plan_reports_the_language():
    events = list(E.psych_council_stream("x", ["cbt"], llm=Recorder(), corpus_dir=None, language="en"))
    assert events[0][1]["language"] == "en"


def test_synthesis_parser_reads_localized_and_markdown_headings():
    text = """Интегративная формулировка...

**Совпадения:**
1. все видят тревогу
2) сон нарушен

### Расхождения
- КПТ против гештальта
* разный темп
"""
    conv, div = E._parse_synthesis(text)
    assert conv == ["все видят тревогу", "сон нарушен"]
    assert div == ["КПТ против гештальта", "разный темп"]


def test_synthesis_parser_still_reads_the_english_markers():
    conv, div = E._parse_synthesis("x\nCONVERGENCES:\n- a\nDIVERGENCES:\n- b")
    assert (conv, div) == (["a"], ["b"])


def test_synthesis_parser_accepts_other_russian_names():
    conv, div = E._parse_synthesis("Сходства:\n- a\nРазличия:\n- b")
    assert (conv, div) == (["a"], ["b"])


def test_an_ordinary_line_is_not_taken_for_a_heading():
    conv, div = E._parse_synthesis("Общее впечатление\n- не пункт совпадений\nCONVERGENCES:\n- a")
    assert conv == ["a"] and div == []

