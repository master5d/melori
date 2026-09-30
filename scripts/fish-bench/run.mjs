import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { SCENARIOS, buildTtsRequest } from "./scenarios.mjs";
import { loadFishKey } from "./env.mjs";
import { buildCreateModelRequest, assertPrivate } from "./model.mjs";
import { rowFor, summarizeRun } from "./bench.mjs";

const { values: args } = parseArgs({
  options: {
    ref: { type: "string" },
    "ref-id": { type: "string" },
    model: { type: "string", default: "s2.1-pro" },
    out: { type: "string", default: "./artifacts/fish" },
    only: { type: "string" },
  },
});

const apiKey = loadFishKey({
  envFilePath: resolve(import.meta.dirname, "..", "..", ".env.fish"),
});
const outDir = resolve(args.out);
mkdirSync(outDir, { recursive: true });

async function safeText(res) {
  try {
    return (await res.text()).slice(0, 300);
  } catch {
    return "<unreadable>";
  }
}

async function ensureRefId() {
  if (args["ref-id"]) return args["ref-id"];
  if (!args.ref)
    throw new Error(
      "either --ref <ref.wav> or --ref-id <model-id> is required",
    );

  const wavBytes = readFileSync(resolve(args.ref));
  const { url, headers, form } = buildCreateModelRequest({
    apiKey,
    title: "echo-owner-private",
    wavBytes,
    wavName: basename(args.ref),
  });
  assertPrivate(form);

  const res = await fetch(url, {
    method: "POST",
    headers,
    body: form,
    signal: AbortSignal.timeout(60_000),
  });
  if (!res.ok)
    throw new Error(
      `model create failed: HTTP ${res.status} ${await safeText(res)}`,
    );

  const created = await res.json();
  console.log(
    `voice model created: id=${created._id} state=${created.state} visibility=${created.visibility}`,
  );
  if (created.visibility !== "private") {
    throw new Error(
      `SERVER returned visibility=${created.visibility}; delete the model in the Fish dashboard NOW`,
    );
  }

  return created._id;
}

async function batchOnce(refId, scenario) {
  const { url, headers, body } = buildTtsRequest({
    apiKey,
    model: args.model,
    refId,
    text: scenario.text,
    format: "wav",
    latency: "normal",
  });
  const t0 = performance.now();
  const res = await fetch(url, {
    method: "POST",
    headers,
    body,
    signal: AbortSignal.timeout(60_000),
  });
  if (!res.ok) {
    return rowFor({
      id: scenario.id,
      mode: "batch",
      status: res.status,
      error: await safeText(res),
    });
  }

  const audio = Buffer.from(await res.arrayBuffer());
  const ms = Math.round(performance.now() - t0);
  writeFileSync(join(outDir, `fish_${scenario.id}_batch.wav`), audio);
  return rowFor({
    id: scenario.id,
    mode: "batch",
    status: res.status,
    ms,
    bytes: audio.length,
  });
}

async function streamOnce(refId, scenario) {
  const { url, headers, body } = buildTtsRequest({
    apiKey,
    model: args.model,
    refId,
    text: scenario.text,
    format: "wav",
    latency: "low",
  });
  const t0 = performance.now();
  const res = await fetch(url, {
    method: "POST",
    headers,
    body,
    signal: AbortSignal.timeout(60_000),
  });
  if (!res.ok) {
    return rowFor({
      id: scenario.id,
      mode: "stream",
      status: res.status,
      error: await safeText(res),
    });
  }

  let ttfaMs = null;
  const chunks = [];
  for await (const chunk of res.body) {
    if (ttfaMs === null) ttfaMs = Math.round(performance.now() - t0);
    chunks.push(Buffer.from(chunk));
  }

  const ms = Math.round(performance.now() - t0);
  const audio = Buffer.concat(chunks);
  writeFileSync(join(outDir, `fish_${scenario.id}_stream.wav`), audio);
  return rowFor({
    id: scenario.id,
    mode: "stream",
    status: res.status,
    ms,
    ttfaMs,
    bytes: audio.length,
  });
}

const refId = await ensureRefId();
const only = args.only ? new Set(args.only.split(",")) : null;
const rows = [];

for (const scenario of SCENARIOS) {
  if (only && !only.has(scenario.id)) continue;

  console.log(`>> ${scenario.id} ${scenario.label}`);
  rows.push(
    await batchOnce(refId, scenario).catch((e) =>
      rowFor({ id: scenario.id, mode: "batch", status: 0, error: String(e) }),
    ),
  );
  rows.push(
    await streamOnce(refId, scenario).catch((e) =>
      rowFor({ id: scenario.id, mode: "stream", status: 0, error: String(e) }),
    ),
  );

  for (const r of rows.slice(-2)) {
    console.log(
      `   ${r.mode}: ${r.ok ? "ok" : "FAIL"} status=${r.status} ms=${r.ms} ttfa=${r.ttfaMs} bytes=${r.bytes}${r.error ? " err=" + r.error : ""}`,
    );
  }
}

const summary = summarizeRun(rows);
summary.meta = {
  model: args.model,
  refId,
  date: new Date().toISOString(),
  outDir,
};
writeFileSync(
  join(outDir, "bench-results.json"),
  JSON.stringify(summary, null, 2),
);
console.log(
  `\n${summary.ok} ok / ${summary.failed} failed -> ${join(outDir, "bench-results.json")}`,
);
if (summary.failed > 0) process.exitCode = 2;
