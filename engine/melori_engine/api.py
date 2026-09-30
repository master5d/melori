"""HTTP surface. Consent is enforced in the store; this layer maps errors to status codes."""
from __future__ import annotations
import json
import os
from dataclasses import asdict
from datetime import date, datetime
from fastapi import APIRouter, File, HTTPException, Request, Response, UploadFile
from fastapi.responses import JSONResponse, StreamingResponse
from pydantic import BaseModel, Field
from melori_engine.consent_template import CURRENT_TEMPLATE_VERSION, render_consent
from melori_engine.council import engine as council
from melori_engine.llm import LLMUnavailable
from melori_engine.llm import is_local_url
from melori_engine.council.language import detect_language, normalize
from melori_engine.practice import notes as note_engine
from melori_engine.practice import asks as ask_engine
from melori_engine.practice.diagnosis import find_diagnostic_phrases
from melori_engine.practice import email as email_engine
from melori_engine.practice import sessions as session_store
from melori_engine.practice import store as st
from melori_engine.practice import templates as tm
from melori_engine.practice import index as index_engine
from melori_engine.practice import search as search_engine
from melori_engine.practice import chat as chat_engine
from melori_engine.practice import recap as recap_engine
from melori_engine.practice.models import Consent, NoteVersion, Segment

router = APIRouter()

class ClientCreate(BaseModel):
    alias: str
    tags: list[str] = Field(default_factory=list)
    intake: str = ""
    permissions: list[str]
    consent_date: str
    retain_days: int | None = None

class SessionOpen(BaseModel):
    meeting_type: str = "session"

class SessionClose(BaseModel):
    note: str

class CouncilRequest(BaseModel):
    situation: str
    specialist_ids: list[str] = Field(default_factory=list)
    mode: str = "practitioner"
    ground_in_records: bool = False
    client_id: str | None = None
    session_id: str | None = None
    language: str | None = None  # of the source transcript; detected from situation if absent

class TemplateDraft(BaseModel):
    name: str
    sections: list[dict]

class ClientTemplateUpdate(BaseModel):
    template_id: str | None = None

class TranscriptRequest(BaseModel):
    segments: list[dict]
    duration_ms: int = 0

class NoteGenerateRequest(BaseModel):
    template_id: str
    language: str | None = None
    segments: list[dict] | None = None

class AskRequest(BaseModel):
    question: str = ""
    kind: str = "free"
    segments: list[dict] = Field(default_factory=list)
    elapsed_ms: int = 0

class NoteCreateRequest(BaseModel):
    template_id: str
    fields: dict[str, str]
    parent: int | None = None

class EmailNoteIn(BaseModel):
    n: int | None = None
    template_id: str = "free"
    sections: list[list[str]]
    fields: dict[str, str]

class EmailGenerateRequest(BaseModel):
    language: str | None = None
    note: EmailNoteIn | None = None  # only used when no note is stored (client without `retain`)

class EmailUpdateRequest(BaseModel):
    subject: str
    body: str

class ChatRequest(BaseModel):
    question: str
    chat: int | None = None
    language: str | None = None

class RecapRequest(BaseModel):
    language: str | None = None

def _root(request: Request):
    return request.app.state.settings.clients_root

def client_out(c) -> dict:
    co = c.consent
    return {"id": c.id, "alias": c.alias, "tags": c.tags, "created": c.created,
            "template_id": c.template_id,
            "consent": {"permissions": sorted(co.permissions), "date": co.date, "retain_days": co.retain_days,
                        "template_version": co.template_version, "signed_sha256": co.signed_sha256,
                        "revoked": co.revoked, "active": co.active}}

def session_out(s) -> dict:
    return {"id": s.id, "client": s.client, "date": s.date, "created": s.created, "status": s.status,
            "meeting_type": s.meeting_type, "note": s.note, "runs": [asdict(r) for r in s.runs],
            "template_id": s.template_id, "duration_ms": s.duration_ms, "language": s.language,
            "transcript": [asdict(segment) for segment in s.transcript],
            "notes": [{"n": n.n, "created": n.created, "author": n.author,
                       "template_id": n.template_id, "parent": n.parent, "warnings": list(n.warnings)} for n in s.notes],
            "asks": s.asks, "migrated": s.migrated}


