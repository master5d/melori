import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type InputSpectrum } from "../../../bindings";

export interface SpectrumState {
  /**
   * The latest analyzer frame (dBFS per band), or null before the first frame.
   * Read this from a requestAnimationFrame loop — NOT during render. Frames
   * arrive ~24/s and deliberately do not trigger React re-renders.
   */
  framesRef: React.MutableRefObject<number[] | null>;
  /** Band center frequencies, owned by Rust — the single source of truth for the axis. */
  freqs: number[];
  /** The capture rate the current bands were computed for; 0 before the first frame. */
  sampleRate: number;
  /** True once at least one frame has arrived. */
  active: boolean;
}

/**
 * Subscribes to `input-spectrum` and fetches the Rust-owned band centers.
 *
 * LISTEN-ONLY: never calls startInputMonitor/stopInputMonitor. The Epic-A meter
 * (useInputMonitor) owns the monitor stream's lifecycle — a second start/stop
 * pair would tear the stream out from under it. The spectrum is therefore live
 * only while the meter is mounted; both sit in the Audio section, and the meter
 * is what turns the backend's `monitor_active` gate on.
 */
export function useSpectrum(): SpectrumState {
  const framesRef = useRef<number[] | null>(null);
  const rateRef = useRef(0);
  const activeRef = useRef(false);
  const [freqs, setFreqs] = useState<number[]>([]);
  const [sampleRate, setSampleRate] = useState(0);
  const [active, setActive] = useState(false);

  useEffect(() => {
    let alive = true;
    let unlisten: UnlistenFn | null = null;

    void (async () => {
      try {
        const un = await listen<InputSpectrum>("input-spectrum", (e) => {
          if (!alive) return;
          framesRef.current = e.payload.bands;

          if (!activeRef.current) {
            activeRef.current = true;
            setActive(true);
          }

          // The rate changes only when the capture stream reopens on a new device.
          // Refetch the axis then — and only then.
          if (e.payload.sample_rate !== rateRef.current) {
            rateRef.current = e.payload.sample_rate;
            setSampleRate(e.payload.sample_rate);
            void commands.getSpectrumFreqs().then((res) => {
              if (alive && res.status === "ok") setFreqs(res.data);
            });
          }
        });
        if (!alive) {
          un();
          return;
        }
        unlisten = un;
      } catch {
        // IPC-layer throw (e.g. webview teardown mid-await) — degrade quietly;
        // the panel simply shows its idle state.
      }
    })();

    return () => {
      alive = false;
      if (unlisten) unlisten();
      framesRef.current = null;
      rateRef.current = 0;
      activeRef.current = false;
    };
  }, []);

  return { framesRef, freqs, sampleRate, active };
}
