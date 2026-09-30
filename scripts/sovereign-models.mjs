// Sovereign model fetcher: downloads official upstream artifacts named in
// src-tauri/resources/models.json, computes SHA256, and writes the checksums
// back into the manifest. Never touches blob.handy.computer.
//
// Usage:
//   node scripts/sovereign-models.mjs               # all models
//   node scripts/sovereign-models.mjs --class=1     # only provenance_class 1
//   node scripts/sovereign-models.mjs --id=turbo    # a single model
//   node scripts/sovereign-models.mjs --force       # re-download even if staged
import { createHash } from "node:crypto";
import { createWriteStream, createReadStream } from "node:fs";
import { readFile, writeFile, mkdir, stat, rename, rm } from "node:fs/promises";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { fileURLToPath } from "node:url";
import path from "node:path";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = path.join(ROOT, "src-tauri", "resources", "models.json");
const STAGING = path.join(ROOT, ".sovereign-models");

export async function sha256File(filePath) {
  const hash = createHash("sha256");
  await pipeline(createReadStream(filePath), hash);
  return hash.digest("hex");
}

export async function download(url, dest, { idleMs = 120_000 } = {}) {
  // Артефакты многогигабайтные — общий таймаут на весь запрос неуместен.
  // Вместо него idle-контроль: таймер сбрасывается каждым полученным chunk'ом;
  // 120 s без единого байта = зависший сервер/линк -> abort с внятной ошибкой,
  // а не вечно висящий pipeline.
  const controller = new AbortController();
  let idleTimer = null;
  let stalled = false;
  const armIdle = () => {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      stalled = true;
      controller.abort();
    }, idleMs);
  };
  armIdle();
  const tmp = dest + ".tmp";
  try {
    const res = await fetch(url, {
      redirect: "follow",
      signal: controller.signal,
    });
    if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
    await mkdir(path.dirname(dest), { recursive: true });
    async function* resetIdlePerChunk(src) {
      for await (const chunk of src) {
        armIdle();
        yield chunk;
      }
    }
    try {
      await pipeline(
        resetIdlePerChunk(Readable.fromWeb(res.body)),
        createWriteStream(tmp),
      );
      await rename(tmp, dest);
    } catch (e) {
      await rm(tmp, { force: true }).catch(() => {});
      throw e;
    }
    return dest;
  } catch (e) {
    if (stalled) {
      throw new Error(
        `download stalled: no progress for ${idleMs / 1000}s from ${url}`,
      );
    }
    throw e;
  } finally {
    clearTimeout(idleTimer);
  }
}

export async function loadManifest() {
  return JSON.parse(await readFile(MANIFEST, "utf8"));
}

export async function saveManifest(m) {
  const tmp = MANIFEST + ".tmp";
  await writeFile(tmp, JSON.stringify(m, null, 2) + "\n", "utf8");
  await rename(tmp, MANIFEST);
}

async function fileExists(p) {
  try {
    await stat(p);
    return true;
  } catch {
    return false;
  }
}

async function processEntry(entry, { force }) {
  // Skip artifacts whose own-R2 host is not provisioned yet (Tasks 8/9 fill these).
  if (entry.url.includes("<R2_BASE>")) {
    console.log(`skip ${entry.id} (R2 host not provisioned)`);
    return {
      id: entry.id,
      class: entry.provenance_class,
      sha256: entry.sha256,
      bytes: 0,
    };
  }
  // Multi-file entry (files[]): stage each file under .sovereign-models/<id>/<path>,
  // hash it, and write files[i].sha256 back — the per-file mirror of the single branch.
  if (Array.isArray(entry.files)) {
    let bytes = 0;
    for (const f of entry.files) {
      if (
        f.path.includes("..") ||
        f.path.startsWith("/") ||
        /^[A-Za-z]:/.test(f.path)
      ) {
        throw new Error(`${entry.id}: unsafe file path ${f.path}`);
      }
      const dest = path.join(STAGING, entry.id, ...f.path.split("/"));
      if (!(await fileExists(dest)) || force) {
        console.log(`download ${entry.id}/${f.path} <- ${f.url}`);
        await download(f.url, dest);
      }
      const sum = await sha256File(dest);
      if (f.sha256 && f.sha256 !== sum) {
        console.warn(
          `WARN ${entry.id}/${f.path}: sha changed ${f.sha256} -> ${sum}`,
        );
      }
      f.sha256 = sum;
      bytes += (await stat(dest)).size;
    }
    const prevMb = entry.size_mb;
    entry.size_mb = Math.max(1, Math.round(bytes / (1024 * 1024)));
    return {
      id: entry.id,
      class: entry.provenance_class,
      files: entry.files.length,
      bytes,
      size_mb: entry.size_mb,
      prev_mb: prevMb,
    };
  }
  const suffix = entry.is_directory
    ? entry.url.endsWith(".tar.bz2")
      ? ".tar.bz2"
      : ".tar.gz"
    : "";
  const dest = path.join(STAGING, entry.filename + suffix);
  if (!(await fileExists(dest)) || force) {
    console.log(`download ${entry.id} <- ${entry.url}`);
    await download(entry.url, dest);
  }
  const sum = await sha256File(dest);
  if (entry.sha256 && entry.sha256 !== sum) {
    console.warn(`WARN ${entry.id}: sha changed ${entry.sha256} -> ${sum}`);
  }
  entry.sha256 = sum;
  const bytes = (await stat(dest)).size;
  // Reconcile size_mb to the real artifact (manifest values were copied from the
  // old Handy quants; official variants differ, e.g. medium q4_1 -> q5_0).
  const prevMb = entry.size_mb;
  entry.size_mb = Math.max(1, Math.round(bytes / (1024 * 1024)));
  return {
    id: entry.id,
    class: entry.provenance_class,
    sha256: sum,
    bytes,
    size_mb: entry.size_mb,
    prev_mb: prevMb,
  };
}

async function main() {
  const args = process.argv.slice(2);
  const force = args.includes("--force");
  const idFilter = args.find((a) => a.startsWith("--id="))?.slice(5);
  const classFilter = args.find((a) => a.startsWith("--class="))?.slice(8);

  const manifest = await loadManifest();
  const report = [];
  for (const entry of manifest.models) {
    if (idFilter && entry.id !== idFilter) continue;
    if (classFilter && String(entry.provenance_class) !== classFilter) continue;
    report.push(await processEntry(entry, { force }));
  }
  await saveManifest(manifest);
  console.table(report);
  console.log(`Updated ${report.length} model(s) in ${MANIFEST}`);
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