def _segment(value: dict, index: int) -> Segment:
    try:
        return Segment(int(value.get("i", index)), str(value["source"]), int(value.get("start_ms", 0)),
                      int(value.get("end_ms", 0)), str(value["text"]))
    except (KeyError, TypeError, ValueError) as exc:
        raise HTTPException(422, "invalid transcript segment") from exc


def _session_detail(root, cid: str, sid: str) -> dict:
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    current = s.notes[-1] if s.notes else None
    return {"session": {"id": s.id, "client": s.client, "date": s.date, "created": s.created,
                         "status": s.status, "meeting_type": s.meeting_type, "template_id": s.template_id,
                         "duration_ms": s.duration_ms, "language": s.language},
            "transcript": [asdict(segment) for segment in s.transcript],
            "note": ({"n": current.n, "created": current.created, "author": current.author,
                      "template_id": current.template_id, "sections": [list(x) for x in current.sections],
                      "fields": current.fields, "parent": current.parent,
                      "provenance": current.provenance, "warnings": list(current.warnings)} if current else None),
            "versions": [{"n": n.n, "created": n.created, "author": n.author,
                          "template_id": n.template_id, "parent": n.parent,
                          "provenance": n.provenance, "warnings": list(n.warnings)} for n in s.notes],
            "email": session_store.read_email(session_store.session_dir(root, cid, sid)),
            "asks": s.asks, "runs": [asdict(run) for run in s.runs]}

def _guard(fn):
    try:
        return fn()
    except st.ConsentError as exc:
        raise HTTPException(403, str(exc))
    except st.StoreError as exc:
        msg = str(exc)
        raise HTTPException(404 if msg.startswith("no such") else 422, msg)

def template_out(t):
    return {"id": t.id, "name": t.name, "builtin": t.builtin,
            "sections": [{"key": s.key, "title": s.title, "guidance": s.guidance} for s in t.sections]}

def _template_error(exc: tm.TemplateError) -> HTTPException:
    msg = str(exc)
    return HTTPException({"builtin": 409, "not found": 404}.get(msg, 422), msg)

def _template_guard(fn):
    try:
        return fn()
    except tm.TemplateError as exc:
        raise _template_error(exc)

@router.get("/api/clients")
def list_clients(request: Request):
    return [client_out(c) for c in st.list_clients(_root(request))]

@router.get("/api/templates")
def list_templates(request: Request, language: str | None = None):
    lang = normalize(language) or "en"  # built-ins are shown in the interface's language
    return [template_out(tm.builtin_localized(t, lang)) for t in tm.list_templates(request.app.state.settings.data_dir)]

@router.post("/api/templates", status_code=201)
def create_template(req: TemplateDraft, request: Request):
    return template_out(_template_guard(lambda: tm.create_template(request.app.state.settings.data_dir, req.name, req.sections)))

@router.put("/api/templates/{template_id}")
def update_template(template_id: str, req: TemplateDraft, request: Request):
    return template_out(_template_guard(lambda: tm.update_template(request.app.state.settings.data_dir, template_id, req.name, req.sections)))

@router.delete("/api/templates/{template_id}", status_code=204)
def delete_template(template_id: str, request: Request, force: bool = False):
    try:
        tm.delete_template(request.app.state.settings.data_dir, template_id,
                           clients_root=request.app.state.settings.clients_root, force=force)
    except tm.TemplateInUse as exc:
        return JSONResponse({"detail": str(exc), "clients": exc.n}, status_code=409)
    except tm.TemplateError as exc:
        raise _template_error(exc)
    return Response(status_code=204)

