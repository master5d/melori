from melori_engine.practice import chat


class EmptyLLM:
    def __init__(self):
        self.calls = 0

    def complete(self, messages):
        self.calls += 1
        return ""


def test_messages_are_scoped_to_dated_fragments_and_history():
    messages = chat.build_chat_messages("What happened?", [{"date": "2026-09-12", "kind": "note", "label": "Plan", "text": "rest"}], [], "en")
    assert "[2026-09-12] (note · Plan) rest" in messages[-1]["content"]
    assert "English" in messages[-1]["content"]  # the answer language is set
    assert "diagnose" in messages[0]["content"]
    assert "not in the records" in messages[0]["content"]


def test_empty_model_response_retries_three_times_then_fails_loudly():
    # a silent model must not read as "this is not in the records"
    llm = EmptyLLM()
    import pytest
    with pytest.raises(chat.ChatError):
        chat.answer(llm, "q", [{"date": "2026-09-12", "text": "x"}], [], "en")
    assert llm.calls == 3


def test_refs_are_normalized_to_bare_dates():
    # local-floor copied the fragment header into its citations (live check 2026-09-29)
    text = "Slept badly [2026-09-12 · note · Data], better [2026-09-19]."
    assert chat.normalize_refs(text) == "Slept badly [2026-09-12], better [2026-09-19]."


def test_save_turn_creates_and_appends_chat(tmp_path):
    turn = {"q": "q", "a": "a"}
    n = chat.save_turn(tmp_path, "anna", None, turn)
    assert n == 1
    chat.save_turn(tmp_path, "anna", n, {"q": "q2", "a": "a2"})
    assert len(chat.read_chat(tmp_path, "anna", 1)["turns"]) == 2
    assert "a" not in chat.list_chats(tmp_path, "anna")[0]["turns"][0]
