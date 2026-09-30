import { describe, it, expect } from "vitest";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadFishKey } from "./env.mjs";

describe("loadFishKey", () => {
  it("берёт FISH_API_KEY из env первым", () => {
    expect(
      loadFishKey({
        env: { FISH_API_KEY: "from-env" },
        envFilePath: "C:/nonexistent/.env.fish",
      }),
    ).toBe("from-env");
  });

  it("падает обратно на .env.fish (формат KEY=value, CRLF терпит)", () => {
    const dir = mkdtempSync(join(tmpdir(), "fishbench-"));
    const p = join(dir, ".env.fish");
    writeFileSync(p, "FISH_API_KEY=from-file\r\n");
    expect(loadFishKey({ env: {}, envFilePath: p })).toBe("from-file");
  });

  it("без ключа бросает ошибку, не раскрывая содержимого", () => {
    const dir = mkdtempSync(join(tmpdir(), "fishbench-"));
    const p = join(dir, ".env.fish");
    writeFileSync(p, "OTHER=secret-stuff\n");
    expect(() => loadFishKey({ env: {}, envFilePath: p })).toThrow(
      /FISH_API_KEY not found/,
    );
    try {
      loadFishKey({ env: {}, envFilePath: p });
    } catch (e) {
      expect(e.message).not.toContain("secret-stuff");
    }
  });
});
