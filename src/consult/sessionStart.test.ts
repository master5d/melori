import { describe, expect, it } from "vitest";
import type { EngineClient } from "./api";
import { canStartSession } from "./sessionStart";

const client = (active: boolean, permissions: string[]): EngineClient =>
  ({
    id: "anna",
    alias: "Анна",
    tags: [],
    created: "",
    consent: {
      active,
      permissions,
      date: "",
      retain_days: null,
      template_version: "v1",
      signed_sha256: null,
      revoked: null,
    },
  }) as EngineClient;

describe("canStartSession", () => {
  it("allows a session with active consent to the transcript", () => {
    expect(canStartSession(client(true, ["transcript", "council"]))).toBe(true);
  });
  it("refuses after revocation", () => {
    expect(canStartSession(client(false, ["transcript"]))).toBe(false);
  });
  it("refuses without the transcript permission", () => {
    expect(canStartSession(client(true, ["council"]))).toBe(false);
  });
});
