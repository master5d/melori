import { describe, it, expect } from "vitest";
import {
  linearToDbfs,
  dbfsToFraction,
  isClipping,
  peakHold,
} from "./meterMath";

describe("linearToDbfs", () => {
  it("maps reference points", () => {
    expect(linearToDbfs(1)).toBeCloseTo(0, 4);
    expect(linearToDbfs(0.5)).toBeCloseTo(-6.02, 1);
    expect(linearToDbfs(0)).toBe(-100);
    expect(linearToDbfs(-0.2)).toBe(-100);
  });
});

describe("dbfsToFraction", () => {
  it("maps floor..0 to 0..1 and clamps", () => {
    expect(dbfsToFraction(0)).toBeCloseTo(1, 5);
    expect(dbfsToFraction(-60)).toBeCloseTo(0, 5);
    expect(dbfsToFraction(-30)).toBeCloseTo(0.5, 5);
    expect(dbfsToFraction(10)).toBe(1); // clamps above 0
    expect(dbfsToFraction(-120)).toBe(0); // clamps below floor
  });

  it("honors a custom floor", () => {
    expect(dbfsToFraction(-20, -40)).toBeCloseTo(0.5, 5);
  });
});

describe("isClipping", () => {
  it("flags near-full-scale peaks", () => {
    expect(isClipping(1.0)).toBe(true);
    expect(isClipping(0.997)).toBe(true);
    expect(isClipping(0.9)).toBe(false);
  });
});

describe("peakHold", () => {
  it("holds the max and decays each tick", () => {
    expect(peakHold(-30, -10, 3)).toBe(-10); // new peak wins
    expect(peakHold(-10, -25, 3)).toBe(-13); // decays from held
    expect(peakHold(-10, -12, 3)).toBe(-12); // current above decayed-hold
  });
});