@router.post("/api/clients")
def create_client(req: ClientCreate, request: Request):
    consent = Consent(permissions=frozenset(req.permissions), date=req.consent_date, retain_days=req.retain_days)
    return client_out(_guard(lambda: st.create_client(_root(request), req.alias, tags=req.tags, consent=consent, intake=req.intake)))

@router.get("/api/clients/{cid}")
def client_detail(cid: str, request: Request):
    c = _guard(lambda: st.get_client(_root(request), cid))
    if c is None:
        raise HTTPException(404, f"no such client: {cid}")
    return {"client": client_out(c), "sessions": [session_out(s) for s in st.list_sessions(_root(request), cid)]}

@router.put("/api/clients/{cid}")
def update_client_template(cid: str, req: ClientTemplateUpdate, request: Request):
    if req.template_id is not None and tm.get_template(request.app.state.settings.data_dir, req.template_id) is None:
        raise HTTPException(422, f"unknown template: {req.template_id}")
    return client_out(_guard(lambda: st.set_client_template(_root(request), cid, req.template_id)))

@router.get("/api/clients/{cid}/consent/template")
def consent_template(cid: str, request: Request):
    c = _guard(lambda: st.get_client(_root(request), cid))
    if c is None:
        raise HTTPException(404, f"no such client: {cid}")
    return {"version": CURRENT_TEMPLATE_VERSION, "text": render_consent(c)}

@router.post("/api/clients/{cid}/consent/signed")
async def consent_signed(cid: str, request: Request, file: UploadFile = File(...)):
    data = await file.read()
    return client_out(_guard(lambda: st.attach_signed_consent(_root(request), cid, data=data, template_version=CURRENT_TEMPLATE_VERSION)))

@router.post("/api/clients/{cid}/revoke")
def revoke(cid: str, request: Request):
    return client_out(_guard(lambda: st.revoke(_root(request), cid, today=date.today().isoformat())))

@router.delete("/api/clients/{cid}", status_code=204)
def delete(cid: str, request: Request):
    _guard(lambda: st.delete_client(_root(request), cid))
    return Response(status_code=204)

@router.post("/api/clients/{cid}/sessions")
def open_session(cid: str, req: SessionOpen, request: Request):
    if req.meeting_type not in {"session", "business"}:
        raise HTTPException(422, "meeting_type must be session or business")
    return session_out(_guard(lambda: st.open_session(_root(request), cid, date=date.today().isoformat(), meeting_type=req.meeting_type)))

@router.delete("/api/clients/{cid}/sessions/{sid}", status_code=204)
def delete_session(cid: str, sid: str, request: Request):
    _guard(lambda: st.delete_session(_root(request), cid, sid))
    return Response(status_code=204)

@router.get("/api/clients/{cid}/search")
def search_client(cid: str, request: Request, q: str = "", mode: str = "both", limit: int = 20):
    root = _root(request)
    client = _guard(lambda: st.require_permission(root, cid, "transcript"))
    if mode not in {"words", "meaning", "both"}:
        raise HTTPException(422, "mode must be words, meaning, or both")
    if mode == "meaning" and "council" not in client.consent.permissions:
        raise HTTPException(403, "permission 'council' not granted")
    embed = request.app.state.embed if "council" in client.consent.permissions and request.app.state.settings.embed_model else None
    ix = index_engine.ClientIndex(root, cid)
    try:  # an open index.sqlite would keep Windows from deleting the client's folder
        report = ix.refresh(embed=embed, embed_model=request.app.state.settings.embed_model if embed else "")
        effective = mode if mode == "words" or embed is not None else "words"
        result = search_engine.hybrid(ix, q, effective, embed, max(1, min(limit, 100)))
    finally:
        ix.close()
    result["index"]["meaning_available"] = bool(embed is not None and not report.meaning_error)
    result["index"]["meaning_error"] = report.meaning_error
    return result

@router.get("/api/clients/{cid}/chats")
def list_client_chats(cid: str, request: Request):
    _guard(lambda: st.require_permission(_root(request), cid, "transcript"))
    return chat_engine.list_chats(_root(request), cid)

