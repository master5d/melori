# melori

melori is a local-first copilot for one-to-one consultation work — coaching, counselling, and other practitioner sessions held over video calls. It captures both sides of the call, transcribes them live on the device, keeps per-client notes and sessions behind explicit, per-kind client consent, and can read a session through a panel of psychology lenses. It is a fork of echo, itself derived from [Handy](https://github.com/cjpais/Handy), with a Rust/Tauri shell, a React interface, and a small local Python engine (`engine/`).

Everything runs on the practitioner's machine. Speech recognition is local; the engine listens only on loopback with a per-run token; language-model calls go only to an endpoint the practitioner configures (a local server by default), with a warning before session content leaves the machine. No account, no telemetry.

melori is meant for sessions where the client has given written consent. It never records the other party covertly and never stores audio or video.

## Build

Requirements: Node.js 20+ and stable Rust.

```bash
npm install
npm run dev
npm run build
npm run tauri dev
```

Run frontend tests with `npm run test:unit`; run native leaf-crate tests from `src-tauri` with `cargo test --offline --workspace --exclude echo`.

## Consent and client data

A client meeting starts only for a client with recorded consent. Consent is granted per kind of data — `transcript`, `video`, `council` (analysis by a language model) and `retain` (keeping material after the session, for a set number of days or until revoked) — and the engine checks each permission at the point of use. melori generates a versioned consent text; the signed copy is attached to the client card with its SHA-256.

- Audio and video are never stored — only text, notes and numeric series.
- Without `retain`, nothing about the session remains on disk after it ends.
- Session notes follow a template — built-in SOAP, DAP and GROW, or your own from Settings → Note templates — and each client can have a default one. The engine writes the note from the transcript (this needs `council`); every model draft and every practitioner edit is kept as a separate version, and a readable `session.md` sits next to the parts in the session's folder.
- Each session opens as its own page — Notes (edit, regenerate, every version), Transcript, Questions and Email — and every model answer shows what was sent to the model and whether it left the machine. The follow-up email is drafted from the current note only (no transcript, no council analysis), edited by the practitioner and opened in your mail client; melori never sends mail itself.
- A client card remembers: "Last time" shows the previous session's plan and email subject at a glance (and, on request, a short model summary of the last sessions); Search finds words in any form and — with an embedding model set in Settings — meaning; Chat answers questions from that client's records only, citing the sessions, and says so when the records do not hold the answer. The per-client index, chats and summary live in the client's folder and expire with the sessions.
- During a client meeting: two lights show who is speaking; Ask answers questions about the meeting so far (free, or "last 5 minutes", "what might I have missed", "what did we agree") from the transcript only; a long silence (5 minutes by default, Settings → Audio) asks whether the meeting ended and stops it after 60 s unless you continue; ten meeting actions can be bound to hotkeys, including "mark this moment". Note sections that read like a diagnosis are flagged for the practitioner to check — the text is never changed.
- The psychology council needs the `council` permission. Its lenses run side by side (4 at a time by default; `MELORI_COUNCIL_CONCURRENCY`, at most 6) and answer in the language of the session's transcript.
- Revoking consent stops the running meeting and blocks new sessions; deleting a client removes their folder entirely.
- Client data lives in the engine's data directory (`%APPDATA%\melori\clients` on Windows, `~/Library/Application Support/melori` on macOS, `~/.local/share/melori` on Linux; override with `MELORI_DATA_DIR`), never in the repository. melori checks at start-up whether the disk is encrypted and warns if it is not.

## Models

Officially sourced models download from the URL recorded in the manifest. Models without an official source have a `mirror_path` and remain unavailable until the user supplies a model mirror URL in settings. Checksums are verified before an artifact is accepted.

## License

melori is released under the MIT License — see `LICENSE`. It is derived from Handy (MIT, © CJ Pais); that notice and the third-party components are listed in `NOTICE` and `THIRD_PARTY.md`.
