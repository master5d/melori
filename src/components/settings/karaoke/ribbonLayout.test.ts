import { describe, it, expect } from "vitest";
import {
  hzToY,
  timeToX,
  targetOctaveHzs,
  PITCH_MIN_HZ,
  PITCH_MAX_HZ,
} from "./ribbonLayout";

describe("hzToY", () => {
  it("maps the max frequency to the top (y=0) and min to the bottom", () => {
    expect(hzToY(PITCH_MAX_HZ, 100)).toBeCloseTo(0, 5);
    expect(hzToY(PITCH_MIN_HZ, 100)).toBeCloseTo(100, 5);
  });
  it("is monotonic decreasing in hz", () => {
    expect(hzToY(200, 100)).toBeGreaterThan(hzToY(400, 100));
  });
  it("clamps out-of-range input", () => {
    expect(hzToY(10, 100)).toBe(100);
    expect(hzToY(5000, 100)).toBe(0);
  });
});

describe("timeToX", () => {
  it("puts 'now' at the right edge and the window start at the left", () => {
    expect(timeToX(10, 10, 4, 400)).toBeCloseTo(400, 5);
    expect(timeToX(6, 10, 4, 400)).toBeCloseTo(0, 5);
    expect(timeToX(8, 10, 4, 400)).toBeCloseTo(200, 5);
  });
});

describe("targetOctaveHzs", () => {
  it("returns every octave of the target within the pitch range", () => {
    const hzs = targetOctaveHzs(130.81); // C3
    // expect ~C2(65.4 out of range low), C3(130.8), C4(261.6), C5(523.2)
    expect(hzs.some((h) => Math.abs(h - 130.81) < 1)).toBe(true);
    expect(hzs.some((h) => Math.abs(h - 261.63) < 1)).toBe(true);
    expect(hzs.some((h) => Math.abs(h - 523.25) < 1)).toBe(true);
    expect(hzs.every((h) => h >= PITCH_MIN_HZ && h <= PITCH_MAX_HZ)).toBe(true);
  });
});
