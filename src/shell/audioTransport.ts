// Pure, framework-free reducers backing `useAudioTransport` (Task 3). Kept
// dependency-free so they're trivially unit-testable — mirrors the pattern
// established by `src/overlay/pillLogic.ts` for the recording pill.
import type { TransportState } from "./TransportBar";

/**
 * The live pipeline states the backend now broadcasts to the main window via
 * the bare-string `recording-state` event (Tasks 1-2).
 */
export type RecState =
  | "idle"
  | "preparing"
  | "recording"
  | "transcribing"
  | "processing";

/**
 * Maps the backend pipeline state to the transport bar's 3-state view model
 * (`TransportState`, from `TransportBar.tsx`). Mirrors the spirit of
 * `pillLogic.ts`'s `pillVariant`: `preparing` reads as busy/"processing"
 * here (the transport bar has no dedicated loading variant), and
 * `transcribing`/`processing` both collapse into `processing`.
 */
export function recToTransport(rec: RecState): TransportState {
  switch (rec) {
    case "recording":
      return "recording";
    case "transcribing":
    case "processing":
    case "preparing":
      return "processing";
    case "idle":
    default:
      return "idle";
  }
}

/**
 * Formats a millisecond duration as a tabular `M:SS` string for the
 * transport bar's session timecode, e.g. `formatTimecode(65000) === "1:05"`.
 * Negative/NaN input clamps to `"0:00"` (mirrors `pillLogic.ts::formatTimer`,
 * minus the decisecond digit the compact deck timecode doesn't need).
 */
export function formatTimecode(ms: number): string {
  const safeMs = Number.isFinite(ms) && ms > 0 ? ms : 0;
  const totalSeconds = Math.floor(safeMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

/** Shared empty-bands constant: the meters' "dark" value. */
export const EMPTY_BANDS: number[] = [];

/**
 * Gates the live mic spectrum on active recording. The backend's level
 * callback fires whenever the audio stream is open — not just while
 * recording (idle-open window after a dictation, or the whole AlwaysOn
 * lifetime) — and never sends a terminal empty frame when the stream
 * closes. Deriving the visible bands from `recState` (rather than trusting
 * incoming frames alone) keeps the IN meter honest and guarantees it can
 * never stay frozen-lit once recording stops.
 */
export function visibleMicBands(
  recState: RecState,
  liveBands: number[],
): number[] {
  return recState === "recording" ? liveBands : EMPTY_BANDS;
}

const DEFAULT_METER_SEGMENTS = 10;

/**
 * Counts how many of `segments` meter LEDs should be lit from a live mic
 * spectrum (`bands`, 0..1 per-band values from the `mic-level` event).
 * Empty/absent bands (idle, no signal yet) light 0 segments. Values are
 * defensively clamped to [0, 1] before averaging (mic-level spikes or
 * out-of-range inputs never overflow the lit count), and the result is
 * always clamped to [0, segments] and never `NaN`.
 */
export function meterLit(
  bands: number[],
  segments: number = DEFAULT_METER_SEGMENTS,
): number {
  if (!Number.isFinite(segments) || segments <= 0) return 0;
  if (!bands || bands.length === 0) return 0;

  const clamped = bands.map((v) =>
    Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0,
  );
  const avg = clamped.reduce((sum, v) => sum + v, 0) / clamped.length;
  const lit = Math.round(avg * segments);
  return Math.min(segments, Math.max(0, lit));
}