@router.get("/api/clients/{cid}/chats/{n}")
def get_client_chat(cid: str, n: int, request: Request):
    _guard(lambda: st.require_permission(_root(request), cid, "transcript"))
    try:
        return chat_engine.read_chat(_root(request), cid, n)
    except FileNotFoundError as exc:
        raise HTTPException(404, f"no such chat: {n}") from exc

@router.delete("/api/clients/{cid}/chats/{n}", status_code=204)
def remove_client_chat(cid: str, n: int, request: Request):
    try:
        chat_engine.delete_chat(_root(request), cid, n)
    except FileNotFoundError as exc:
        raise HTTPException(404, f"no such chat: {n}") from exc
    return Response(status_code=204)

@router.post("/api/clients/{cid}/chats")
def ask_client_chat(cid: str, req: ChatRequest, request: Request):
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    c = _guard(lambda: st.require_permission(root, cid, "council"))
    language = normalize(req.language) or detect_language(req.question)
    ix = index_engine.ClientIndex(root, cid)
    try:
        embed = request.app.state.embed if settings.embed_model else None
        report = ix.refresh(embed=embed, embed_model=settings.embed_model if embed else "")
        mode = "both" if embed is not None else "words"
        found = search_engine.hybrid(ix, req.question, mode, embed, 8)["results"]
        sessions = {s.id: s for s in st.list_sessions(root, cid)}
        fragments = [{**hit, "text": next((row["text"] for row in ix._open().execute("SELECT text FROM chunks WHERE id=?", (hit["chunk_id"],))), ""),
                      "date": sessions[hit["session_id"]].date} for hit in found if hit["session_id"] in sessions]
    finally:
        ix.close()
    existing = None
    if req.chat is not None:
        try:
            existing = chat_engine.read_chat(root, cid, req.chat)
        except FileNotFoundError as exc:
            raise HTTPException(404, f"no such chat: {req.chat}") from exc
    history = existing.get("turns", []) if existing else []
    if not fragments:
        answer, attempts = chat_engine.NO_RECORDS.get(language[:2], chat_engine.NO_RECORDS["en"]), 0
    else:
        try:
            answer, attempts = chat_engine.answer(llm, req.question, fragments, history, language)
        except (chat_engine.ChatError, LLMUnavailable) as exc:
            raise HTTPException(502, str(exc)) from exc
    turn = {"q": req.question, "a": answer, "at": datetime.now().isoformat(timespec="seconds"),
            "refs": [{"session_id": f["session_id"], "kind": f["kind"], "chunk_id": f["chunk_id"]} for f in fragments],
            "provenance": {"model": settings.llm_model, "endpoint_local": is_local_url(settings.llm_base_url),
                           "input": "fragments", "chars_sent": sum(len(f.get("text", "")) for f in fragments),
                           "fragments": len(fragments), "sessions": len({f["session_id"] for f in fragments}),
                           "language": language, "attempts": attempts}}
    stored = "retain" in c.consent.permissions
    n = req.chat
    if stored:
        try:
            n = chat_engine.save_turn(root, cid, req.chat, turn)
        except FileNotFoundError as exc:
            raise HTTPException(404, f"no such chat: {req.chat}") from exc
    return {"chat": n if stored else None, "turn": turn, "stored": stored}

@router.get("/api/clients/{cid}/recap")
def get_client_recap(cid: str, request: Request):
    root = _root(request)
    _guard(lambda: st.require_permission(root, cid, "transcript"))
    sessions = st.list_sessions(root, cid)
    value = recap_engine.read_recap(root, cid)
    return {"quick": recap_engine.quick_recap(root, cid), "model": value,
            "model_stale": recap_engine.recap_stale(value, sessions)}

