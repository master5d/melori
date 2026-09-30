import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { type InputDeviceHealth } from "../../../bindings";
import { statusKey, type HealthStatusKey } from "./deviceHealthMath";

/**
 * Subscribes to `input-device-health` (Epic D2).
 *
 * LISTEN-ONLY, on purpose: it never calls `startInputMonitor`/`stopInputMonitor`.
 * The Epic-A meter owns the capture stream's lifecycle; a second start/stop pair
 * would tear the stream out from under it — the exact Critical Epic C shipped.
 *
 * Unlike `useSpectrum`, these frames arrive at 4/s and each one changes the card,
 * so plain `useState` is right here — there is no re-render storm to dodge.
 */
export function useDeviceHealth(): {
  health: InputDeviceHealth | null;
  key: HealthStatusKey;
} {
  const [health, setHealth] = useState<InputDeviceHealth | null>(null);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;

    void (async () => {
      const un = await listen<InputDeviceHealth>("input-device-health", (e) => {
        setHealth(e.payload);
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
  }, []);

  return { health, key: statusKey(health?.status) };
}
