//! Pure Web-Audio-free pitch DSP for the Karaoke sing-along ribbon.
//! No React/DOM — vitest-hermetic (tested with synthetic sine buffers).

const MIN_HZ = 80; // vocal band floor
const MAX_HZ = 1000; // vocal band ceiling
const RMS_GATE = 0.01; // below this the frame is treated as unvoiced

/**
 * Autocorrelation pitch detector (ACF + parabolic interpolation), restricted
 * to the vocal band. Returns the fundamental in Hz, or null when the frame is
 * too quiet / no clear pitch. Adapted from the well-known cwilso autoCorrelate.
 */
export function detectPitch(
  samples: Float32Array,
  sampleRate: number,
): number | null {
  const size = samples.length;
  let rms = 0;
  for (let i = 0; i < size; i++) rms += samples[i] * samples[i];
  rms = Math.sqrt(rms / size);
  if (rms < RMS_GATE) return null;

  // Trim leading/trailing low-amplitude samples to sharpen the ACF.
  const thres = 0.2;
  let r1 = 0;
  let r2 = size - 1;
  for (let i = 0; i < size / 2; i++)
    if (Math.abs(samples[i]) < thres) {
      r1 = i;
      break;
    }
  for (let i = 1; i < size / 2; i++)
    if (Math.abs(samples[size - i]) < thres) {
      r2 = size - i;
      break;
    }
  const buf = samples.slice(r1, r2);
  const n = buf.length;
  if (n < 2) return null;

  // Lag range corresponding to the vocal band.
  const minLag = Math.max(1, Math.floor(sampleRate / MAX_HZ));
  const maxLag = Math.min(n - 1, Math.ceil(sampleRate / MIN_HZ));

  const c = new Float32Array(n);
  for (let lag = 0; lag <= maxLag; lag++) {
    let sum = 0;
    for (let j = 0; j < n - lag; j++) sum += buf[j] * buf[j + lag];
    c[lag] = sum;
  }

  // Walk past the initial downslope, then take the strongest peak in-band.
  let d = 0;
  while (d < maxLag && c[d] > c[d + 1]) d++;
  let maxval = -1;
  let maxpos = -1;
  for (let i = Math.max(d, minLag); i <= maxLag; i++) {
    if (c[i] > maxval) {
      maxval = c[i];
      maxpos = i;
    }
  }
  if (maxpos <= 0) return null;

  // Parabolic interpolation around the peak for sub-sample accuracy.
  let t0 = maxpos;
  const x1 = c[t0 - 1];
  const x2 = c[t0];
  const x3 = c[t0 + 1] ?? c[t0];
  const a = (x1 + x3 - 2 * x2) / 2;
  const b = (x3 - x1) / 2;
  if (a) t0 = t0 - b / (2 * a);

  const hz = sampleRate / t0;
  if (hz < MIN_HZ || hz > MAX_HZ) return null;
  return hz;
}

const NOTE_NAMES = [
  "C",
  "C#",
  "D",
  "D#",
  "E",
  "F",
  "F#",
  "G",
  "G#",
  "A",
  "A#",
  "B",
];

/** Nearest note name + octave for a frequency (A4 = 440). */
export function hzToNoteName(hz: number): string {
  const midi = Math.round(12 * Math.log2(hz / 440) + 69);
  const name = NOTE_NAMES[((midi % 12) + 12) % 12];
  const octave = Math.floor(midi / 12) - 1;
  return `${name}${octave}`;
}

/** Signed cents from targetHz to hz (positive = sharp). */
export function centsOff(hz: number, targetHz: number): number {
  return 1200 * Math.log2(hz / targetHz);
}

/** Octave-folded cents distance between hz and the target pitch-class (0..600). */
export function octaveFoldedCents(hz: number, targetHz: number): number {
  let c = (((1200 * Math.log2(hz / targetHz)) % 1200) + 1200) % 1200;
  return Math.min(c, 1200 - c);
}

/** True when hz is within tolCents of any octave of targetHz. */
export function octaveFoldedInBand(
  hz: number,
  targetHz: number,
  tolCents: number,
): boolean {
  return octaveFoldedCents(hz, targetHz) <= tolCents;
}
