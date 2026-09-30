import json

import pytest

from melori_engine.practice import notes
from melori_engine.practice.templates import BUILTIN


class FakeLLM:
    def __init__(self, replies):
        self.replies = list(replies)
        self.calls = []

    def complete(self, messages, response_format=None):
        self.calls.append((messages, response_format))
        return self.replies.pop(0)


def test_schema_for_dap_requires_all_sections():
    assert notes.schema_for(BUILTIN["dap"])["required"] == ["data", "assessment", "plan"]


def test_build_messages_mentions_language_titles_and_guidance():
    messages = notes.build_messages(BUILTIN["dap"], "Клиент рассказал о цели.", "ru")
    user = messages[-1]["content"]
    assert "Russian" in user
    assert "Data" in user and "session observations and facts" in user
    assert "Assessment" in user and "Plan" in user


def test_generate_retries_empty_answers():
    llm = FakeLLM(["", "", json.dumps({"data": "d", "assessment": "a", "plan": "p"})])
    fields, attempts = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields == {"data": "d", "assessment": "a", "plan": "p"}
    assert attempts == 3


def test_generate_reports_missing_section():
    llm = FakeLLM([json.dumps({"data": "d", "assessment": "a"})])
    with pytest.raises(notes.NoteGenerationError, match="plan"):
        notes.generate(llm, BUILTIN["dap"], "facts", "en")


def test_generate_reads_an_object_wrapped_in_a_code_fence():
    llm = FakeLLM(['```json\n{"data": "d", "assessment": "a", "plan": "p"}\n```'])
    fields, _ = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields == {"data": "d", "assessment": "a", "plan": "p"}


def test_generate_reads_markdown_sections_by_key_or_title():
    # the real local-floor ignores response_format and answered like this (neutral probe, 2026-09-28)
    llm = FakeLLM(["**data**: The client wants to walk.\n\n**Assessment**: Motivated.\nStill motivated.\n\n## Plan\nBuy shoes."])
    fields, _ = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields == {"data": "The client wants to walk.", "assessment": "Motivated.\nStill motivated.", "plan": "Buy shoes."}


def test_generate_joins_list_values():
    llm = FakeLLM([json.dumps({"data": "d", "assessment": "a", "plan": ["one", "two"]})])
    fields, _ = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields["plan"] == "- one\n- two"


def test_generate_asks_again_after_an_unreadable_reply():
    llm = FakeLLM(["I cannot comply.", json.dumps({"data": "d", "assessment": "a", "plan": "p"})])
    fields, attempts = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields["plan"] == "p" and attempts == 2


def test_generate_gives_up_after_three_unreadable_replies():
    llm = FakeLLM(["no", "still no", "never"])
    with pytest.raises(notes.NoteGenerationError, match="invalid JSON"):
        notes.generate(llm, BUILTIN["dap"], "facts", "en")


def test_build_messages_shows_the_exact_json_shape():
    user = notes.build_messages(BUILTIN["dap"], "facts", "en")[-1]["content"]
    assert '{"data": "...", "assessment": "...", "plan": "..."}' in user


def test_generate_drops_extra_keys():
    llm = FakeLLM([json.dumps({"data": "d", "assessment": "a", "plan": "p", "extra": "x"})])
    fields, _ = notes.generate(llm, BUILTIN["dap"], "facts", "en")
    assert fields == {"data": "d", "assessment": "a", "plan": "p"}
