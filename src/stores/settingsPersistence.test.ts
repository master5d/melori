import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

// Guard against the "settings reset" regression: a settings control wired in the
// UI via `updateSetting("<key>", ...)` only persists if `<key>` has an entry in
// the `settingUpdaters` map (or is one of the two keys intentionally handled
// elsewhere). Without it, `updateSetting` updates the UI optimistically, logs
// "No handler for setting", and drops the change — so it reverts on restart.
// This test fails the build if any literal-key call site lacks a persister.

function walkTsFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walkTsFiles(p));
    else if (/\.(ts|tsx)$/.test(entry.name)) out.push(p);
  }
  return out;
}

describe("settings persistence wiring", () => {
  it("every literal updateSetting(key) has a persister or is intentionally excluded", () => {
    const root = join(process.cwd(), "src");
    const store = readFileSync(join(root, "stores/settingsStore.ts"), "utf8");

    // Keys handled by the settingUpdaters map. Scope extraction to the map block
    // so unrelated `(value) =>` arrows elsewhere in the file don't count.
    const mapStart = store.indexOf("const settingUpdaters");
    const mapEnd = store.indexOf("\n};", mapStart);
    const mapBlock = store.slice(mapStart, mapEnd);

    const handled = new Set<string>();
    for (const m of mapBlock.matchAll(/^\s+([a-z0-9_]+):\s*\(value\)\s*=>/gm)) {
      handled.add(m[1]);
    }
    // Intentionally excluded in updateSetting's fallback (handled elsewhere).
    handled.add("bindings");
    handled.add("selected_model");

    // Collect every literal-key updateSetting call across the frontend.
    const used = new Set<string>();
    for (const file of walkTsFiles(root)) {
      const src = readFileSync(file, "utf8");
      for (const m of src.matchAll(/updateSetting\(\s*["']([a-z0-9_]+)["']/g)) {
        used.add(m[1]);
      }
    }

    const orphans = [...used].filter((k) => !handled.has(k)).sort();
    expect(
      orphans,
      `settings changed in the UI but never persisted (add a settingUpdaters entry + backend command): ${orphans.join(", ")}`,
    ).toEqual([]);
  });
});
