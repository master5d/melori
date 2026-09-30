import { describe, expect, it } from "vitest";
import {
  withGeneratedRecap,
  highlightParts,
  parseRefs,
  resolveRef,
  tabForHit,
  indexLine,
  daysAgoLine,
} from "./memoryLogic";

describe("memory logic", () => {
  it("splits highlighted snippet", () => {
    expect(highlightParts("про [[сон]] и режим")).toEqual([
      { text: "про ", hit: false },
      { text: "сон", hit: true },
      { text: " и режим", hit: false },
    ]);
  });
  it("parses date refs in an answer", () => {
    expect(parseRefs("Тема сна [2026-09-12], потом [2026-09-19].")).toEqual([
      { text: "Тема сна " },
      { date: "2026-09-12" },
      { text: ", потом " },
      { date: "2026-09-19" },
      { text: "." },
    ]);
  });
  it("resolves a ref to the first session of the day, or null if deleted", () => {
    const s = [
      { id: "2026-09-12-02", date: "2026-09-12" },
      { id: "2026-09-12-01", date: "2026-09-12" },
    ];
    expect(resolveRef("2026-09-12", s)).toBe("2026-09-12-01");
    expect(resolveRef("2026-08-01", s)).toBeNull();
  });
  it("opens the right tab for a hit", () => {
    expect(tabForHit("transcript")).toBe("transcript");
    expect(tabForHit("note")).toBe("notes");
    expect(tabForHit("email")).toBe("email");
  });
  it("formats index and age lines", () => {
    const t = (key: string, options?: Record<string, unknown>) =>
      `${key}:${JSON.stringify(options ?? {})}`;
    expect(
      indexLine(
        {
          sessions: 2,
          chunks: 12,
          vectors_missing: 4,
          meaning_available: false,
          meaning_error: null,
        },
        t,
      ),
    ).toContain("consult.memory.indexing");
    expect(
      indexLine(
        {
          sessions: 2,
          chunks: 12,
          vectors_missing: 0,
          meaning_available: false,
          meaning_error: "offline",
        },
        t,
      ),
    ).toContain("consult.memory.meaningUnavailable");
    expect(daysAgoLine(0, t)).toContain("consult.memory.today");
    expect(daysAgoLine(12, t)).toContain('"count":12');
  });
});

describe("withGeneratedRecap", () => {
  it("puts the generated recap where the block reads it and keeps the quick recap", () => {
    const quick = {
      session_id: "s",
      date: "2026-09-12",
      days_ago: 3,
      plan: null,
      email_subject: null,
    };
    const recap = {
      created: "c",
      sessions: ["s"],
      points: ["one"],
      provenance: null,
    };
    const merged = withGeneratedRecap(
      { quick, model: null, model_stale: true } as never,
      { recap, model_stale: false } as never,
    );
    expect(merged).toEqual({ quick, model: recap, model_stale: false });
  });
});
