import { describe, it, expect } from "vitest";
import { ECHO_MODULES, SUITE_MODULES, SYSTEM_SECTIONS } from "./modules";
import { SECTIONS_CONFIG } from "../components/Sidebar";

describe("shell module registry", () => {
  it("covers every SECTIONS_CONFIG section exactly once, split across modules + system sections", () => {
    const allSectionIds = Object.keys(SECTIONS_CONFIG).sort();

    const moduleSections = [...SUITE_MODULES, ...ECHO_MODULES].flatMap(
      (m) => m.sections,
    );
    const combined = [...moduleSections, ...SYSTEM_SECTIONS];

    const seen = new Set<string>();
    const duplicates: string[] = [];
    for (const id of combined) {
      if (seen.has(id)) duplicates.push(id);
      seen.add(id);
    }

    expect(
      duplicates,
      "no section should appear in more than one module/system list",
    ).toEqual([]);
    expect(
      [...seen].sort(),
      "union(modules.sections, SYSTEM_SECTIONS) must equal the full SECTIONS_CONFIG id set",
    ).toEqual(allSectionIds);
  });

  it("exposes only the consult module", () => {
    const ids = SUITE_MODULES.map((m) => m.id);
    expect(ids).toEqual(["consult"]);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("keeps the six inherited echo modules outside navigation", () => {
    expect(ECHO_MODULES.map((m) => m.id)).toEqual([
      "dictate",
      "assistant",
      "vocal",
      "language",
      "clone",
      "studio",
    ]);
  });

  it("every module carries an icon and a titleKey", () => {
    for (const mod of SUITE_MODULES) {
      expect(mod.icon, `${mod.id} is missing an icon`).toBeTruthy();
      expect(mod.titleKey, `${mod.id} is missing a titleKey`).toMatch(
        /^shell\.modules\./,
      );
    }
  });
});
