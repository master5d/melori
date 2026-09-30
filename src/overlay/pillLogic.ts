// Pure, framework-free helpers for the Warm Studio recording pill.
// Kept dependency-free so they're trivially unit-testable and reusable by
// any future host (overlay webview today, potentially a settings preview
// later).

export type OverlayState =
  | "preparing"
  | "recording"
  | "transcribing"
  | "processing";

export type PillVariant = "idle" | "listening" | "processing" | "loading";

/** Maps the backend pipeline state to the pill's visual variant. */
export function pillVariant(state: OverlayState): PillVariant {
  switch (state) {
    case "recording":
      return "listening";
    case "transcribing":
    case "processing":
      return "processing";
    case "preparing":
      return "loading";
    default:
      return "idle";
  }
}

/**
 * Whether the pill's cancel affordance (button click or Esc key) should
 * act in this variant. Mirrors the existing `canCancel` semantics in
 * RecordingOverlay.tsx (`isRecording || isPreparing`, i.e. only the
 * "listening" and "loading" variants) — "processing" is intentionally
 * NOT cancellable today; this helper must not broaden that.
 */
export function isCancellable(variant: PillVariant): boolean {
  return variant === "listening" || variant === "loading";
}

/**
 * Formats a millisecond duration as a tabular `M:SS.d` string for the
 * mono session timer, e.g. `formatTimer(7200) === "0:07.2"`.
 * Negative/NaN input is clamped to 0.
 */
export function formatTimer(ms: number): string {
  const safeMs = Number.isFinite(ms) && ms > 0 ? ms : 0;
  const totalDeciseconds = Math.floor(safeMs / 100);
  const minutes = Math.floor(totalDeciseconds / 600);
  const seconds = Math.floor((totalDeciseconds % 600) / 10);
  const deciseconds = totalDeciseconds % 10;
  return `${minutes}:${String(seconds).padStart(2, "0")}.${deciseconds}`;
}

const BAR_MIN = 6;
const BAR_MAX = 34;

/**
 * Resamples a per-band mic spectrum into `count` bar heights (px) within
 * [BAR_MIN, BAR_MAX]. Mapping:
 *   1. `bands` is bucketed into `count` contiguous, roughly-equal slices
 *      (nearest-neighbour resample — no interpolation needed for a
 *      visual waveform).
 *   2. Each bucket is averaged, inputs clamped to [0, 1] first (mic-level
 *      values are already 0..1, but defensively clamp against spikes).
 *   3. The averaged 0..1 value is scaled linearly into [BAR_MIN, BAR_MAX].
 * Empty `bands` (no mic signal yet, e.g. idle/loading) yields `count`
 * values all === BAR_MIN so the waveform renders a flat resting line
 * instead of NaN/undefined heights.
 */
export function barsFromBands(bands: number[], count: number): number[] {
  if (count <= 0) return [];
  if (!bands || bands.length === 0) {
    return new Array(count).fill(BAR_MIN);
  }

  const clamped = bands.map((v) =>
    Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0,
  );

  const bars: number[] = new Array(count);
  for (let i = 0; i < count; i++) {
    const start = Math.floor((i * clamped.length) / count);
    const end = Math.max(
      start + 1,
      Math.floor(((i + 1) * clamped.length) / count),
    );
    let sum = 0;
    for (let j = start; j < end; j++) sum += clamped[j];
    const avg = sum / (end - start);
    bars[i] = BAR_MIN + avg * (BAR_MAX - BAR_MIN);
  }
  return bars;
}
