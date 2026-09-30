import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  EMPTY_BANDS,
  recToTransport,
  visibleMicBands,
  type RecState,
} from "./audioTransport";
import type { TransportState } from "./TransportBar";

export interface AudioTransportState {
  recState: RecState;
  transport: TransportState;
  elapsedMs: number;
  micBands: number[];
  ttsActive: boolean;
}

/**
 * Subscribes the main window to the live pipeline signals Tasks 1-2 added
 * (`recording-state`, `mic-level`, `tts-playback`) and derives the transport
 * bar / audio status bar view state. Thin Tauri-event glue only — the
 * pure state -> view mapping lives in `audioTransport.ts` and is unit-tested
 * there; this hook has no logic of its own worth testing in isolation
 * (mirrors `RecordingOverlay.tsx`'s listener-setup pattern).
 */
export function useAudioTransport(): AudioTransportState {
  const [recState, setRecState] = useState<RecState>("idle");
  const [micBands, setMicBands] = useState<number[]>(EMPTY_BANDS);
  const [ttsActive, setTtsActive] = useState(false);
  const [elapsedMs, setElapsedMs] = useState(0);
  const recordingStartRef = useRef<number | null>(null);

  useEffect(() => {
    const setup = async () => {
      const unlistenRecState = await listen<RecState>(
        "recording-state",
        (event) => setRecState(event.payload),
      );

      // Keep the raw per-band spectrum (frozen-array hygiene: a fresh array
      // only on payloads that actually carry bands, otherwise fall back to
      // the shared empty-array constant so idle renders don't churn).
      const unlistenMicLevel = await listen<number[]>("mic-level", (event) => {
        const payload = event.payload;
        setMicBands(payload && payload.length > 0 ? payload : EMPTY_BANDS);
      });

      const unlistenTts = await listen<{ active: boolean }>(
        "tts-playback",
        (event) => setTtsActive(Boolean(event.payload?.active)),
      );

      return () => {
        unlistenRecState();
        unlistenMicLevel();
        unlistenTts();
      };
    };

    // setup() is async and resolves to the listener teardown fn; wire it
    // into React's cleanup the same way RecordingOverlay.tsx does, so
    // remounts (e.g. StrictMode dev) don't leak or double-register.
    const cleanupPromise = setup();
    return () => {
      void cleanupPromise.then((cleanup) => cleanup?.());
    };
  }, []);

  // Mono session timer: starts counting from the moment recording begins,
  // resets once it leaves the "recording" state. Mirrors
  // RecordingOverlay.tsx's timer effect (keyed there on the pill's
  // "listening" variant).
  useEffect(() => {
    if (recState !== "recording") {
      recordingStartRef.current = null;
      setElapsedMs(0);
      return;
    }
    recordingStartRef.current = Date.now();
    setElapsedMs(0);
    const interval = window.setInterval(() => {
      if (recordingStartRef.current !== null) {
        setElapsedMs(Date.now() - recordingStartRef.current);
      }
    }, 100);
    return () => window.clearInterval(interval);
  }, [recState]);

  return {
    recState,
    transport: recToTransport(recState),
    elapsedMs,
    // Surfaced live only while recording (see visibleMicBands): forces the
    // meters dark off-recording regardless of what frames the backend sent,
    // so a stale/frozen spectrum can never persist.
    micBands: visibleMicBands(recState, micBands),
    ttsActive,
  };
}
