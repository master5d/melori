from melori_engine.practice.diagnosis import find_diagnostic_phrases as f

def test_diagnosis_dictionary():
    found = f({"assessment": "Стресс-индуцированная бессонница, F51.0; возможно тревожное расстройство.", "data": "Клиент расстроен, тревожится из-за работы."})
    phrases = " | ".join(x["phrase"].lower() for x in found)
    assert "бессонница" in phrases and "f51.0" in phrases and "расстройство" in phrases
    assert all(x["section"] == "assessment" for x in found)
    assert f({"plan": "Suspected generalized anxiety disorder (ADHD ruled out)."})
    assert f({"plan": "Client feels anxious and upset."}) == []


def test_the_clients_own_word_is_not_a_formulation():
    # "у меня бессонница" is what the client said; a qualified condition is the model's formulation
    assert f({"data": "Клиент говорит: у меня бессонница уже месяц."}) == []
    assert [x["phrase"].lower() for x in f({"assessment": "Хроническая бессонница."})] == ["хроническая бессонница"]
    assert f({"assessment": "Возможен синдром выгорания."})
