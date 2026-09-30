import { describe, it, expect } from "vitest";
import {
  decideMount,
  appendRow,
  sourceMode,
  formatElapsed,
  type Row,
} from "./copilotLogic";

describe("decideMount", () => {
  it("re-attaches when a meeting is already active (never restarts)", () => {
    const d = decideMount({ active: true, loopback_active: true }, false);
    expect(d.action).toBe("reattach");
    expect(d.loopbackActive).toBe(true);
  });
  it("shows the consent gate when inactive and consent not yet acked", () => {
    const d = decideMount({ active: false, loopback_active: false }, false);
    expect(d.action).toBe("consent");
  });
  it("starts immediately when inactive and consent already acked", () => {
    const d = decideMount({ active: false, loopback_active: false }, true);
    expect(d.action).toBe("start");
  });
});

describe("appendRow", () => {
  it("appends a row preserving order", () => {
    const rows: Row[] = [{ id: 1, source: "me", text: "hi" }];
    const next = appendRow(rows, { id: 2, source: "others", text: "yo" }, 100);
    expect(next.map((r) => r.id)).toEqual([1, 2]);
    expect(next[1].source).toBe("others");
  });
  it("caps to the most recent `cap` rows", () => {
    let rows: Row[] = [];
    for (let i = 1; i <= 5; i++) {
      rows = appendRow(rows, { id: i, source: "me", text: `${i}` }, 3);
    }
    expect(rows.map((r) => r.id)).toEqual([3, 4, 5]);
  });
});

describe("sourceMode", () => {
  it("reports both when loopback is active", () => {
    expect(sourceMode(true)).toBe("both");
  });
  it("reports mic-only when loopback is inactive", () => {
    expect(sourceMode(false)).toBe("micOnly");
  });
});

describe("formatElapsed", () => {
  it("formats mm:ss with zero-padded seconds", () => {
    expect(formatElapsed(134_000)).toBe("2:14");
  });
  it("clamps negative input to 0:00", () => {
    expect(formatElapsed(-5)).toBe("0:00");
  });
});
