// Model-CDN sovereignty parity QA (Task 10).
//
// For each officially-sourced model in src-tauri/resources/models.json (the 12 that
// are NOT garage mirrors), transcribe the language-appropriate reference clip with
// Echo's offline CLI and score the output against the known ground truth (word error
// rate). Surfaces any official upstream artifact that regresses vs the clean ground
// truth — especially `medium` (q4_1 -> official q5_0) and the sherpa-onnx bundles.
//
// The 4 garage-hosted artifacts (canary-1b-v2 + moonshine x3) are byte-identical
// mirrors of what the original app used, so they need no parity check and are skipped.
//
// Usage:
//   node scripts/parity-qa.mjs --echo <path-to-echo-binary>
//   node scripts/parity-qa.mjs --echo ../src-tauri/target/release/echo --only turbo,medium
//   node scripts/parity-qa.mjs            # auto-detects target/release/echo[.exe]
//
// Prereqs: the models under test must already be downloaded in Echo (Settings ->
// Models); mirror-only models also need the model mirror configured in Settings.
// Exit code is non-zero if any model lands in REVIEW, so it can gate CI.
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { existsSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = path.join(ROOT, "src-tauri", "resources", "models.json");
const PARITY = path.join(ROOT, "docs", "parity");
const WER_PASS = 0.2; // <=20% word error on clean TTS speech => PASS, else REVIEW

function arg(name) {
  const a = process.argv.find((x) => x.startsWith(`--${name}=`));
  if (a) return a.slice(name.length + 3);
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function findEcho() {
  const explicit = arg("echo");
  if (explicit) return path.resolve(explicit);
  for (const p of [
    "src-tauri/target/release/echo",
    "src-tauri/target/release/echo.exe",
    "src-tauri/target/debug/echo",
    "src-tauri/target/debug/echo.exe",
  ]) {
    const full = path.join(ROOT, p);
    if (existsSync(full)) return full;
  }
  return null;
}

const normalize = (s) =>
  s
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();

function wer(refText, hypText) {
  const ref = normalize(refText).split(" ").filter(Boolean);
  const hyp = normalize(hypText).split(" ").filter(Boolean);
  if (ref.length === 0) return hyp.length === 0 ? 0 : 1;
  // Levenshtein over word arrays.
  const dp = Array.from({ length: ref.length + 1 }, () =>
    new Array(hyp.length + 1).fill(0),
  );
  for (let i = 0; i <= ref.length; i++) dp[i][0] = i;
  for (let j = 0; j <= hyp.length; j++) dp[0][j] = j;
  for (let i = 1; i <= ref.length; i++) {
    for (let j = 1; j <= hyp.length; j++) {
      const cost = ref[i - 1] === hyp[j - 1] ? 0 : 1;
      dp[i][j] = Math.min(
        dp[i - 1][j] + 1,
        dp[i][j - 1] + 1,
        dp[i - 1][j - 1] + cost,
      );
    }
  }
  return dp[ref.length][hyp.length] / ref.length;
}

function transcribe(echo, clip, modelId, lang) {
  // Echo CLI logs to stdout, so don't scrape stdout — capture the transcript via
  // --output to a temp file (clean plain text), then read it back.
  const outFile = path.join(
    os.tmpdir(),
    `echo-parity-${modelId}-${lang}-${Date.now()}.txt`,
  );
  try {
    execFileSync(
      echo,
      [
        "--transcribe-file",
        clip,
        "--model",
        modelId,
        "--language",
        lang,
        "--output",
        outFile,
        "--no-tray",
      ],
      {
        encoding: "utf8",
        timeout: 600000,
        stdio: ["ignore", "ignore", "pipe"],
      },
    );
    return readFileSync(outFile, "utf8").trim();
  } finally {
    try {
      rmSync(outFile, { force: true });
    } catch {}
  }
}

async function main() {
  const echo = findEcho();
  if (!echo || !existsSync(echo)) {
    console.error(
      "Echo binary not found. Build it (`npm run tauri build`) or pass --echo <path>.",
    );
    process.exit(2);
  }
  const manifest = JSON.parse(await readFile(MANIFEST, "utf8"));
  const gt = JSON.parse(
    await readFile(path.join(PARITY, "ground-truth.json"), "utf8"),
  );
  const clips = gt.clips.map((c) => ({
    ...c,
    path: path.join(PARITY, c.file),
  }));

  const only = arg("only")
    ?.split(",")
    .map((s) => s.trim());
  const official = manifest.models.filter((m) => !m.mirrored && !m.converted);
  const targets = only ? official.filter((m) => only.includes(m.id)) : official;

  console.log(`Echo: ${echo}`);
  console.log(
    `Testing ${targets.length} officially-sourced models against ground truth\n`,
  );
  const rows = [];
  let anyReview = false;

  for (const m of targets) {
    const langs = new Set(m.supported_languages || []);
    const applicable = clips.filter((c) => langs.has(c.lang));
    if (applicable.length === 0) {
      rows.push({
        model: m.id,
        lang: "-",
        wer: "-",
        verdict: "SKIP (no en/ru)",
        hyp: "",
      });
      continue;
    }
    for (const clip of applicable) {
      let verdict, werVal, hyp;
      try {
        hyp = transcribe(echo, clip.path, m.id, clip.lang);
        werVal = wer(clip.text, hyp);
        verdict = werVal <= WER_PASS ? "PASS" : "REVIEW";
        if (verdict === "REVIEW") anyReview = true;
      } catch (e) {
        const msg = (e.stderr || e.message || "").toString().split("\n")[0];
        verdict = /not.*download|not available|Model not found/i.test(msg)
          ? "NOT DOWNLOADED"
          : "ERROR";
        werVal = "-";
        hyp = msg.slice(0, 60);
        anyReview = true;
      }
      rows.push({
        model: m.id,
        lang: clip.lang,
        wer: typeof werVal === "number" ? werVal.toFixed(3) : werVal,
        verdict,
        hyp: (hyp || "").slice(0, 70),
      });
    }
  }

  console.table(rows);
  console.log(
    `\nGround truth (en): "${gt.clips.find((c) => c.lang === "en").text}"`,
  );
  console.log(
    `Ground truth (ru): "${gt.clips.find((c) => c.lang === "ru").text}"`,
  );
  console.log(
    "\nPASS = WER <= 0.20 on clean TTS speech. REVIEW = inspect the hyp column;" +
      " a high WER may be a quant/vintage regression OR just TTS artifacts — judge by eye.",
  );
  process.exit(anyReview ? 1 : 0);
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});
