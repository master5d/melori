import { describe, it, expect } from "vitest";
import {
  scoreInPitch,
  builtinTargetHz,
  IN_BAND_CENTS,
  type PitchSample,
} from "./pitchScore";
import { analyzeReferenceContour, contourHzAt } from "./referencePitch";

function sine(hz: number, sampleRate: number, durSec: number): Float32Array {
  const n = Math.floor(sampleRate * durSec);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++)
    out[i] = Math.sin((2 * Math.PI * hz * i) / sampleRate);
  return out;
}

describe("scoreInPitch", () => {
  const T = builtinTargetHz(); // 130.81 (C3)
  it("scores all-in-band samples as 100%", () => {
    const s: PitchSample[] = [
      { t: 0, hz: T, targetHz: T },
      { t: 1, hz: T * 2, targetHz: T }, // octave up = still in class
    ];
    expect(scoreInPitch(s).percentInPitch).toBe(100);
  });
  it("excludes unvoiced (null hz) frames from the denominator", () => {
    const s: PitchSample[] = [
      { t: 0, hz: T, targetHz: T },
      { t: 1, hz: null, targetHz: T }, // unvoiced — ignored
    ];
    const r = scoreInPitch(s);
    expect(r.voicedFrames).toBe(1);
    expect(r.percentInPitch).toBe(100);
    expect(r.totalFrames).toBe(2);
  });
  it("scores a half-off take as 50%", () => {
    const off = 146.83; // D3, ~200 cents off C3
    const s: PitchSample[] = [
      { t: 0, hz: T, targetHz: T },
      { t: 1, hz: off, targetHz: T },
    ];
    expect(scoreInPitch(s).percentInPitch).toBe(50);
  });
  it("returns 0% with no voiced frames", () => {
    expect(scoreInPitch([{ t: 0, hz: null, targetHz: T }]).percentInPitch).toBe(
      0,
    );
  });
  it("exposes the ±50 cent tolerance constant", () => {
    expect(IN_BAND_CENTS).toBe(50);
  });
});

describe("analyzeReferenceContour", () => {
  it("extracts a ~330 Hz contour from a solo sine reference", () => {
    const sr = 44100;
    const contour = analyzeReferenceContour(sine(330, sr, 0.3), sr, 0.05, 0.05);
    expect(contour.length).toBeGreaterThan(2);
    const mid = contour[Math.floor(contour.length / 2)];
    expect(Math.abs(1200 * Math.log2(mid.hz / 330))).toBeLessThan(10);
  });
  it("contourHzAt returns the nearest point in time", () => {
    const contour = [
      { t: 0, hz: 200 },
      { t: 1, hz: 400 },
    ];
    expect(contourHzAt(contour, 0.1)).toBe(200);
    expect(contourHzAt(contour, 0.9)).toBe(400);
    expect(contourHzAt([], 0.5)).toBeNull();
  });
});
