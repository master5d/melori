from melori_engine.practice import index as I, store as st
from tests.test_chunks import mk, session_with


def test_fleeting_vowel_forms_share_a_variant():
    # сон/сна/сном, день/дня, отец/отца: one index term in common, by rule — not by a per-word exception
    for forms in (("сон", "сна", "сном"), ("день", "дня", "днём"), ("отец", "отца", "отцом")):
        common = set.intersection(*(set(I.stem_variants(f)) for f in forms))
        assert common, forms


def test_stem_merges_word_forms():
    assert I.stem("работе") == I.stem("работу") == I.stem("работа")
    assert I.stem("sleeping") == I.stem("sleep")


def test_fts_query_is_escaped(tmp_path):
    c = mk(tmp_path); session_with(tmp_path, c.id, "2026-09-01", [("others", "про сон и работу")])
    ix = I.ClientIndex(tmp_path, c.id); ix.refresh()
    for q in ['сон" OR (x*', "AND", "-", '"', "((", "NEAR(a b)"]:
        ix.search_words(q)
    assert [h.session_id for h in ix.search_words('сон" OR (x*')]


def test_words_find_other_forms_and_refresh_is_incremental(tmp_path):
    c = mk(tmp_path)
    s1, _ = session_with(tmp_path, c.id, "2026-09-01", [("others", "Плохо сплю, сна почти нет")])
    s2, d2 = session_with(tmp_path, c.id, "2026-09-08", [("others", "Работа отнимает вечера")])
    ix = I.ClientIndex(tmp_path, c.id)
    r = ix.refresh()
    assert sorted(r.rebuilt) == sorted([s1.id, s2.id]) and r.vectors_missing == r.chunks
    assert [h.session_id for h in ix.search_words("сон")] == [s1.id]
    assert ix.refresh().rebuilt == []
    (d2 / "email.json").write_text('{"subject": "тема", "body": "про сон", "author": "model", "from_note": 2, "created": "", "provenance": null, "opened_in_mail_at": null}', encoding="utf-8")
    assert ix.refresh().rebuilt == [s2.id]
    st.delete_session(tmp_path, c.id, s1.id)
    r = ix.refresh()
    assert r.dropped == [s1.id] and {h.session_id for h in ix.search_words("сон")} == {s2.id}


def test_empty_client_searches_to_nothing(tmp_path):
    c = mk(tmp_path); ix = I.ClientIndex(tmp_path, c.id); ix.refresh()
    assert ix.search_words("что угодно") == []
