import { describe, expect, it } from "vitest";
import { clientsPageState } from "./clientsPageState";

describe("clients page state", () => {
  it("engine down keeps a loaded list visible and disables creation", () => {
    const state = clientsPageState({
      engine: "down",
      diskEncryption: "on",
      clients: [{ id: "anna" } as never],
      purged: [],
    });
    expect(state.banners).toContain("engine-down");
    expect(state.canCreate).toBe(false);
    expect(state.view).toBe("list");
  });

  it("engine starting shows loading until the list arrives", () => {
    const state = clientsPageState({
      engine: "starting",
      diskEncryption: null,
      clients: null,
      purged: [],
    });
    expect(state.canCreate).toBe(false);
    expect(state.view).toBe("loading");
  });

  it("disk off and unknown have distinct banners", () => {
    expect(
      clientsPageState({
        engine: "ready",
        diskEncryption: "off",
        clients: [],
        purged: [],
      }).banners,
    ).toContain("disk-off");
    expect(
      clientsPageState({
        engine: "ready",
        diskEncryption: "unknown",
        clients: [],
        purged: [],
      }).banners,
    ).toContain("disk-unknown");
  });

  it("empty journal when ready and no clients", () => {
    expect(
      clientsPageState({
        engine: "ready",
        diskEncryption: "on",
        clients: [],
        purged: [],
      }).view,
    ).toBe("empty");
  });

  it("surfaces a non-empty purge report", () => {
    expect(
      clientsPageState({
        engine: "ready",
        diskEncryption: "on",
        clients: [],
        purged: ["anna/2026-01-01-01"],
      }).banners,
    ).toContain("purged");
  });
  it("an unencrypted or undetected disk warns but does not block creating a client", () => {
    for (const diskEncryption of ["off", "unknown", null] as const) {
      expect(
        clientsPageState({
          engine: "ready",
          diskEncryption,
          clients: [],
          purged: [],
        }).canCreate,
      ).toBe(true);
    }
  });
  it("engine down before the list ever loaded does not claim an empty journal", () => {
    const s = clientsPageState({
      engine: "down",
      diskEncryption: null,
      clients: null,
      purged: [],
    });
    expect(s.view).toBe("unavailable");
    expect(s.banners).toContain("engine-down");
  });
});
