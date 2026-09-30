import { describe, expect, it } from "vitest";
import {
  barsFromBands,
  formatTimer,
  isCancellable,
  pillVariant,
  type OverlayState,
  type PillVariant,
} from "./pillLogic";

describe("pillVariant", () => {
  const cases: Array<[OverlayState, string]> = [
    ["recording", "listening"],
    ["transcribing", "processing"],
    ["processing", "processing"],
    ["preparing", "loading"],
  ];

  for (const [state, expected] of cases) {
    it(`maps ${state} -> ${expected}`, () => {
      expect(pillVariant(state)).toBe(expected);
    });
  }
});

describe("isCancellable", () => {
  // Matches the current cancel-button semantics in RecordingOverlay.tsx
  // (canCancel = isRecording || isPreparing, i.e. state "recording" or
  // "preparing" -> variant "listening"/"loading"). "processing" is
  // intentionally NOT cancellable today — don't broaden that here.
  const cases: Array<[PillVariant, boolean]> = [
    ["idle", false],
    ["listening", true],
    ["processing", false],
    ["loading", true],
  ];

  for (const [variant, expected] of cases) {
    it(`${variant} -> ${expected}`, () => {
      expect(isCancellable(variant)).toBe(expected);
    });
  }
});

describe("pill state regression", () => {
  const stateCases: Array<[OverlayState, PillVariant]> = [
    ["preparing", "loading"],
    ["recording", "listening"],
    ["transcribing", "processing"],
    ["processing", "processing"],
  ];

  for (const [state, expected] of stateCases) {
    it(`keeps ${state} -> ${expected}`, () => {
      expect(pillVariant(state)).toBe(expected);
    });
  }

  const cancellableCases: Array<[PillVariant, boolean]> = [
    ["idle", false],
    ["listening", true],
    ["processing", false],
    ["loading", true],
  ];

  for (const [variant, expected] of cancellableCases) {
    it(`keeps cancellability ${variant} -> ${expected}`, () => {
      expect(isCancellable(variant)).toBe(expected);
    });
  }
});

describe("formatTimer", () => {
  it("formats 0ms", () => {
    expect(formatTimer(0)).toBe("0:00.0");
  });

  it("formats sub-minute", () => {
    expect(formatTimer(7200)).toBe("0:07.2");
  });

  it("formats over a minute", () => {
    expect(formatTimer(65400)).toBe("1:05.4");
  });
});

describe("barsFromBands", () => {
  const MIN = 6;
  const MAX = 34;

  it("returns count values all at MIN when bands is empty", () => {
    const bars = barsFromBands([], 14);
    expect(bars).toHaveLength(14);
    for (const b of bars) expect(b).toBe(MIN);
  });

  it("returns count finite values within [MIN, MAX] for a real spectrum", () => {
    const bands = [
      0.9, 0.8, 0.1, 0.05, 0.6, 0.95, 0.2, 0.4, 0.7, 0.3, 0.5, 0.15, 0.85, 0.25,
      0.6, 0.1,
    ];
    const bars = barsFromBands(bands, 14);
    expect(bars).toHaveLength(14);
    for (const b of bars) {
      expect(Number.isFinite(b)).toBe(true);
      expect(b).toBeGreaterThanOrEqual(MIN);
      expect(b).toBeLessThanOrEqual(MAX);
    }
  });

  it("scales up with louder input (monotonic-ish)", () => {
    const quiet = barsFromBands(new Array(14).fill(0.05), 14);
    const loud = barsFromBands(new Array(14).fill(0.95), 14);
    for (let i = 0; i < 14; i++) {
      expect(loud[i]).toBeGreaterThan(quiet[i]);
    }
  });
});
