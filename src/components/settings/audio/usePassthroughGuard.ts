import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { type InputMonitorLevel } from "../../../bindings";
import { ClipGuard } from "./monitorGuard";

/**
 * While passthrough is enabled, watches the shared `input-monitor-level` peak
 * (the meter owns the monitor lifecycle — this hook only listens) and calls
 * `onTrip` once a sustained clip is detected. Belt-and-suspenders with the
 * backend mute-ramp: the ramp softens the start, the guard is the emergency
 * stop.
 */
export function usePassthroughGuard(
  enabled: boolean,
  onTrip: () => void,
): void {
  const onTripRef = useRef(onTrip);
  onTripRef.current = onTrip;

  useEffect(() => {
    if (!enabled) return;
    const guard = new ClipGuard();
    let unlisten: UnlistenFn | null = null;
    let alive = true;

    void (async () => {
      const un = await listen<InputMonitorLevel>("input-monitor-level", (e) => {
        if (guard.observe(e.payload.peak)) {
          guard.reset();
          onTripRef.current();
        }
      });
      if (!alive) {
        un();
        return;
      }
      unlisten = un;
    })();

    return () => {
      alive = false;
      if (unlisten) unlisten();
    };
  }, [enabled]);
}
