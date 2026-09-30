//! Pure offline reference-contour extraction (custom uploads). Takes raw PCM
//! (channel data + sample rate) so it is node-testable without an AudioBuffer.
import { detectPitch } from "./pitchDetect";

export interface ContourPoint {
  t: number;
  hz: number;
}

/**
 * Slide a window over the reference PCM at `hopSec` steps, detect pitch per
 * window, and collect the voiced points. Best-effort and monophonic — reliable
 * for a solo chant/voice reference, not for polyphonic mixes.
 */
export function analyzeReferenceContour(
  channel: Float32Array,
  sampleRate: number,
  hopSec = 0.05,
  winSec = 0.05,
): ContourPoint[] {
  const hop = Math.max(1, Math.floor(sampleRate * hopSec));
  const win = Math.max(256, Math.floor(sampleRate * winSec));
  const out: ContourPoint[] = [];
  for (let start = 0; start + win <= channel.length; start += hop) {
    const hz = detectPitch(channel.subarray(start, start + win), sampleRate);
    if (hz != null) out.push({ t: start / sampleRate, hz });
  }
  return out;
}

/** Nearest-in-time contour value, or null when the contour is empty. */
export function contourHzAt(contour: ContourPoint[], t: number): number | null {
  if (contour.length === 0) return null;
  let best = contour[0];
  let bestD = Math.abs(contour[0].t - t);
  for (const p of contour) {
    const d = Math.abs(p.t - t);
    if (d < bestD) {
      bestD = d;
      best = p;
    }
  }
  return best.hz;
}