@router.post("/api/clients/{cid}/recap")
def create_client_recap(cid: str, request: Request, req: RecapRequest | None = None):
    req = req or RecapRequest()  # the card posts no body
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    c = _guard(lambda: st.require_permission(root, cid, "council"))
    closed = sorted((s for s in st.list_sessions(root, cid) if s.status == "closed"), key=lambda s: (s.date, s.id))[-3:]
    notes = [(s.date, session_store.note_markdown(s.notes[-1] if s.notes else None)) for s in closed]
    notes = [(d, text) for d, text in notes if text.strip()]
    if not notes:  # nothing to summarise: do not call the model on an empty input
        raise HTTPException(422, "no closed session with a note")
    language = normalize(req.language) or detect_language("\n".join(text for _, text in notes))
    try:
        points, attempts = recap_engine.model_recap(llm, notes, language)
    except (recap_engine.RecapError, LLMUnavailable) as exc:
        raise HTTPException(502, str(exc)) from exc
    value = {"created": datetime.now().isoformat(timespec="seconds"), "sessions": [s.id for s in closed], "points": points,
             "provenance": {"model": settings.llm_model, "endpoint_local": is_local_url(settings.llm_base_url),
                            "input": "notes", "chars_sent": sum(len(text) for _, text in notes),
                            "language": language, "attempts": attempts}}
    stored = "retain" in c.consent.permissions
    if stored:
        recap_engine.write_recap(root, cid, value)
    return {"recap": value, "stored": stored, "model_stale": False}

@router.post("/api/clients/{cid}/sessions/{sid}/close")
def close_session(cid: str, sid: str, req: SessionClose, request: Request):
    saved, retained = _guard(lambda: st.close_session(_root(request), cid, sid, note=req.note))
    return {"retained": retained, "session": session_out(saved) if saved else None}


@router.get("/api/clients/{cid}/sessions/{sid}")
def session_detail(cid: str, sid: str, request: Request):
    return _session_detail(_root(request), cid, sid)


@router.post("/api/clients/{cid}/sessions/{sid}/transcript", status_code=204)
def save_transcript(cid: str, sid: str, req: TranscriptRequest, request: Request):
    root = _root(request)
    c = _guard(lambda: st.require_permission(root, cid, "transcript"))
    if "retain" not in c.consent.permissions:
        return Response(status_code=204)
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    d = session_store.session_dir(root, cid, sid)
    segments = [_segment(value, i) for i, value in enumerate(req.segments, 1)]
    meta = session_store.read_meta(d)
    meta["duration_ms"] = req.duration_ms
    meta["language"] = detect_language("\n".join(segment.text for segment in segments))
    session_store.write_meta(d, meta)
    session_store.write_transcript(d, segments)
    session_store.rebuild_session_md(d, c.alias)
    return Response(status_code=204)


@router.post("/api/clients/{cid}/sessions/{sid}/asks")
def ask_meeting(cid: str, sid: str, req: AskRequest, request: Request):
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    c = _guard(lambda: st.require_permission(root, cid, "council"))
    if req.kind not in {"free", *ask_engine.QUICK}:
        raise HTTPException(422, "unknown ask kind")
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    language = normalize(s.language) or detect_language("\n".join(str(x.get("text", "")) for x in req.segments))
    question = ask_engine.QUICK[req.kind][language] if req.kind != "free" else req.question.strip()
    if req.kind == "free" and not question:
        raise HTTPException(422, "question is required")
    try:
        answer, provenance = ask_engine.ask(llm, question, req.kind, req.segments, req.elapsed_ms, language)
    except (ask_engine.AskError, LLMUnavailable) as exc:
        raise HTTPException(502, str(exc)) from exc
    provenance = {"model": settings.llm_model, "endpoint_local": is_local_url(settings.llm_base_url), **provenance}
    value = {"at": datetime.now().isoformat(timespec="seconds"), "elapsed_ms": req.elapsed_ms,
             "kind": req.kind, "question": question, "answer": answer, "provenance": provenance}
    stored = "retain" in c.consent.permissions
    if stored:
        session_store.append_ask(session_store.session_dir(root, cid, sid), value)
    return {"ask": value, "stored": stored}


