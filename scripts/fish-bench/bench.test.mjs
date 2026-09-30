import { describe, it, expect } from "vitest";
import { rowFor, summarizeRun } from "./bench.mjs";

describe("rowFor", () => {
  it("успешная строка несёт латентности и размер", () => {
    expect(
      rowFor({ id: "S1", mode: "batch", status: 200, ms: 1234, bytes: 9000 }),
    ).toEqual({
      id: "S1",
      mode: "batch",
      ok: true,
      status: 200,
      ms: 1234,
      ttfaMs: null,
      bytes: 9000,
      error: null,
    });
  });

  it("больное состояние (429) — ok:false, ошибка сохранена, НЕ исключение", () => {
    const r = rowFor({
      id: "S2",
      mode: "stream",
      status: 429,
      error: "rate limited",
    });
    expect(r.ok).toBe(false);
    expect(r.status).toBe(429);
    expect(r.error).toBe("rate limited");
  });
});

describe("summarizeRun", () => {
  it("считает ok/failed и не теряет строк", () => {
    const rows = [
      rowFor({ id: "S1", mode: "batch", status: 200, ms: 1, bytes: 1 }),
      rowFor({
        id: "S1",
        mode: "stream",
        status: 402,
        error: "payment required",
      }),
    ];
    const s = summarizeRun(rows);
    expect(s.ok).toBe(1);
    expect(s.failed).toBe(1);
    expect(s.rows).toHaveLength(2);
  });
});
