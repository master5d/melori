import httpx
import json
import pytest

from melori_engine import llm as L


def test_is_local_url():
    assert L.is_local_url("http://127.0.0.1:11434/v1")
    assert L.is_local_url("http://localhost:4000/v1")
    assert not L.is_local_url("https://gw.example.net/v1")


def test_extra_header_parsing(monkeypatch):
    monkeypatch.delenv(L.EXTRA_HEADER_ENV, raising=False)
    assert L.extra_headers() == {}
    for bad in ("", "no-colon", ": value", "name:", "bad name: v", "n: v\r\nx: y"):
        monkeypatch.setenv(L.EXTRA_HEADER_ENV, bad)
        assert L.extra_headers() == {}, bad
    monkeypatch.setenv(L.EXTRA_HEADER_ENV, " x-client-tag :  melori ")
    assert L.extra_headers() == {"x-client-tag": "melori"}


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


def test_extra_header_is_sent_for_chat_and_embeddings(monkeypatch):
    monkeypatch.setenv(L.EXTRA_HEADER_ENV, "x-client-tag: melori")
    seen = []

    def handler(request):
        seen.append((request.url.path, request.headers.get("x-client-tag")))
        if request.url.path.endswith("/embeddings"):
            return httpx.Response(200, json={"data": [{"index": 0, "embedding": [1]}]})
        return httpx.Response(200, json={"choices": [{"message": {"content": "ok"}}]})

    url = "https://gw.example.net/v1"
    L.OpenAICompatLLM(url, "m", transport=httpx.MockTransport(handler)).complete([])
    L.OpenAICompatEmbeddings(url, "embed", transport=httpx.MockTransport(handler)).embed(["x"])
    assert seen == [("/v1/chat/completions", "melori"), ("/v1/embeddings", "melori")]


def test_no_extra_header_by_default(monkeypatch):
    monkeypatch.delenv(L.EXTRA_HEADER_ENV, raising=False)
    seen = []

    def handler(request):
        seen.append(request.headers.get("x-client-tag"))
        return httpx.Response(200, json={"choices": [{"message": {"content": "ok"}}]})

    for url in ("http://127.0.0.1:9/v1", "https://gw.example.net/v1"):
        L.OpenAICompatLLM(url, "m", transport=httpx.MockTransport(handler)).complete([])
    assert seen == [None, None]


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
