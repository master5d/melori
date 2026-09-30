import httpx
import json
import pytest

from melori_engine import llm as L


def test_is_local_url():
    assert L.is_local_url("http://127.0.0.1:11434/v1")
    assert L.is_local_url("http://localhost:4000/v1")
    assert not L.is_local_url("https://gw.example.net/v1")


def test_complete_parses_choice():
    def handler(request):
        assert request.url.path.endswith("/chat/completions")
        return httpx.Response(200, json={"choices": [{"message": {"content": "hi"}}]})
    m = L.OpenAICompatLLM("http://127.0.0.1:9/v1", "local", transport=httpx.MockTransport(handler))
    assert m.complete([{"role": "user", "content": "x"}]) == "hi"


def test_complete_sends_response_format_when_given():
    def handler(request):
        assert json.loads(request.content)["response_format"] == {"type": "json_object"}
        return httpx.Response(200, json={"choices": [{"message": {"content": "{}"}}]})
    m = L.OpenAICompatLLM("http://127.0.0.1:9/v1", "local", transport=httpx.MockTransport(handler))
    assert m.complete([{"role": "user", "content": "x"}], {"type": "json_object"}) == "{}"


def test_non_2xx_is_unavailable():
    m = L.OpenAICompatLLM("http://127.0.0.1:9/v1", "local", transport=httpx.MockTransport(lambda r: httpx.Response(503)))
    with pytest.raises(L.LLMUnavailable):
        m.complete([{"role": "user", "content": "x"}])


def test_connection_error_is_unavailable():
    def boom(request):
        raise httpx.ConnectError("down")
    m = L.OpenAICompatLLM("http://127.0.0.1:9/v1", "local", transport=httpx.MockTransport(boom))
    with pytest.raises(L.LLMUnavailable):
        m.complete([{"role": "user", "content": "x"}])


def test_api_key_is_sent_as_bearer_only_when_given():
    seen = []

    def handler(request):
        seen.append(request.headers.get("authorization"))
        return httpx.Response(200, json={"choices": [{"message": {"content": "ok"}}]})
    L.OpenAICompatLLM("http://127.0.0.1:9/v1", "m", transport=httpx.MockTransport(handler),
                      api_key="k-123").complete([{"role": "user", "content": "x"}])
    L.OpenAICompatLLM("http://127.0.0.1:9/v1", "m",
                      transport=httpx.MockTransport(handler)).complete([{"role": "user", "content": "x"}])
    assert seen == ["Bearer k-123", None]


def test_embeddings_are_ordered_by_index():
    def handler(request):
        body = json.loads(request.content)
        assert body == {"model": "embed", "input": ["a", "b"]}
        return httpx.Response(200, json={"data": [
            {"index": 1, "embedding": [2, 0]}, {"index": 0, "embedding": [1, 0]}]})
    client = L.OpenAICompatEmbeddings("http://127.0.0.1:9/v1", "embed", transport=httpx.MockTransport(handler))
    assert client.embed(["a", "b"]) == [[1.0, 0.0], [2.0, 0.0]]


def test_embeddings_non_2xx_is_unavailable():
    client = L.OpenAICompatEmbeddings("http://127.0.0.1:9/v1", "embed",
                                      transport=httpx.MockTransport(lambda r: httpx.Response(500)))
    with pytest.raises(L.EmbeddingsUnavailable):
        client.embed(["x"])
