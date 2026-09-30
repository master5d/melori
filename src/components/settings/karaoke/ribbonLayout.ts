//! Pure geometry for the horizontal pitch ribbon. No React/DOM.

export const PITCH_MIN_HZ = 80; // Y-axis floor (~E2)
export const PITCH_MAX_HZ = 660; // Y-axis ceiling (~E5) — covers chant + octaves
export const RIBBON_WINDOW_SEC = 4; // seconds of history visible

/** Log-frequency Y: higher pitch → smaller y (top). Clamped to [0, height]. */
export function hzToY(
  hz: number,
  height: number,
  minHz = PITCH_MIN_HZ,
  maxHz = PITCH_MAX_HZ,
): number {
  const lo = Math.log2(minHz);
  const hi = Math.log2(maxHz);
  const f = (Math.log2(hz) - lo) / (hi - lo);
  const clamped = Math.min(1, Math.max(0, f));
  return height * (1 - clamped);
}

/** X for a timestamp: `now` at the right edge, `now - windowSec` at the left. */
export function timeToX(
  t: number,
  now: number,
  windowSec: number,
  width: number,
): number {
  const f = (t - (now - windowSec)) / windowSec;
  return width * Math.min(1, Math.max(0, f));
}

/** Every octave of the target pitch class within [minHz, maxHz]. */
export function targetOctaveHzs(
  targetHz: number,
  minHz = PITCH_MIN_HZ,
  maxHz = PITCH_MAX_HZ,
): number[] {
  let h = targetHz;
  while (h > minHz) h /= 2; // drop below the floor
  const out: number[] = [];
  for (h *= 2; h <= maxHz; h *= 2) if (h >= minHz) out.push(h);
  return out;
}
