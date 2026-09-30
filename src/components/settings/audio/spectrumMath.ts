//! Pure presentation math for the Audio-console spectrum. No DOM/React —
//! vitest-hermetic.
//!
//! This module deliberately contains NO frequency math. Rust owns the band
//! layout (`band_centers`) and hands the centers over; here we only *search* that
//! array. If both sides computed the log layout, the two formulas could drift and
//! the drawn axis would lie about what the bars mean.
//!
//! Peak-hold is not reimplemented here — the canvases reuse `peakHold` from
//! `meterMath.ts` (Epic A), applied element-wise.

/** The dBFS floor the backend clamps every band to. */
export const DB_FLOOR = -100;

function clamp01(x: number): number {
  if (x < 0) return 0;
  if (x > 1) return 1;
  return x;
}

/** Map a dBFS value to a canvas y: 0 dBFS → 0 (top), DB_FLOOR → `height` (bottom). */
export function dbfsToY(dbfs: number, height: number): number {
  const frac = clamp01((dbfs - DB_FLOOR) / (0 - DB_FLOOR));
  return height * (1 - frac);
}

/** Map a dBFS value to a 0..1 intensity for the spectrogram's colour ramp. */
export function dbfsToIntensity(dbfs: number): number {
  return clamp01((dbfs - DB_FLOOR) / (0 - DB_FLOOR));
}

/**
 * Place frequency gridlines on the x axis by searching the Rust-supplied band
 * centers. Returns an x FRACTION (0..1) across the band-index axis — which is
 * what the canvas draws, since the bands are already log-spaced.
 *
 * Marks outside the array's range are SKIPPED rather than drawn off-canvas — e.g.
 * the 10 kHz mark on a 16 kHz device, whose Nyquist is only 8 kHz.
 */
export function gridlinePositions(
  freqs: number[],
  marks: number[],
): { freq: number; x: number }[] {
  const last = freqs.length - 1;
  if (last < 1) return [];

  const out: { freq: number; x: number }[] = [];
  for (const mark of marks) {
    if (mark < freqs[0] || mark > freqs[last]) continue;
    let i = 0;
    while (i < last && freqs[i + 1] < mark) i++;
    const lo = freqs[i];
    const hi = freqs[i + 1];
    // Linear interpolation between adjacent centers. With 128 bands the centers
    // are ~5% apart, so the error inside one span is negligible.
    const t = hi === lo ? 0 : (mark - lo) / (hi - lo);
    out.push({ freq: mark, x: (i + t) / last });
  }
  return out;
}

/**
 * Should the spectrum canvas repaint this animation frame?
 *
 * The rAF loop runs at ~60 Hz while analyzer frames arrive at ~24 Hz, so most
 * ticks would otherwise redraw an identical image. Repaint only when something
 * can actually have changed:
 *   - `dirty`      — a resize or a prop change invalidated the canvas;
 *   - a NEW frame  — `src` is a different array object than the one last drawn
 *                    (the hook assigns a fresh array per event, so identity is a
 *                    sound "new frame" signal);
 *   - `holdMoving` — the peak-hold line is still decaying, which animates
 *                    BETWEEN arriving frames and so is not redundant.
 *
 * With peak-hold off, frozen, or once the hold has converged onto a static
 * frame, an unchanged frame costs nothing.
 */
export function shouldRepaint(
  dirty: boolean,
  src: number[] | null,
  lastDrawn: number[] | null,
  holdMoving: boolean,
): boolean {
  if (dirty) return true;
  if (src !== lastDrawn) return true;
  return holdMoving;
}
