//! Pure scoring for the Karaoke sing-along. No React/DOM.
import { octaveFoldedInBand } from "./pitchDetect";

/** In-band tolerance (± cents) — the standard "in tune" window for singing. */
export const IN_BAND_CENTS = 50;

/** Built-in drone tonic pitch class expressed as C3 (the synth's C triad). */
export const BUILTIN_TARGET_HZ = 130.81;

export interface PitchSample {
  t: number;
  hz: number | null;
  targetHz: number | null;
}

export interface PitchScore {
  percentInPitch: number;
  voicedFrames: number;
  totalFrames: number;
}

/** The analytic built-in target Hz (octave-tolerant pitch class C). */
export function builtinTargetHz(): number {
  return BUILTIN_TARGET_HZ;
}

/**
 * Percent of voiced frames whose detected pitch is within IN_BAND_CENTS of the
 * target pitch class (octave-folded). Unvoiced frames (null hz or null target)
 * are excluded from the denominator.
 */
export function scoreInPitch(samples: PitchSample[]): PitchScore {
  let voiced = 0;
  let inBand = 0;
  for (const s of samples) {
    if (s.hz == null || s.targetHz == null) continue;
    voiced++;
    if (octaveFoldedInBand(s.hz, s.targetHz, IN_BAND_CENTS)) inBand++;
  }
  return {
    percentInPitch: voiced === 0 ? 0 : Math.round((inBand / voiced) * 100),
    voicedFrames: voiced,
    totalFrames: samples.length,
  };
}
