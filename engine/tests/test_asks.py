import pytest
from melori_engine.practice import asks as A

SEG = lambda i, src, start, text: {"i": i, "source": src, "start_ms": start, "end_ms": start + 2000, "text": text}

def test_recap5_takes_only_the_last_five_minutes_and_skips_marks():
    segs = [SEG(1, "others", 0, "старое начало"), {"i": 2, "source": "mark", "start_ms": 400_000, "end_ms": 400_000, "text": ""}, SEG(3, "others", 590_000, "свежая реплика"), SEG(4, "me", 598_000, "вопрос практика")]
    text, truncated = A.transcript_for_ask(segs, "recap5", 600_000)
    assert "старое начало" not in text and "Клиент: свежая реплика" in text and "Я: вопрос практика" in text
    assert "mark" not in text and truncated is False

def test_long_transcript_keeps_the_end_and_says_so():
    segs = [SEG(i, "others", i * 1000, f"реплика номер {i} " + "x" * 80) for i in range(400)]
    text, truncated = A.transcript_for_ask(segs, "free", 400_000)
    assert truncated and len(text) <= 12000 and "реплика номер 399" in text and "реплика номер 0 " not in text

def test_messages_forbid_scripts_and_diagnoses_and_set_the_language():
    msgs = A.build_ask_messages("Что с ним?", "Клиент: плохо сплю", True, "ru")
    sys = msgs[0]["content"].lower()
    assert "diagnos" in sys and ("what to say" in sys or "script" in sys)
    assert "Russian" in msgs[-1]["content"] and "omitted" in msgs[-1]["content"].lower()

class Empty:
    def __init__(self): self.calls = 0
    def complete(self, messages): self.calls += 1; return ""

def test_empty_answers_fail_loudly_after_three_attempts():
    llm = Empty()
    with pytest.raises(A.AskError): A.ask(llm, "q", "free", [SEG(1, "others", 0, "текст")], 10_000, "ru")
    assert llm.calls == 3
