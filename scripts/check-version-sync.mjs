#!/usr/bin/env node
// Версия приложения живёт в трёх файлах и должна оставаться
// в синхроне. Требование ничем не проверялось, и к 2026-08-21 они разъехались:
// package.json 1.1.7 против src-tauri/Cargo.toml и tauri.conf.json 1.1.8.
//
// Молчаливость этого дрейфа — не мелочь. Из tauri.conf.json версию берёт CI
// (`Get version from tauri.conf.json` в build.yml) и она же уходит в latest.json
// апдейтера, а package.json — то, что видит фронт и любой npm-инструмент. Разойдясь,
// они дают релиз, где имя артефакта и то, что о себе думает приложение, — разные
// числа, и заметить это можно только руками в момент выпуска.
//
// Прогон: node scripts/check-version-sync.mjs   (и шагом в code-quality.yml)
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Cargo.toml читается регуляркой намеренно: тянуть toml-парсер в скрипт, который
// нужен один раз за коммит, дороже, чем якорь на `[package]`. Якорь обязателен —
// в файле есть и `[workspace.package]`, и версии зависимостей.
function cargoVersion(text) {
  const pkg = text.split(/^\[/m).find((s) => s.startsWith("package]"));
  if (!pkg) return null;
  const m = pkg.match(/^\s*version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

const sources = [
  {
    file: "package.json",
    version: JSON.parse(readFileSync(join(root, "package.json"), "utf8"))
      .version,
  },
  {
    file: "src-tauri/tauri.conf.json",
    version: JSON.parse(
      readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
    ).version,
  },
  {
    file: "src-tauri/Cargo.toml",
    version: cargoVersion(
      readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8"),
    ),
  },
];

const missing = sources.filter((s) => !s.version);
if (missing.length) {
  // Не найденное поле — это ПОЛОМКА проверки, а не «версии совпали». Ровно этот
  // класс (пустое множество читается как «всё ровно») и делает гейты ложно-зелёными.
  console.error("✗ Версию не удалось прочитать — проверка НЕ состоялась:");
  for (const s of missing) console.error(`  ${s.file}`);
  process.exit(1);
}

const unique = [...new Set(sources.map((s) => s.version))];
if (unique.length > 1) {
  console.error("✗ Версия разъехалась по файлам:");
  for (const s of sources) console.error(`  ${s.version.padEnd(12)} ${s.file}`);
  console.error(
    "\n  Все три файла версии обязаны совпадать. Приведи их к одной и повтори.",
  );
  process.exit(1);
}

console.log(`✓ Версия ${unique[0]} — одинакова во всех трёх файлах`);
