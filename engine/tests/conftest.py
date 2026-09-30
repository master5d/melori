import pytest
from fastapi.testclient import TestClient

from melori_engine.app import create_app
from melori_engine.config import Settings

TOKEN = "t" * 32


class FakeLLM:
    def __init__(self, reply: str = "CONVERGENCES:\n- a\nDIVERGENCES:\n- b"):
        self.reply = reply
        self.calls: list[list[dict]] = []

    def complete(self, messages: list[dict]) -> str:
        self.calls.append(messages)
        return self.reply


class FakeEmbed:
    def __init__(self):
        self.calls: list[list[str]] = []

    def embed(self, texts):
        self.calls.append(list(texts))
        return [[float(len(texts)), 1.0] for _ in texts]

    __call__ = embed


@pytest.fixture
def settings(tmp_path):
    return Settings(token=TOKEN, port=0, data_dir=tmp_path / "data",
                    llm_base_url="http://127.0.0.1:11434/v1", llm_model="local", corpus_dir=None)


@pytest.fixture
def llm():
    return FakeLLM()


@pytest.fixture
def embed():
    return FakeEmbed()


@pytest.fixture
def client(settings, llm, embed):
    app = create_app(settings, llm=llm, embed=embed)
    with TestClient(app) as c:
        c.headers.update({"X-Melori-Token": TOKEN})
        yield c
