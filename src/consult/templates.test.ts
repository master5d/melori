import { describe, expect, it } from "vitest";
import { slugKey, validateTemplateDraft } from "./templates";
describe("template helpers", () => {
  it("transliterates and numbers empty keys", () => {
    expect(slugKey("Субъективно")).toBe("subektivno");
    expect(slugKey("!!!", 3)).toBe("section_3");
  });
  it("validates", () => {
    expect(validateTemplateDraft({ name: "", sections: [] })).toEqual([
      "name",
      "empty",
    ]);
    expect(
      validateTemplateDraft({
        name: "x",
        sections: [{ key: "Bad Key", title: "", guidance: "x".repeat(401) }],
      }),
    ).toEqual(["key:0", "title:0", "guidance:0"]);
  });
  it("finds duplicates and count", () => {
    const sections = Array.from({ length: 13 }, (_, i) => ({
      key: i < 2 ? "same" : `k${i}`,
      title: "x",
      guidance: "",
    }));
    const e = validateTemplateDraft({ name: "x", sections });
    expect(e).toContain("tooMany");
    expect(e).toContain("duplicate:1");
  });
});
