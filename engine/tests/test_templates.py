import pytest
from melori_engine.practice import templates as T


def test_builtins_are_soap_dap_grow_and_free_is_hidden(tmp_path):
    ids = [t.id for t in T.list_templates(tmp_path)]
    assert ids[:3] == ["soap", "dap", "grow"] and "free" not in ids
    assert [s.key for s in T.BUILTIN["dap"].sections] == ["data", "assessment", "plan"]
    assert [s.key for s in T.BUILTIN["grow"].sections] == ["goal", "reality", "options", "will"]


def test_builtin_titles_follow_the_session_language():
    assert T.builtin_localized(T.BUILTIN["soap"], "ru").sections[0].title != T.builtin_localized(T.BUILTIN["soap"], "en").sections[0].title


@pytest.mark.parametrize("sections", [[], [{"key": "Bad Key", "title": "x", "guidance": ""}],
                                      [{"key": "a", "title": "", "guidance": ""}],
                                      [{"key": "a", "title": "x", "guidance": "g" * 401}],
                                      [{"key": f"k{i}", "title": "x", "guidance": ""} for i in range(13)],
                                      [{"key": "a", "title": "x", "guidance": ""}, {"key": "a", "title": "y", "guidance": ""}]])
def test_invalid_sections_are_rejected(sections):
    with pytest.raises(T.TemplateError):
        T.validate("Mine", sections)


def test_custom_template_roundtrip(tmp_path):
    t = T.create_template(tmp_path, "Коучинг", [{"key": "focus", "title": "Фокус", "guidance": "о чём сессия"}])
    assert t.id.startswith("t-") and not t.builtin
    assert T.get_template(tmp_path, t.id).sections[0].title == "Фокус"
    T.update_template(tmp_path, t.id, "Коучинг 2", [{"key": "focus", "title": "Фокус", "guidance": ""}])
    assert T.get_template(tmp_path, t.id).name == "Коучинг 2"


def test_builtins_are_read_only(tmp_path):
    with pytest.raises(T.TemplateError):
        T.update_template(tmp_path, "soap", "x", [{"key": "a", "title": "x", "guidance": ""}])
    with pytest.raises(T.TemplateError):
        T.delete_template(tmp_path, "soap", clients_root=tmp_path / "clients")
