// DEV-ONLY overlay browser-smoke harness — drives the Warm Studio recording
// pill through its states via synthetic Tauri events, over the mock event
// bus installed by ./tauriMock.ts. Two modes:
//   - loop (default): cycles preparing → recording → transcribing → loop,
//     so the pill can be watched live for morph/spring correctness.
//   - pinned (`?overlayState=<name>`): emits a single state once and stops,
//     for stable per-state screenshots.
// Gated behind VITE_TAURI_MOCK + VITE_OVERLAY_DEMO by the caller
// (installOverlayDemo.ts) so this module is dead-code-eliminated from prod
// the same way tauriMock.ts is (see installMockFirst.ts).
import { emit } from "@tauri-apps/api/event";
import type { OverlayState } from "../overlay/pillLogic";

const WAVE_BAR_COUNT = 14;
const MIC_LEVEL_INTERVAL_MS = 120;
const STATE_HOLD_PREPARING_MS = 2500;
const STATE_HOLD_RECORDING_MS = 4000;
const STATE_HOLD_TRANSCRIBING_MS = 2500;

const SAMPLE_SUBTITLE =
  "This is a sample subtitle line for the Warm Studio listening capsule.";

/**
 * Maps the `?overlayState=` URL param to the raw `OverlayState` payload the
 * backend would send via `show-overlay`. "loading" is accepted as an alias
 * for "preparing" — both render the pill's "loading" variant (see
 * overlay/pillLogic.ts::pillVariant), and the brief names both so either
 * spelling works when composing the screenshot URL.
 */
const URL_PARAM_TO_STATE: Record<string, OverlayState> = {
  preparing: "preparing",
  loading: "preparing",
  listening: "recording",
  processing: "processing",
};

let micLevelTimer: number | undefined;

function randomMicLevels(): number[] {
  return Array.from({ length: WAVE_BAR_COUNT }, () => Math.random());
}

function startMicLevelTicker(): void {
  stopMicLevelTicker();
  micLevelTimer = window.setInterval(() => {
    void emit("mic-level", randomMicLevels());
  }, MIC_LEVEL_INTERVAL_MS);
}

function stopMicLevelTicker(): void {
  if (micLevelTimer !== undefined) {
    window.clearInterval(micLevelTimer);
    micLevelTimer = undefined;
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

/** Emits one `show-overlay` state, plus the mic-level ticker + a sample
 * subtitle when entering the "recording" (listening capsule) state. */
async function driveState(state: OverlayState): Promise<void> {
  await emit("show-overlay", state);
  if (state === "recording") {
    startMicLevelTicker();
    await emit("subtitle-update", SAMPLE_SUBTITLE);
  } else {
    stopMicLevelTicker();
  }
}

async function runLoop(): Promise<void> {
  for (;;) {
    await driveState("preparing");
    await delay(STATE_HOLD_PREPARING_MS);
    await driveState("recording");
    await delay(STATE_HOLD_RECORDING_MS);
    await driveState("transcribing");
    await delay(STATE_HOLD_TRANSCRIBING_MS);
  }
}

/**
 * Installs the DEV overlay demo. If the page URL carries a recognized
 * `?overlayState=` param, emits that one state and holds (starting the
 * mic-level ticker for "listening" so the waveform still animates).
 * Otherwise loops through all states indefinitely.
 *
 * `show-overlay` is (re-)emitted a few times over the first ~1.6s because the
 * overlay registers its `listen("show-overlay", …)` handler asynchronously in
 * a mount effect — a single emit at module-load can fire before the handler
 * exists and be dropped by the mock event bus. Re-emitting is idempotent.
 */
export function installOverlayDemo(): void {
  const params = new URLSearchParams(window.location.search);
  const pinned = params.get("overlayState");
  const pinnedState = pinned ? URL_PARAM_TO_STATE[pinned] : undefined;
  if (pinnedState) {
    for (const ms of [0, 300, 900, 1600]) {
      window.setTimeout(() => void driveState(pinnedState), ms);
    }
    return;
  }
  // Let the overlay's async listener register before the loop's first emit.
  window.setTimeout(() => void runLoop(), 400);
}
