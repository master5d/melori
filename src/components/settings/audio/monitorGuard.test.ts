import { describe, it, expect } from "vitest";
import { ClipGuard, CLIP_THRESHOLD, TRIP_FRAMES } from "./monitorGuard";

describe("ClipGuard", () => {
  it("does not trip below threshold", () => {
    const g = new ClipGuard();
    let tripped = false;
    for (let i = 0; i < TRIP_FRAMES * 2; i++) {
      tripped = g.observe(CLIP_THRESHOLD - 0.01) || tripped;
    }
    expect(tripped).toBe(false);
  });

  it("trips after TRIP_FRAMES consecutive clipping frames", () => {
    const g = new ClipGuard();
    // The first TRIP_FRAMES-1 observations must not trip; the TRIP_FRAMES-th does.
    for (let i = 0; i < TRIP_FRAMES - 1; i++) {
      expect(g.observe(1.0)).toBe(false);
    }
    expect(g.observe(1.0)).toBe(true);
  });

  it("resets the run on a sub-threshold frame", () => {
    const g = new ClipGuard();
    for (let i = 0; i < TRIP_FRAMES - 1; i++) {
      g.observe(1.0);
    }
    // A quiet frame breaks the streak; the counter restarts.
    expect(g.observe(0.0)).toBe(false);
    for (let i = 0; i < TRIP_FRAMES - 1; i++) {
      expect(g.observe(1.0)).toBe(false);
    }
    expect(g.observe(1.0)).toBe(true);
  });

  it("clears its streak on reset()", () => {
    const g = new ClipGuard();
    for (let i = 0; i < TRIP_FRAMES - 1; i++) {
      g.observe(1.0);
    }
    g.reset();
    expect(g.observe(1.0)).toBe(false);
  });
});
