import { describe, expect, it } from "vitest";
import { shouldStopOnRevoke } from "./meetingClient";

describe("revoke during meeting", () => {
  it("stops when the bound client is revoked", () => {
    expect(
      shouldStopOnRevoke(
        { client_id: "anna", session_id: "2026-09-27-01" },
        "anna",
      ),
    ).toBe(true);
  });
  it("does not stop for another client or no binding", () => {
    expect(
      shouldStopOnRevoke({ client_id: "anna", session_id: "x" }, "boris"),
    ).toBe(false);
    expect(shouldStopOnRevoke(null, "anna")).toBe(false);
  });
});
