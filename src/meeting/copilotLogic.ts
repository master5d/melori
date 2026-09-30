//! Pure decision + transcript helpers for the meeting copilot panel.
//! No React, no Tauri runtime imports — unit-tested in isolation.

export type MountAction = "reattach" | "consent" | "start";

export interface MountDecision {
  action: MountAction;
  loopbackActive: boolean;
}

/** What the panel should do on mount, given C1 capture status + whether the
 *  user has already acknowledged the one-time recording-consent notice. */
export function decideMount(
  status: { active: boolean; loopback_active: boolean },
  consentAcked: boolean,
): MountDecision {
  if (status.active) {
    return { action: "reattach", loopbackActive: status.loopback_active };
  }
  if (!consentAcked) {
    return { action: "consent", loopbackActive: false };
  }
  return { action: "start", loopbackActive: false };
}

export interface Row {
  id: number;
  source: "me" | "others";
  text: string;
}

/** Append a finalized segment to the rolling transcript, capped to the most
 *  recent `cap` rows so the panel never grows unbounded. */
export function appendRow(
  rows: Row[],
  seg: { id: number; source: "me" | "others"; text: string },
  cap: number,
): Row[] {
  const next = [...rows, { id: seg.id, source: seg.source, text: seg.text }];
  return next.length > cap ? next.slice(next.length - cap) : next;
}

/** Which sides are being captured — `both` (mic + system loopback) or `micOnly`. */
export function sourceMode(loopbackActive: boolean): "both" | "micOnly" {
  return loopbackActive ? "both" : "micOnly";
}

/** mm:ss elapsed from a millisecond duration (clamped at 0). */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
