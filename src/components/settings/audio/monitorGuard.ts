/** Peak (linear 0..1) at/above which a frame counts as clipping. */
export const CLIP_THRESHOLD = 0.99;
/** Consecutive clipping frames before auto-disabling (~500 ms at ~31 fps). */
export const TRIP_FRAMES = 16;

/**
 * Watches the input peak for a sustained clip — the signature of runaway
 * feedback — and trips once it has held for TRIP_FRAMES consecutive frames.
 * A sub-threshold frame resets the streak. Pure and framework-free.
 */
export class ClipGuard {
  private streak = 0;

  /** Feed one frame's peak; returns true exactly when the trip fires. */
  observe(peak: number): boolean {
    if (peak >= CLIP_THRESHOLD) {
      this.streak += 1;
      return this.streak >= TRIP_FRAMES;
    }
    this.streak = 0;
    return false;
  }

  reset(): void {
    this.streak = 0;
  }
}
