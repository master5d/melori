// DEV-ONLY shell transport browser-smoke harness — drives `AudioStatusBar`/
// `TransportBar` through the live 3-event contract Tasks 1-3 wired up
// (`recording-state`/`mic-level`/`tts-playback`) via the mock event bus
// installed by ./tauriMock.ts, so the Rec dot/timecode/IN-OUT meters can be
// screenshotted without a native Tauri window. Two modes, mirroring
// src/dev/overlayStateCycler.ts exactly:
//   - loop (default): cycles preparing -> recording -> transcribing -> idle
//     (+ a tts-playback pulse each lap), so the bars can be watched live.
//   - pinned (`?transportState=<name>`): emits that one `recording-state`
//     once and holds it (starting the mic-level meander for "recording"),
//     for stable, race-free per-state screenshots. `&tts=1` additionally
//     holds `tts-playback` active, for the OUT-meter screenshot.
// Gated behind VITE_TAURI_MOCK + VITE_TRANSPORT_DEMO by the caller
// (installTransportDemo.ts) so this module is dead-code-eliminated from prod
// the same way tauriMock.ts is (see installMockFirst.ts).
import { emit } from "@tauri-apps/api/event";
import type { RecState } from "../shell/audioTransport";

const MIC_LEVEL_INTERVAL_MS = 120;
const MIC_BAND_COUNT = 10;

const STATE_HOLD_PREPARING_MS = 2500;
const STATE_HOLD_RECORDING_MS = 4500;
const STATE_HOLD_TRANSCRIBING_MS = 2200;
const STATE_HOLD_IDLE_MS = 1500;
const TTS_PULSE_MS = 2500;

const PINNED_STATES: readonly RecState[] = [
  "idle",
  "preparing",
  "recording",
  "transcribing",
];

let micLevelTimer: number | undefined;
let meander: number[] = Array.from({ length: MIC_BAND_COUNT }, () => 0.3);

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v));
}

/** One smoothed random-walk step per band, so the IN meter drifts instead of
 * flickering — a closer stand-in for a real mic spectrum than pure noise. */
function stepMeander(): number[] {
  meander = meander.map((v) => clamp01(v + (Math.random() - 0.5) * 0.35));
  return meander;
}

function startMicLevelTicker(): void {
  stopMicLevelTicker();
  micLevelTimer = window.setInterval(() => {
    void emit("mic-level", stepMeander());
  }, MIC_LEVEL_INTERVAL_MS);
}

function stopMicLevelTicker(): void {
  if (micLevelTimer !== undefined) {
    window.clearInterval(micLevelTimer);
    micLevelTimer = undefined;
  }
  void emit("mic-level", [] as number[]);
}

/** Emits one `recording-state` frame (bare string payload per Tasks 1-2),
 * starting/stopping the mic-level ticker to match (only "recording" has a
 * live spectrum in the real pipeline). */
async function driveRecState(state: RecState): Promise<void> {
  await emit("recording-state", state);
  if (state === "recording") {
    startMicLevelTicker();
  } else {
    stopMicLevelTicker();
  }
}

/** Emits a `tts-playback` active pulse so the transport bar's OUT meter can
 * be observed/screenshotted, then clears it (unless `sustain`, for the
 * pinned `&tts=1` screenshot mode, which holds it active indefinitely). */
async function pulseTtsPlayback(sustain = false): Promise<void> {
  await emit("tts-playback", { active: true });
  if (sustain) return;
  await delay(TTS_PULSE_MS);
  await emit("tts-playback", { active: false });
}

/**
 * Loops the full pipeline cycle preparing -> recording -> transcribing ->
 * idle, running the mic-level meander ticker for the duration of
 * "recording" and pulsing `tts-playback` once per lap (read-back of the
 * just-finished transcript, modeled after the "idle" state).
 */
async function runLoop(): Promise<void> {
  for (;;) {
    await driveRecState("preparing");
    await delay(STATE_HOLD_PREPARING_MS);

    await driveRecState("recording");
    await delay(STATE_HOLD_RECORDING_MS);

    await driveRecState("transcribing");
    await delay(STATE_HOLD_TRANSCRIBING_MS);

    await driveRecState("idle");
    await pulseTtsPlayback();
    await delay(STATE_HOLD_IDLE_MS);
  }
}

/**
 * Installs the DEV transport demo. If the page URL carries a recognized
 * `?transportState=<name>` param (one of `PINNED_STATES`), emits that one
 * `recording-state` a few times and holds (plus `tts-playback` active if
 * `&tts=1` is also set) — race-free single-state screenshots. Otherwise
 * loops through the full cycle indefinitely.
 *
 * Emits are repeated over the first ~1.6s because `useAudioTransport`
 * registers its `listen("recording-state", …)` handler asynchronously in a
 * mount effect — a single emit at module-load can fire before the handler
 * exists and be dropped by the mock event bus (see overlayStateCycler.ts's
 * identical note). Re-emitting is idempotent.
 */
export function installTransportDemo(): void {
  const params = new URLSearchParams(window.location.search);
  const pinnedParam = params.get("transportState");
  const pinnedState =
    pinnedParam && (PINNED_STATES as readonly string[]).includes(pinnedParam)
      ? (pinnedParam as RecState)
      : undefined;

  if (pinnedState) {
    for (const ms of [0, 300, 900, 1600]) {
      window.setTimeout(() => void driveRecState(pinnedState), ms);
    }
    if (params.get("tts") === "1") {
      window.setTimeout(() => void pulseTtsPlayback(true), 300);
    }
    return;
  }

  // Let the main window's async listener register before the loop's first emit.
  window.setTimeout(() => void runLoop(), 400);
}
