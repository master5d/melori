// scripts/check-nix-deps.mjs — keep .nix/bun.nix in sync with bun.lock.
//
// Written in plain Node ESM (no bun-specific globals) so that `npm install`,
// `pnpm install`, and `bun install` all succeed on any machine, even without
// bun installed. The actual regeneration step still requires bun (it shells
// out to `bunx bun2nix`), so this script skips gracefully when bun is absent
// — that path only matters for the Nix build, which has bun available.
//
// Triggered automatically via the package.json "postinstall" hook, or run
// manually: `node scripts/check-nix-deps.mjs`. The Nix build strips the
// postinstall hook entirely (see flake.nix), so this never runs there.
//
// If it regenerates .nix/bun.nix, commit it with bun.lock + .nix/bun-lock-hash.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const nixDir = join(root, ".nix");
const lockFile = join(root, "bun.lock");
const hashFile = join(nixDir, "bun-lock-hash");
const nixFile = join(nixDir, "bun.nix");

// bun2nix is Nix-only and hangs on Windows CI.
if (process.platform === "win32") process.exit(0);

// No bun.lock — nothing to sync.
if (!existsSync(lockFile)) process.exit(0);

// Only meaningful when bun is available (the Nix dev path). On machines
// without bun (npm/pnpm developers) this is a no-op so installs never break.
const bunVersion = spawnSync("bun", ["--version"], { stdio: "ignore" });
if (bunVersion.error || bunVersion.status !== 0) process.exit(0);

mkdirSync(nixDir, { recursive: true });

const currentHash = createHash("sha256")
  .update(readFileSync(lockFile))
  .digest("hex");
const storedHash = existsSync(hashFile)
  ? readFileSync(hashFile, "utf-8").trim()
  : "";

// bun.nix already matches bun.lock — nothing to do.
if (currentHash === storedHash) process.exit(0);

console.log(`[check-nix-deps] bun.lock changed, regenerating ${nixFile}...`);

const result = spawnSync("bunx", ["bun2nix", "-o", nixFile], {
  cwd: root,
  stdio: "inherit",
});

if (result.error || result.status !== 0) {
  console.warn(
    "[check-nix-deps] bunx bun2nix unavailable; .nix/bun.nix may be outdated.",
  );
  console.warn("[check-nix-deps] Non-Nix users can safely ignore this.");
  // Don't block install — CI validates bun.nix independently.
  process.exit(0);
}

writeFileSync(hashFile, currentHash + "\n");
console.log(`[check-nix-deps] Updated ${nixFile}`);
console.log(
  "[check-nix-deps] Commit: bun.lock .nix/bun.nix .nix/bun-lock-hash",
);