def _note_response(note, sections, fields, provenance=None):
    return {"n": note.n if note else None, "created": note.created if note else None,
            "author": note.author if note else "model", "template_id": note.template_id if note else None,
            "sections": [list(x) for x in sections], "fields": dict(fields), "parent": note.parent if note else None,
            "provenance": provenance, "warnings": list(note.warnings) if note else find_diagnostic_phrases(fields)}


@router.post("/api/clients/{cid}/sessions/{sid}/notes:generate")
def generate_note(cid: str, sid: str, req: NoteGenerateRequest, request: Request):
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    c = _guard(lambda: st.require_permission(root, cid, "council"))
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    template = tm.get_template(settings.data_dir, req.template_id)
    if template is None:
        raise HTTPException(422, f"unknown template: {req.template_id}")
    segments = [_segment(value, i) for i, value in enumerate(req.segments, 1)] if req.segments is not None else s.transcript
    transcript_text = "\n".join(f"{segment.source}: {segment.text}" for segment in segments).strip()
    if not transcript_text:
        raise HTTPException(422, "transcript is empty")
    language = normalize(req.language) or normalize(s.language) or detect_language(transcript_text)
    try:
        fields, attempts = note_engine.generate(llm, tm.builtin_localized(template, language), transcript_text, language)
    except (note_engine.NoteGenerationError, LLMUnavailable) as exc:
        raise HTTPException(502, str(exc)) from exc
    provenance = {"model": settings.llm_model, "endpoint_local": is_local_url(settings.llm_base_url),
                  "input": "transcript", "chars_sent": len(transcript_text), "language": language,
                  "attempts": attempts}
    stored = "retain" in c.consent.permissions
    saved = None
    if stored:
        d = session_store.session_dir(root, cid, sid)
        localized = tm.builtin_localized(template, language)
        saved = session_store.add_note(d, author="model", template_id=template.id, fields=fields,
                                       sections=[(section.key, section.title) for section in localized.sections],
                                       parent=s.notes[-1].n if s.notes else None, provenance=provenance,
                                       warnings=find_diagnostic_phrases(fields))
        session_store.rebuild_session_md(d, c.alias)
    sections =[(section.key, section.title) for section in tm.builtin_localized(template, language).sections]
    return {"note": _note_response(saved, sections, fields, provenance), "stored": stored}


def _current_note(root, cid: str, sid: str, sent: "EmailNoteIn | None"):
    """The stored current note; without `retain` nothing is stored, so the window sends the note it shows
    (like transcript segments for note generation). The engine keeps no copy in memory."""
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    if s.notes:
        return s, s.notes[-1]
    if sent is None:
        return s, None
    sections = tuple((str(k), str(t)) for k, t in sent.sections)
    if not sections or {k for k, _ in sections} != set(sent.fields):
        raise HTTPException(422, "note sections do not match its fields")
    return s, NoteVersion(n=sent.n or 0, created="", author="model", template_id=sent.template_id,
                          fields=dict(sent.fields), parent=None, sections=sections)


@router.post("/api/clients/{cid}/sessions/{sid}/email:generate")
def generate_email(cid: str, sid: str, request: Request, req: EmailGenerateRequest | None = None):
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    c = _guard(lambda: st.require_permission(root, cid, "council"))
    s, note = _current_note(root, cid, sid, req.note if req else None)
    if note is None:
        raise HTTPException(422, "write a note first")
    language = (normalize(req.language if req else None) or normalize(s.language)
                or detect_language(email_engine.note_text(note)))
    try:
        fields, attempts = email_engine.generate_email(llm, note, language)
    except (email_engine.EmailGenerationError, LLMUnavailable) as exc:
        raise HTTPException(502, str(exc)) from exc
    provenance = {"model": settings.llm_model, "endpoint_local": is_local_url(settings.llm_base_url),
                  "input": "note", "chars_sent": len(email_engine.note_text(note)),
                  "language": language, "attempts": attempts}
    value = {"subject": fields["subject"], "body": fields["body"], "author": "model",
             "from_note": note.n or None, "created": datetime.now().isoformat(timespec="seconds"),
             "provenance": provenance, "opened_in_mail_at": None}
    stored = "retain" in c.consent.permissions
    if stored:
        d = session_store.session_dir(root, cid, sid)
        session_store.write_email(d, value)
        session_store.rebuild_session_md(d, c.alias)
    return {"email": value, "stored": stored}


