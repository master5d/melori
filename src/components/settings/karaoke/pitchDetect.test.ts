import { describe, it, expect } from "vitest";
import {
  detectPitch,
  hzToNoteName,
  centsOff,
  octaveFoldedCents,
  octaveFoldedInBand,
} from "./pitchDetect";

/** Generate `durSec` of a pure sine at `hz` sampled at `sampleRate`. */
function sine(hz: number, sampleRate: number, durSec: number): Float32Array {
  const n = Math.floor(sampleRate * durSec);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++)
    out[i] = Math.sin((2 * Math.PI * hz * i) / sampleRate);
  return out;
}

describe("detectPitch", () => {
  it("detects a 220 Hz sine within 3 cents", () => {
    const hz = detectPitch(sine(220, 44100, 0.05), 44100);
    expect(hz).not.toBeNull();
    expect(Math.abs(centsOff(hz!, 220))).toBeLessThan(3);
  });
  it("detects a 440 Hz sine within 3 cents", () => {
    const hz = detectPitch(sine(440, 44100, 0.05), 44100);
    expect(hz).not.toBeNull();
    expect(Math.abs(centsOff(hz!, 440))).toBeLessThan(3);
  });
  it("does not report a 220 Hz tone as an octave (110/440)", () => {
    const hz = detectPitch(sine(220, 44100, 0.05), 44100)!;
    expect(Math.abs(centsOff(hz, 110))).toBeGreaterThan(50);
    expect(Math.abs(centsOff(hz, 440))).toBeGreaterThan(50);
  });
  it("returns null for silence", () => {
    expect(detectPitch(new Float32Array(2048), 44100)).toBeNull();
  });
});

describe("note/cents helpers", () => {
  it("names A4 = 440 Hz", () => {
    expect(hzToNoteName(440)).toBe("A4");
  });
  it("centsOff is signed and ~0 at unison", () => {
    expect(Math.abs(centsOff(440, 440))).toBeLessThan(1e-6);
    expect(centsOff(880, 440)).toBeCloseTo(1200, 0);
  });
  it("octaveFoldedCents folds octaves to 0", () => {
    expect(octaveFoldedCents(261.63, 130.81)).toBeLessThan(2); // C4 vs C3 = same class
    expect(octaveFoldedCents(130.81, 130.81)).toBeLessThan(1e-6);
  });
  it("octaveFoldedInBand accepts an octave-away match within tolerance", () => {
    expect(octaveFoldedInBand(261.63, 130.81, 50)).toBe(true); // C4 vs C3 tonic
    expect(octaveFoldedInBand(146.83, 130.81, 50)).toBe(false); // D3 vs C3 ~ 200c off
  });
});
