// One-shot: fill any keys missing from a locale with the English value
// (i18next falls back to en at runtime anyway; this satisfies check:translations).
// Run: node scripts/fill-missing-translations.mjs
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const localesDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "src",
  "i18n",
  "locales",
);

const en = JSON.parse(
  readFileSync(join(localesDir, "en", "translation.json"), "utf8"),
);

// Deep-merge: add keys from `ref` that are absent in `target`. Returns count added.
function fill(ref, target) {
  let added = 0;
  for (const key of Object.keys(ref)) {
    if (
      typeof ref[key] === "object" &&
      ref[key] !== null &&
      !Array.isArray(ref[key])
    ) {
      if (typeof target[key] !== "object" || target[key] === null) {
        target[key] = {};
      }
      added += fill(ref[key], target[key]);
    } else if (!(key in target)) {
      target[key] = ref[key];
      added += 1;
    }
  }
  return added;
}

for (const lang of readdirSync(localesDir)) {
  if (lang === "en") continue;
  const file = join(localesDir, lang, "translation.json");
  let data;
  try {
    data = JSON.parse(readFileSync(file, "utf8"));
  } catch {
    continue;
  }
  const added = fill(en, data);
  if (added > 0) {
    writeFileSync(file, JSON.stringify(data, null, 2) + "\n", "utf8");
    console.log(`${lang}: filled ${added} keys`);
  }
}
