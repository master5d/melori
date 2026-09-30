"""OpenAI-compatible chat client. No provider names, no keys in code: base_url + model alias only."""
from __future__ import annotations

from urllib.parse import urlparse
import httpx

LOCAL_HOSTS = {"127.0.0.1", "localhost", "::1"}


class LLMUnavailable(RuntimeError):
    pass


class EmbeddingsUnavailable(RuntimeError):
    pass


def is_local_url(url: str) -> bool:
    return (urlparse(url).hostname or "") in LOCAL_HOSTS


class OpenAICompatLLM:
    def __init__(self, base_url: str, model: str, timeout: float = 120.0,
                 transport: httpx.BaseTransport | None = None, api_key: str = ""):
        self.base_url = base_url.rstrip("/")
        self.model = model
        # Optional bearer key for gateways that require one; never logged, never part of the URL.
        headers = {"Authorization": f"Bearer {api_key}"} if api_key else {}
        self._client = httpx.Client(timeout=httpx.Timeout(timeout, connect=10.0), transport=transport,
                                    headers=headers)

    def complete(self, messages: list[dict], response_format=None) -> str:
        body = {"model": self.model, "messages": messages}
        if response_format is not None:
            body["response_format"] = response_format
        try:
            response = self._client.post(f"{self.base_url}/chat/completions",
                                         json=body)
        except httpx.HTTPError as exc:
            raise LLMUnavailable(f"LLM endpoint unreachable: {type(exc).__name__}") from exc
        if response.status_code // 100 != 2:
            raise LLMUnavailable(f"LLM endpoint returned {response.status_code}")
        try:
            return response.json()["choices"][0]["message"]["content"] or ""
        except (KeyError, IndexError, TypeError, ValueError) as exc:
            raise LLMUnavailable("LLM endpoint returned an unexpected body") from exc


class OpenAICompatEmbeddings:
    def __init__(self, base_url: str, model: str, timeout: float = 120.0,
                 api_key: str = "", transport: httpx.BaseTransport | None = None):
        self.base_url = base_url.rstrip("/")
        self.model = model
        headers = {"Authorization": f"Bearer {api_key}"} if api_key else {}
        self._client = httpx.Client(timeout=httpx.Timeout(timeout, connect=10.0), transport=transport,
                                    headers=headers)

    def embed(self, texts: list[str]) -> list[list[float]]:
        try:
            response = self._client.post(f"{self.base_url}/embeddings", json={"model": self.model, "input": texts})
        except httpx.HTTPError as exc:
            raise EmbeddingsUnavailable(f"embeddings endpoint unreachable: {type(exc).__name__}") from exc
        if response.status_code // 100 != 2:
            raise EmbeddingsUnavailable(f"embeddings endpoint returned {response.status_code}")
        try:
            data = response.json()["data"]
            ordered = sorted(data, key=lambda item: int(item["index"]))
            return [[float(x) for x in item["embedding"]] for item in ordered]
        except (KeyError, IndexError, TypeError, ValueError) as exc:
            raise EmbeddingsUnavailable("embeddings endpoint returned an unexpected body") from exc

    __call__ = embed  # the index and search take any `texts -> vectors` callable
