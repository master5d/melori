//! Pure presentation math for the Audio-console input meter. No DOM/React —
//! vitest-hermetic. Mirrors the Rust `linear_to_dbfs` (echo-audio/meter.rs);
//! the backend emits linear peak/rms and this maps them to meter pixels.

/** Convert a linear magnitude to dBFS (`20*log10`), floored at -100. `x<=0` → -100. */
export function linearToDbfs(x: number): number {
  if (x <= 0) return -100;
  const db = 20 * Math.log10(x);
  return db < -100 ? -100 : db;
}

/** Map a dBFS value in `[floor, 0]` to a `[0, 1]` meter fraction, clamped. */
export function dbfsToFraction(db: number, floor = -60): number {
  const frac = (db - floor) / (0 - floor);
  if (frac < 0) return 0;
  if (frac > 1) return 1;
  return frac;
}

/** True when the linear peak is within ~0.03 dB of full scale. */
export function isClipping(peakLinear: number): boolean {
  return peakLinear >= 0.997;
}

/**
 * Peak-hold with linear-in-dB decay: hold the higher of the current dBFS and
 * the previous hold decayed by `decayDb`. `decayDb` is per-tick.
 */
export function peakHold(
  prevHoldDb: number,
  curDb: number,
  decayDb: number,
): number {
  const decayed = prevHoldDb - decayDb;
  return curDb > decayed ? curDb : decayed;
}
