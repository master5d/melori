import { describe, it, expect } from "vitest";
import { paletteKbdHint, systemSectionIds } from "./shellUx";

describe("paletteKbdHint", () => {
  it("returns the ⌘K glyph on macOS", () => {
    expect(paletteKbdHint(true)).toBe("⌘K");
  });

  it("returns Ctrl+K everywhere else", () => {
    expect(paletteKbdHint(false)).toBe("Ctrl+K");
  });
});

describe("systemSectionIds", () => {
  it("returns ids present in allSectionIds but in none of moduleSections, order-preserving", () => {
    expect(
      systemSectionIds(
        ["general", "advanced", "models", "debug", "about", "postprocessing"],
        [["general", "models"]],
      ),
    ).toEqual(["advanced", "debug", "about", "postprocessing"]);
  });

  it("returns all ids when moduleSections is empty", () => {
    expect(systemSectionIds(["general", "advanced", "models"], [])).toEqual([
      "general",
      "advanced",
      "models",
    ]);
  });

  it("excludes a section listed in any module", () => {
    expect(
      systemSectionIds(
        ["general", "advanced", "models"],
        [["general"], ["models"]],
      ),
    ).toEqual(["advanced"]);
  });
});
