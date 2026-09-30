import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const root = process.cwd();
const tokens = JSON.parse(
  readFileSync(join(root, "design/tokens.json"), "utf8"),
);

describe("design tokens", () => {
  it("generated css files are up to date with tokens.json", () => {
    expect(() =>
      execFileSync("node", ["scripts/build-tokens.mjs", "--check"], {
        cwd: root,
      }),
    ).not.toThrow();
  });
  it("every role has a day and an evening value", () => {
    expect(Object.keys(tokens.color.day).sort()).toEqual(
      Object.keys(tokens.color.evening).sort(),
    );
  });
  it("every declared contrast pair passes in both themes", () => {
    expect(() =>
      execFileSync("node", ["scripts/check-contrast.mjs"], { cwd: root }),
    ).not.toThrow();
  });
  it("the three webviews get the same variables", () => {
    const names = (f: string) =>
      [
        ...readFileSync(join(root, f), "utf8").matchAll(/--color-([a-z-]+):/g),
      ].map((m) => m[1]);
    const main = new Set(names("src/theme/tokens.css"));
    for (const f of ["src/meeting/tokens.css", "src/overlay/tokens.css"]) {
      for (const n of names(f)) expect(main.has(n)).toBe(true);
    }
  });
  it("no raw colors outside generated tokens files", () => {
    let out = "";
    try {
      out = execFileSync(
        "git",
        [
          "grep",
          "-nIE",
          "#[0-9a-fA-F]{6}\\b|oklch\\(|rgba?\\(",
          "--",
          "src/*.css",
          "src/**/*.css",
          ":!src/theme/tokens.css",
          ":!src/meeting/tokens.css",
          ":!src/overlay/tokens.css",
        ],
        { cwd: root },
      ).toString();
    } catch (error: any) {
      if (error.status !== 1) throw error;
    }
    expect(out).toBe("");
  });
});
