import { describe, it, expect } from "vitest";
import {
  recToTransport,
  formatTimecode,
  meterLit,
  visibleMicBands,
  EMPTY_BANDS,
} from "./audioTransport";

describe("recToTransport", () => {
  it("recording -> recording", () => {
    expect(recToTransport("recording")).toBe("recording");
  });

  it("transcribing -> processing", () => {
    expect(recToTransport("transcribing")).toBe("processing");
  });

  it("processing -> processing", () => {
    expect(recToTransport("processing")).toBe("processing");
  });

  it("preparing -> processing", () => {
    expect(recToTransport("preparing")).toBe("processing");
  });

  it("idle -> idle", () => {
    expect(recToTransport("idle")).toBe("idle");
  });
});

describe("formatTimecode", () => {
  it("0ms -> 0:00", () => {
    expect(formatTimecode(0)).toBe("0:00");
  });

  it("65000ms -> 1:05", () => {
    expect(formatTimecode(65000)).toBe("1:05");
  });

  it("clamps NaN to 0:00", () => {
    expect(formatTimecode(NaN)).toBe("0:00");
  });

  it("clamps negative to 0:00", () => {
    expect(formatTimecode(-500)).toBe("0:00");
  });

  it("pads seconds under 10", () => {
    expect(formatTimecode(9000)).toBe("0:09");
  });

  it("handles minutes >= 10", () => {
    expect(formatTimecode(605000)).toBe("10:05");
  });
});

describe("meterLit", () => {
  it("empty bands -> 0 lit", () => {
    expect(meterLit([], 10)).toBe(0);
  });

  it("all-max bands -> all segments lit", () => {
    expect(meterLit(new Array(10).fill(1), 10)).toBe(10);
  });

  it("mid-level bands light roughly half", () => {
    const lit = meterLit(new Array(10).fill(0.5), 10);
    expect(lit).toBeGreaterThan(0);
    expect(lit).toBeLessThan(10);
  });

  it("is monotonic-ish: higher average lights >= segments than lower average", () => {
    const low = meterLit(new Array(10).fill(0.2), 10);
    const high = meterLit(new Array(10).fill(0.8), 10);
    expect(high).toBeGreaterThanOrEqual(low);
  });

  it("never returns NaN", () => {
    expect(Number.isNaN(meterLit([NaN, NaN], 10))).toBe(false);
  });

  it("clamps result to [0, segments] even with out-of-range values", () => {
    const lit = meterLit([5, 5, 5], 10);
    expect(lit).toBeGreaterThanOrEqual(0);
    expect(lit).toBeLessThanOrEqual(10);
  });

  it("clamps negative values to 0 contribution", () => {
    const lit = meterLit([-5, -5], 10);
    expect(lit).toBe(0);
  });

  it("zero segments returns 0", () => {
    expect(meterLit([1, 1, 1], 0)).toBe(0);
  });
});

describe("visibleMicBands", () => {
  it("recording -> passes through the live bands", () => {
    const live = [0.1, 0.5, 0.9];
    expect(visibleMicBands("recording", live)).toBe(live);
  });

  it("idle -> forced to EMPTY_BANDS even with a stale live spectrum", () => {
    expect(visibleMicBands("idle", [0.9, 0.9, 0.9])).toBe(EMPTY_BANDS);
  });

  it("processing -> forced to EMPTY_BANDS", () => {
    expect(visibleMicBands("processing", [0.5])).toBe(EMPTY_BANDS);
  });

  it("transcribing -> forced to EMPTY_BANDS", () => {
    expect(visibleMicBands("transcribing", [0.5])).toBe(EMPTY_BANDS);
  });

  it("preparing -> forced to EMPTY_BANDS", () => {
    expect(visibleMicBands("preparing", [0.5])).toBe(EMPTY_BANDS);
  });
});
