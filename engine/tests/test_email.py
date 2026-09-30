import json
import pytest
from melori_engine.practice import email as E
from melori_engine.practice.models import NoteVersion


class FakeLLM:
    def __init__(self, replies):
        self.replies, self.calls = list(replies), []
    def complete(self, messages, response_format=None):
        self.calls.append(messages)
        return self.replies.pop(0)


NOTE = NoteVersion(n=2, created="2026-09-29T10:00:00", author="model", template_id="dap",
                   fields={"data": "Хочет гулять по утрам.", "plan": "Купить обувь на неделе."},
                   parent=None, sections=(("data", "Данные"), ("plan", "План")))


def test_note_text_uses_titles_and_fields():
    assert E.note_text(NOTE) == "Данные:\nХочет гулять по утрам.\n\nПлан:\nКупить обувь на неделе."


def test_messages_carry_the_note_and_the_rules_only():
    msgs = E.build_email_messages(E.note_text(NOTE), "ru")
    text = json.dumps(msgs, ensure_ascii=False)
    assert "Купить обувь на неделе." in text and "Russian" in msgs[-1]["content"]
    assert "diagnos" in msgs[0]["content"].lower()


def test_generate_email_reads_fenced_json_and_counts_attempts():
    llm = FakeLLM(["", '```json\n{"subject": "После встречи", "body": "Добрый день! Договорились купить обувь."}\n```'])
    email, attempts = E.generate_email(llm, NOTE, "ru")
    assert email == {"subject": "После встречи", "body": "Добрый день! Договорились купить обувь."} and attempts == 2


def test_generate_email_trims_to_limits_on_a_sentence():
    body = "Короткая фраза. " * 400
    llm = FakeLLM([json.dumps({"subject": "x" * 200, "body": body})])
    email, _ = E.generate_email(llm, NOTE, "ru")
    assert len(email["subject"]) <= 120 and len(email["body"]) <= 4000 and email["body"].endswith(".")


def test_unreadable_three_times_is_an_email_error():
    with pytest.raises(E.EmailGenerationError):
        E.generate_email(FakeLLM(["no", "no", "no"]), NOTE, "ru")