@router.put("/api/clients/{cid}/sessions/{sid}/email")
def save_email(cid: str, sid: str, req: EmailUpdateRequest, request: Request):
    root = _root(request)
    c = _guard(lambda: st.require_permission(root, cid, "transcript"))
    if "retain" not in c.consent.permissions:
        raise HTTPException(403, "nothing is retained")
    s, current_note = _current_note(root, cid, sid, None)
    d = session_store.session_dir(root, cid, sid)
    current = session_store.read_email(d)
    value = dict(current or {})
    value.update({"subject": req.subject, "body": req.body, "author": "practitioner",
                  "from_note": current.get("from_note") if current else (current_note.n if current_note else None),
                  "created": current.get("created") if current else datetime.now().isoformat(timespec="seconds"),
                  "provenance": current.get("provenance") if current else None,
                  "opened_in_mail_at": current.get("opened_in_mail_at") if current else None})
    session_store.write_email(d, value)
    session_store.rebuild_session_md(d, c.alias)
    return value


@router.post("/api/clients/{cid}/sessions/{sid}/email/opened", status_code=204)
def mark_email_opened(cid: str, sid: str, request: Request):
    root = _root(request)
    c = _guard(lambda: st.require_permission(root, cid, "transcript"))
    if "retain" not in c.consent.permissions:
        return Response(status_code=204)
    d = session_store.session_dir(root, cid, sid)
    value = session_store.read_email(d)
    if value is not None:
        value["opened_in_mail_at"] = datetime.now().isoformat(timespec="seconds")
        session_store.write_email(d, value)
        session_store.rebuild_session_md(d, c.alias)
    return Response(status_code=204)


@router.post("/api/clients/{cid}/sessions/{sid}/notes")
def create_practitioner_note(cid: str, sid: str, req: NoteCreateRequest, request: Request):
    root, settings = _root(request), request.app.state.settings
    c = _guard(lambda: st.require_permission(root, cid, "transcript"))
    if "retain" not in c.consent.permissions:
        raise HTTPException(403, "nothing is retained")
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    parent = next((n for n in s.notes if n.n == req.parent), None) if req.parent is not None else None
    if req.parent is not None and parent is None:
        raise HTTPException(422, f"no such parent version: {req.parent}")
    if parent is not None and parent.template_id == req.template_id and parent.sections:
        sections = list(parent.sections)  # keep the headings the edited version was written with
    else:
        template = tm.get_template(settings.data_dir, req.template_id)
        if template is None:
            raise HTTPException(422, f"unknown template: {req.template_id}")
        localized = tm.builtin_localized(template, s.language or "ru")
        sections = [(section.key, section.title) for section in localized.sections]
    if set(req.fields) != {key for key, _ in sections}:
        raise HTTPException(422, "fields do not match template")
    d = session_store.session_dir(root, cid, sid)
    note = session_store.add_note(d, author="practitioner", template_id=req.template_id, fields=req.fields,
                                  sections=sections, parent=req.parent)
    session_store.rebuild_session_md(d, c.alias)
    return _note_response(note, note.sections, note.fields)


@router.get("/api/clients/{cid}/sessions/{sid}/notes/{n}")
def get_note(cid: str, sid: str, n: int, request: Request):
    root = _root(request)
    s = _guard(lambda: st.get_session(root, cid, sid))
    if s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    try:
        note = session_store.read_note(session_store.session_dir(root, cid, sid), n)
    except (FileNotFoundError, ValueError, KeyError) as exc:
        raise HTTPException(404, f"no such note: {n}") from exc
    return _note_response(note, note.sections, note.fields, note.provenance)

