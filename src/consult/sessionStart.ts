import type { EngineClient } from "./api";

/** A new session needs active consent that includes the transcript. */
export function canStartSession(client: EngineClient): boolean {
  return (
    client.consent.active && client.consent.permissions.includes("transcript")
  );
}
