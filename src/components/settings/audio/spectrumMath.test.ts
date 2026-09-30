import { describe, it, expect } from "vitest";
import {
  DB_FLOOR,
  dbfsToY,
  dbfsToIntensity,
  gridlinePositions,
  shouldRepaint,
} from "./spectrumMath";

describe("dbfsToY", () => {
  it("puts 0 dBFS at the top and the floor at the bottom", () => {
    expect(dbfsToY(0, 100)).toBe(0);
    expect(dbfsToY(DB_FLOOR, 100)).toBe(100);
  });

  it("maps the midpoint to the middle", () => {
    expect(dbfsToY(-50, 100)).toBeCloseTo(50);
  });

  it("clamps out-of-range values", () => {
    expect(dbfsToY(10, 100)).toBe(0);
    expect(dbfsToY(-200, 100)).toBe(100);
  });
});

describe("dbfsToIntensity", () => {
  it("maps the floor to 0 and full scale to 1", () => {
    expect(dbfsToIntensity(DB_FLOOR)).toBe(0);
    expect(dbfsToIntensity(0)).toBe(1);
  });

  it("clamps out-of-range values", () => {
    expect(dbfsToIntensity(5)).toBe(1);
    expect(dbfsToIntensity(-150)).toBe(0);
  });
});

describe("gridlinePositions", () => {
  // A stand-in for the Rust-supplied band centers.
  const freqs = [20, 200, 2000, 20000];

  it("places a mark that sits exactly on a band center", () => {
    const [pos] = gridlinePositions(freqs, [200]);
    expect(pos.freq).toBe(200);
    expect(pos.x).toBeCloseTo(1 / 3); // index 1 of 3 spans
  });

  it("interpolates a mark between two centers", () => {
    const [pos] = gridlinePositions(freqs, [2000]);
    expect(pos.x).toBeCloseTo(2 / 3);
  });

  it("skips marks outside the range instead of drawing them off-canvas", () => {
    // A 16 kHz device's Nyquist is 8 kHz — the 10 kHz mark must not be drawn.
    const narrow = [20, 200, 2000, 8000];
    const out = gridlinePositions(narrow, [100, 1000, 10000]);
    expect(out.map((p) => p.freq)).toEqual([100, 1000]);
  });

  it("returns nothing for a degenerate array", () => {
    expect(gridlinePositions([], [1000])).toEqual([]);
    expect(gridlinePositions([440], [440])).toEqual([]);
  });
});

describe("shouldRepaint", () => {
  it("forces a repaint when dirty, even with an unchanged frame and no moving hold", () => {
    const frame = [1, 2, 3];
    expect(shouldRepaint(true, frame, frame, false)).toBe(true);
  });

  it("repaints when a new frame arrives (different array identity)", () => {
    const lastDrawn = [1, 2, 3];
    const src = [1, 2, 3]; // same values, different object identity
    expect(shouldRepaint(false, src, lastDrawn, false)).toBe(true);
  });

  it("repaints on an unchanged frame while the hold is still moving", () => {
    const frame = [1, 2, 3];
    expect(shouldRepaint(false, frame, frame, true)).toBe(true);
  });

  it("does not repaint an unchanged frame once the hold has converged or is disabled", () => {
    const frame = [1, 2, 3];
    expect(shouldRepaint(false, frame, frame, false)).toBe(false);
  });

  it("does not repaint the idle case: no frame has ever arrived", () => {
    expect(shouldRepaint(false, null, null, false)).toBe(false);
  });
});