def _council_concurrency() -> int:
    """MELORI_COUNCIL_CONCURRENCY, clamped to the measured ceiling."""
    raw = os.environ.get("MELORI_COUNCIL_CONCURRENCY", "")
    try:
        return council.clamp_concurrency(int(raw)) if raw.strip() else council.DEFAULT_CONCURRENCY
    except ValueError:
        return council.DEFAULT_CONCURRENCY

@router.get("/api/psych-council/specialists")
def council_specialists(response: Response):
    from melori_engine.council import specialists as sp
    response.headers["X-Council-Concurrency"] = str(_council_concurrency())
    return [{"id": s.id, "name": s.name, "paradigm": s.paradigm} for s in sp.PSYCH.values()]

@router.post("/api/psych-council/stream")
def council_stream(req: CouncilRequest, request: Request):
    root, settings, llm = _root(request), request.app.state.settings, request.app.state.llm
    try:
        council.preflight(req.situation, req.specialist_ids or None, req.mode)
    except council.PsychCouncilError as exc:
        raise HTTPException(422, str(exc))
    persist = False
    if req.client_id:
        c = _guard(lambda: st.require_permission(root, req.client_id, "council"))
        persist = bool(req.session_id) and "retain" in c.consent.permissions

    def gen():
        opinions, synthesis, order = [], None, []
        try:
            for kind, payload in council.psych_council_stream(req.situation, req.specialist_ids or None, llm=llm,
                                                              corpus_dir=settings.corpus_dir, mode=req.mode,
                                                              concurrency=_council_concurrency(),
                                                              language=req.language):
                d = payload if isinstance(payload, dict) else asdict(payload)
                if kind == "plan": order = d["specialists"]
                elif kind == "opinion": opinions.append(d)
                elif kind == "synthesis": synthesis = d
                yield f"event: {kind}\ndata: {json.dumps(d, ensure_ascii=False)}\n\n"
            if persist:
                rank = {sid: i for i, sid in enumerate(order)}
                opinions.sort(key=lambda o: rank.get(o["specialist_id"], len(rank)))
                st.append_run(root, req.client_id, req.session_id, mode=req.mode,
                              specialist_ids=[o["specialist_id"] for o in opinions],
                              result={"opinions": opinions, "synthesis": synthesis})
        except LLMUnavailable as exc:
            yield "event: error\ndata: " + json.dumps({"detail": str(exc)}) + "\n\n"
        finally:
            yield "event: done\ndata: {}\n\n"
    return StreamingResponse(gen(), media_type="text/event-stream")

@router.get("/api/clients/{cid}/sessions/{sid}/export")
def export_session(cid: str, sid: str, request: Request, include_council: bool = False):
    root = _root(request); c = _guard(lambda: st.get_client(root, cid)); s = _guard(lambda: st.get_session(root, cid, sid))
    if c is None or s is None:
        raise HTTPException(404, f"no such session: {cid}/{sid}")
    parts = [f"# {c.alias} — {s.date}", "", f"Template: {s.template_id or 'soap'}", ""]
    if s.notes:
        parts.extend(["## Note", ""])
        for key, title in s.notes[-1].sections:
            parts.extend([f"### {title}", "", s.notes[-1].fields.get(key, ""), ""])
    if s.transcript:
        parts.extend(["## Transcript", ""])
        parts.extend(f"{segment.source}: {segment.text}" for segment in s.transcript)
    if include_council and s.runs:
        parts += ["", "## Разбор", "", (s.runs[-1].result.get("synthesis") or {}).get("text", "")]
    return {"filename": f"{cid}-{sid}.md", "markdown": "\n".join(parts) + "\n"}

@router.get("/api/purge-report")
def purge_report(request: Request):
    return {"deleted": request.app.state.purged}
