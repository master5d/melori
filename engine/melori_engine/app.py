"""FastAPI app factory. Token on every route; request log = method, route template, status."""
from __future__ import annotations

import hmac
import logging
from urllib.parse import urlparse

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from melori_engine import __version__
from melori_engine.config import Settings
from melori_engine.diskcrypt import disk_encryption_status

log = logging.getLogger("melori.engine")


def create_app(settings: Settings, llm=None, embed=None) -> FastAPI:
    app = FastAPI(title="melori-engine", version=__version__)
    app.state.settings = settings
    app.state.llm = llm
    app.state.embed = embed
    settings.clients_root.mkdir(parents=True, exist_ok=True)
    from datetime import date as _d
    from melori_engine.api import router
    from melori_engine.practice.store import purge_expired
    app.state.purged = purge_expired(settings.clients_root, today=_d.today())
    if app.state.purged:
        log.info("purge: %d expired sessions deleted", len(app.state.purged))
    app.include_router(router)

    @app.middleware("http")
    async def token_and_log(request: Request, call_next):
        given = request.headers.get("X-Melori-Token", "")
        if not hmac.compare_digest(given.encode(), settings.token.encode()):
            log.info("%s %s 401", request.method, request.url.path.split("/")[1:2])
            return JSONResponse({"detail": "bad token"}, status_code=401)
        response = await call_next(request)
        route = request.scope.get("route")
        log.info("%s %s %s", request.method, getattr(route, "path", "?"), response.status_code)
        return response

    @app.get("/health")
    def health():
        local = (urlparse(settings.llm_base_url).hostname or "") in {"127.0.0.1", "localhost", "::1"}
        return {"ok": True, "version": __version__,
                "disk_encryption": disk_encryption_status(settings.data_dir), "llm_local": local}

    return app
