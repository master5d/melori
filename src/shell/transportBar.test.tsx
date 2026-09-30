import { describe, it, expect } from "vitest";
import { transportView } from "./TransportBar";

// Pure state -> view mapping for the Deck skin's transport bar Rec
// indicator. No DOM involved (this repo's vitest is Node-env only — see
// shellHost.test.tsx) so this is the testable core of Task 4's Step 1: the
// mapping is exercised end-to-end even though DeckShell always feeds it
// "idle" today (see TransportBar.tsx's doc-comment for why no
// main-window-reachable recording signal exists yet).
describe("transportView", () => {
  it("idle: neutral styling, non-pulsing, idle aria key", () => {
    const view = transportView("idle");
    expect(view.state).toBe("idle");
    expect(view.variant).toBe("idle");
    expect(view.pulsing).toBe(false);
    expect(view.ariaKey).toBe("shell.deck.transport.recIdleAria");
  });

  it("recording: hot styling, pulsing, recording aria key", () => {
    const view = transportView("recording");
    expect(view.state).toBe("recording");
    expect(view.variant).toBe("hot");
    expect(view.pulsing).toBe(true);
    expect(view.ariaKey).toBe("shell.deck.transport.recRecordingAria");
  });

  it("processing: busy styling, non-pulsing, processing aria key", () => {
    const view = transportView("processing");
    expect(view.state).toBe("processing");
    expect(view.variant).toBe("busy");
    expect(view.pulsing).toBe(false);
    expect(view.ariaKey).toBe("shell.deck.transport.recProcessingAria");
  });
});
