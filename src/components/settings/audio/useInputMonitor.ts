import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type InputMonitorLevel } from "../../../bindings";
import { linearToDbfs, isClipping, peakHold } from "./meterMath";

const HOLD_DECAY_DB = 1.2; // per emitted frame (~31/s → ~37 dB/s)

export interface InputMonitorState {
  rmsDbfs: number;
  peakDbfs: number;
  holdDbfs: number;
  clipping: boolean;
  active: boolean;
  error: string | null;
}

const IDLE: InputMonitorState = {
  rmsDbfs: -100,
  peakDbfs: -100,
  holdDbfs: -100,
  clipping: false,
  active: false,
  error: null,
};

/**
 * Starts the backend input monitor on mount, listens for `input-monitor-level`
 * frames, and exposes dBFS levels + peak-hold + clip for the meter. Stops the
 * monitor and unlistens on unmount. A generation counter guards the async
 * listen setup so a fast mount/unmount/remount discards stale subscriptions.
 *
 * `signalLost` (from the device-health card) zeroes the level state. Levels here
 * are only ever written by an INCOMING event, so when the device dies they simply
 * freeze at their last value. Blanking the bar hides that — until the meter
 * un-blanks, and paints the pre-death levels for the 10–200 ms (longer on
 * Bluetooth/USB) between a successful reopen and cpal's first buffer. Zeroing on
 * loss means there is nothing stale left to paint. It also fixes the peak-hold
 * decay, which ticks per EVENT rather than per wall-clock frame and therefore
 * froze undecayed through an outage.
 *
 * The reset lives in its OWN effect. It must never enter the lifecycle effect's
 * dependencies: that effect owns `startInputMonitor`/`stopInputMonitor`, and a
 * second start/stop pair is the exact Critical an earlier epic of this console
 * shipped.
 */
export function useInputMonitor(signalLost = false): InputMonitorState {
  const [state, setState] = useState<InputMonitorState>(IDLE);
  const holdRef = useRef(-100);
  const genRef = useRef(0);

  useEffect(() => {
    if (!signalLost) return;
    holdRef.current = -100;
    // Levels only. `active` and `error` describe the MONITOR's lifecycle, not the
    // device's, and are not ours to clear here.
    setState((prev) => ({
      ...prev,
      rmsDbfs: IDLE.rmsDbfs,
      peakDbfs: IDLE.peakDbfs,
      holdDbfs: IDLE.holdDbfs,
      clipping: false,
    }));
  }, [signalLost]);

  useEffect(() => {
    genRef.current++;
    const gen = genRef.current;
    holdRef.current = -100;
    let unlisten: UnlistenFn | null = null;

    (async () => {
      try {
        const started = await commands.startInputMonitor();
        if (gen !== genRef.current) {
          // Superseded/unmounted during the await — bail, but do NOT "undo" the
          // start. The backend monitor is a single shared resource, not one per
          // hook instance: our cleanup (which is what bumped the generation)
          // already issued its stop, so a second stop here lands AFTER the new
          // generation's start and kills the monitor it owns. Observed under
          // StrictMode's double-mount, where the real call order is
          // start → stop(cleanup) → start(remount) → stop(this bail) — leaving
          // the console dead: no meter, no spectrum, no health frames at all.
          return;
        }
        if (started.status === "error") {
          setState({ ...IDLE, error: started.error });
          return;
        }
        setState({ ...IDLE, active: true });

        const un = await listen<InputMonitorLevel>(
          "input-monitor-level",
          (e) => {
            if (gen !== genRef.current) return;
            const peakDbfs = linearToDbfs(e.payload.peak);
            const rmsDbfs = linearToDbfs(e.payload.rms);
            holdRef.current = peakHold(
              holdRef.current,
              peakDbfs,
              HOLD_DECAY_DB,
            );
            setState({
              rmsDbfs,
              peakDbfs,
              holdDbfs: holdRef.current,
              clipping: isClipping(e.payload.peak),
              active: true,
              error: null,
            });
          },
        );

        if (gen !== genRef.current) {
          un(); // unmounted during listen setup
          return;
        }
        unlisten = un;
      } catch (err) {
        // IPC-layer throw (e.g. webview teardown mid-await) — degrade to the
        // graceful error state instead of an unhandled promise rejection.
        if (gen === genRef.current) {
          setState({
            ...IDLE,
            error: err instanceof Error ? err.message : String(err),
          });
        }
      }
    })();

    return () => {
      genRef.current++; // invalidate any in-flight setup
      if (unlisten) unlisten();
      void commands.stopInputMonitor();
    };
  }, []);

  return state;
}
