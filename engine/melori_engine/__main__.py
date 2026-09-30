"""`python -m melori_engine` — bind 127.0.0.1 only."""
import logging
import os

import uvicorn

from melori_engine.app import create_app
from melori_engine.config import Settings
from melori_engine.llm import OpenAICompatEmbeddings, OpenAICompatLLM


def main() -> None:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s")
    s = Settings.from_env()
    # The key is read here and handed straight to the client: it is not part of Settings, so it
    # cannot reach /health, logs or error text.
    api_key = os.environ.get("MELORI_LLM_API_KEY", "").strip()
    timeout = float(os.environ.get("MELORI_LLM_TIMEOUT", "600"))
    embed = OpenAICompatEmbeddings(s.llm_base_url, s.embed_model, timeout=timeout, api_key=api_key) if s.embed_model else None
    app = create_app(s, llm=OpenAICompatLLM(s.llm_base_url, s.llm_model, timeout=timeout,
                                            api_key=api_key), embed=embed)
    uvicorn.run(app, host="127.0.0.1", port=s.port, access_log=False, log_level="warning")


if __name__ == "__main__":
    main()
