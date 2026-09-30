---
name: echo-ask
description: Ask the human a question through Echo's floating panel (Windows). Use when a task needs user input, confirmation, or a periodic check-in, and you are running on the user's machine alongside Echo.
---

# Echo Ask

Echo (the dictation app) exposes a localhost Agent Bridge.

1. Read the token from `%APPDATA%\lab.melori.app\agent_bridge_token`.
2. POST to `http://127.0.0.1:4123/v1/ask` with header `Authorization: Bearer <token>`. The call BLOCKS until the human answers, dismisses, or it times out.

Request body:

```json
{
  "question": "...",
  "kind": "text",
  "options": [],
  "timeout_s": 300,
  "speak": false,
  "source": "<your-name>"
}
```

`kind` is `text`, `choice`, or `confirm`. Response:

```json
{ "status": "answered", "answer": "..." }
```

`status` is `answered`, `dismissed`, or `timeout`.

3. Fire-and-forget messages: POST `/v1/notify` with `{ "message": "...", "speak": true }`.
4. Journal: GET `/v1/answers?since=<unix_ms>`.

Etiquette: one question at a time (the server enforces it); keep questions short;
set `speak: true` only for time-sensitive asks; always set `source`.

CLI alternative for cron loops:

```bash
echo --ask "..." --ask-options "a,b" --ask-timeout 60
```
