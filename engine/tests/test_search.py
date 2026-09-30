from melori_engine.practice import index as I, search as SR
from tests.test_chunks import mk, session_with


class FakeEmbed:
    def __init__(self, dim=8, fail_on=None):
        self.dim, self.fail_on, self.calls = dim, fail_on, 0
    def __call__(self, texts):
        self.calls += 1
        if self.fail_on == self.calls:
            raise RuntimeError("embeddings down")
        out = []
        for t in texts:
            v = [0.0] * self.dim
            for w in I.stem_text(t).split():
                v[hash(w) % self.dim] += 1.0
            out.append(v)
        return out


def test_rrf_prefers_items_high_in_both_lists():
    a = [I.Hit(1, "s", "note", "", None, "", 0), I.Hit(2, "s", "note", "", None, "", 0)]
    b = [I.Hit(2, "s", "note", "", None, "", 0), I.Hit(3, "s", "note", "", None, "", 0)]
    assert [h.chunk_id for h in SR.rrf([a, b])][0] == 2


def test_snippet_marks_the_match():
    s = SR.snippet("Долго говорили о работе. Потом про сон и режим.", "сна")
    assert "[[сон]]" in s and len(s) <= 240


def test_model_change_resets_vectors(tmp_path):
    c = mk(tmp_path); session_with(tmp_path, c.id, "2026-09-01", [("others", "про сон")])
    ix = I.ClientIndex(tmp_path, c.id)
    assert ix.refresh(embed=FakeEmbed(8), embed_model="m1").vectors_missing == 0
    r = ix.refresh(embed=FakeEmbed(16), embed_model="m2")
    assert r.vectors_missing == 0 and len(ix.search_vectors([1.0] * 16)) > 0


def test_partial_embedding_failure(tmp_path):
    c = mk(tmp_path)
    for day in range(1, 8):
        # long utterances: enough chunks for more than one batch of 32 (short ones gave 28 — one batch, no failure)
        session_with(tmp_path, c.id, f"2026-09-0{day}", [("others", f"реплика {i} про сон, " + "и ещё немного текста " * 6) for i in range(40)])
    ix = I.ClientIndex(tmp_path, c.id)
    r = ix.refresh(embed=FakeEmbed(8, fail_on=2), embed_model="m")
    assert 0 < r.vectors_missing < r.chunks and r.meaning_error
    out = SR.hybrid(ix, "сон", "both", FakeEmbed(8))
    assert out["results"] and out["index"]["vectors_missing"] == r.vectors_missing
    assert ix.refresh(embed=FakeEmbed(8), embed_model="m").vectors_missing == 0


def test_words_only_never_calls_embeddings(tmp_path):
    c = mk(tmp_path); session_with(tmp_path, c.id, "2026-09-01", [("others", "про сон")])
    ix = I.ClientIndex(tmp_path, c.id); e = FakeEmbed()
    ix.refresh(); out = SR.hybrid(ix, "сон", "words", e)
    assert e.calls == 0 and out["results"] and out["index"]["meaning_available"] is False
