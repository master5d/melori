//! Pure presentation math for the Audio-console device health card. No DOM/React —
//! vitest-hermetic.

import { type DeviceFactsPayload } from "../../../bindings";

/** The backend's five statuses, plus the "we cannot interpret this" fallback. */
export type HealthStatusKey =
  | "ok"
  | "starting"
  | "silent"
  | "stalled"
  | "unavailable"
  | "unknown";

const KNOWN: readonly string[] = [
  "ok",
  "starting",
  "silent",
  "stalled",
  "unavailable",
];

/**
 * Narrow the wire string to a known key. Anything unrecognised becomes
 * "unknown" rather than being trusted — a status we cannot interpret must not
 * drive a destructive UI decision like blanking the meter.
 */
export function statusKey(status: string | undefined): HealthStatusKey {
  return status !== undefined && KNOWN.includes(status)
    ? (status as HealthStatusKey)
    : "unknown";
}

/**
 * Offer "Reconnect" only when reopening the stream could actually help. A
 * silent mic is almost always a MUTED mic — reopening would not fix it, and
 * the button would teach the user the wrong reflex.
 */
export function showReconnect(key: HealthStatusKey): boolean {
  return key === "unavailable" || key === "stalled";
}

/**
 * True when the meter and the spectrum must stop painting their last frame as
 * if it were live audio. Note that "silent" is NOT signal loss: a flat line
 * from a muted mic is the truth, and blanking it would hide the very thing the
 * user came to see.
 */
export function signalLost(key: HealthStatusKey): boolean {
  return key === "unavailable" || key === "stalled";
}

/**
 * True when live monitoring can no longer pass anything through, so the toggle
 * must not keep claiming it is on. Deliberately NARROWER than `signalLost`:
 *
 * - `unavailable` — the input device is gone. The output thread survives (it is
 *   a sink on a different device), MonitorBuf just underruns into silence, so
 *   without this the toggle reads ON while nothing flows. A reopen is required
 *   to come back anyway.
 * - `stalled` — frames merely stopped arriving; they may resume (a Bluetooth
 *   hiccup). Killing a live session over that is worse than a couple of seconds
 *   of silence, and the health card already tells the truth meanwhile.
 */
export function monitoringImpossible(key: HealthStatusKey): boolean {
  return key === "unavailable";
}

/** "48000" -> "48 000 Hz" (space, matching the app's number style). */
export function formatSampleRate(hz: number): string {
  const grouped = hz.toString().replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return `${grouped} Hz`;
}

/** The one-line facts row, or null when the stream has never opened. */
export function formatFacts(facts: DeviceFactsPayload | null): string | null {
  if (!facts) return null;
  return `${formatSampleRate(facts.sample_rate)} · ${facts.channels} ch · ${facts.sample_format}`;
}
