// Sovereignty gate — fails if the Echo model layer regresses off its sovereign rails.
// Runs locally (`node scripts/sovereignty-gate.mjs`) and in CI. Two checks:
//   1. Coupling: no `blob.handy.computer` anywhere except the allow-list (historical
//      attribution, the manifest test guard, vendored upstream forks, planning docs).
//   2. Manifest invariants: src-tauri/resources/models.json is well-formed and fully
//      sovereign (>=16 models, unique ids, exactly one recommended, every model has a
//      non-empty URL that is neither a Handy CDN link nor an unresolved <R2_BASE>
//      placeholder, every model has a sha256 — or, for multi-file entries (files[]),
//      a per-file https url + 64-char sha256 + non-escaping relative path — and a
//      valid provenance_class + engine_type).
// Exit 0 = clean, 1 = gate failed (prints offenders).
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = path.join(ROOT, "src-tauri", "resources", "models.json");

// Paths permitted to mention the former Handy CDN: attribution, the guard that
// asserts against it, vendored upstreams (their own docs/CI), and planning docs.
//
// NB (2026-08-24): проверка идёт по `git grep`, то есть ТОЛЬКО по трекаемым файлам.
// Шаблон для вендоренного форка попадёт в дело лишь если этот форк трекается; оба
// наших лежат в .gitignore, поэтому такие записи недостижимы по построению.
// Запись `/^speakrs-fork\//` снята вместе с выводом форка из рабочего дерева
// (_backups/retired-repos/speakrs-fork-20260824.bundle). `/^transcribe-rs-fork\//`
// ниже недостижима по той же причине и оставлена сознательно: каталог на месте,
// и если его когда-нибудь начнут трекать, разрешение понадобится.
const ALLOW_BLOB = [
  /^THIRD_PARTY\.md$/,
  /^docs\//,
  /^transcribe-rs-fork\//,
  /^src-tauri\/crates\/echo-core\/src\/model_manifest\.rs$/,
  /^scripts\/sovereign-models\.mjs$/,
  /^scripts\/sovereignty-gate\.mjs$/,
];

const ENGINES = new Set([
  "Whisper",
  "Parakeet",
  "Moonshine",
  "MoonshineStreaming",
  "SenseVoice",
  "GigaAM",
  "Canary",
  "Cohere",
  "PiperTts",
  "SupertonicTts",
  "KittenTts",
]);

const fail = [];

// ── 1. Coupling check (git grep over tracked files) ───────────────────────────
let hits = "";
try {
  hits = execFileSync("git", ["grep", "-nI", "blob.handy.computer"], {
    cwd: ROOT,
    encoding: "utf8",
  });
} catch (e) {
  if (e.status !== 1) throw e; // status 1 = no matches = clean
}
for (const line of hits.split("\n").filter(Boolean)) {
  const file = line.split(":")[0].replace(/\\/g, "/");
  if (!ALLOW_BLOB.some((re) => re.test(file))) {
    fail.push(`coupling: blob.handy.computer in ${line}`);
  }
}

// ── 2. Manifest invariants ────────────────────────────────────────────────────
const m = JSON.parse(await readFile(MANIFEST, "utf8"));
if (m.schema_version !== 1)
  fail.push(`manifest: schema_version ${m.schema_version} != 1`);
const ids = new Set();
let recommended = 0;
for (const e of m.models) {
  if (ids.has(e.id)) fail.push(`manifest: duplicate id ${e.id}`);
  ids.add(e.id);
  if (!e.url) fail.push(`manifest: ${e.id} has empty url`);
  else {
    if (/blob\.handy\.computer/.test(e.url))
      fail.push(`manifest: ${e.id} url still on Handy CDN`);
    if (/<R2_BASE>/.test(e.url))
      fail.push(`manifest: ${e.id} has unresolved <R2_BASE> placeholder`);
  }
  if (Array.isArray(e.files)) {
    // Multi-file entry: every file needs a pinned https url + sha256 + safe relative path.
    if (e.files.length === 0) fail.push(`manifest: ${e.id} has empty files[]`);
    for (const f of e.files) {
      const label = `${e.id}/${f.path ?? "<missing path>"}`;
      if (!f.url) fail.push(`manifest: ${label} missing url`);
      else {
        if (!/^https:\/\//.test(f.url))
          fail.push(`manifest: ${label} url is not https`);
        if (/blob\.handy\.computer/.test(f.url))
          fail.push(`manifest: ${label} url still on Handy CDN`);
        if (/<R2_BASE>/.test(f.url))
          fail.push(`manifest: ${label} has unresolved <R2_BASE> placeholder`);
      }
      if (!f.sha256 || f.sha256.length !== 64)
        fail.push(`manifest: ${label} sha256 missing or not 64 hex chars`);
      if (!f.path)
        fail.push(`manifest: ${e.id} has file entry with missing path`);
      else if (
        f.path.includes("..") ||
        f.path.startsWith("/") ||
        /^[A-Za-z]:/.test(f.path)
      )
        fail.push(`manifest: ${label} path escapes model dir`);
    }
  } else if (!e.sha256) fail.push(`manifest: ${e.id} missing sha256`);
  if (!(e.provenance_class >= 1 && e.provenance_class <= 3))
    fail.push(
      `manifest: ${e.id} invalid provenance_class ${e.provenance_class}`,
    );
  if (!ENGINES.has(e.engine_type))
    fail.push(`manifest: ${e.id} unknown engine_type ${e.engine_type}`);
  if (e.is_recommended) recommended++;
}
if (m.models.length < 16)
  fail.push(`manifest: only ${m.models.length} models (expected >=16)`);
if (recommended !== 1)
  fail.push(`manifest: ${recommended} recommended models (expected exactly 1)`);

// ── verdict ───────────────────────────────────────────────────────────────────
if (fail.length) {
  console.error("SOVEREIGNTY GATE FAILED:");
  for (const f of fail) console.error("  - " + f);
  process.exit(1);
}
console.log(
  `Sovereignty gate OK: ${m.models.length} models, ${recommended} recommended, ` +
    `0 stray blob refs, 0 <R2_BASE> placeholders, all checksummed.`,
);
