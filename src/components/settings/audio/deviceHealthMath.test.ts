import { describe, it, expect } from "vitest";
import {
  statusKey,
  showReconnect,
  signalLost,
  monitoringImpossible,
  formatSampleRate,
  formatFacts,
} from "./deviceHealthMath";

describe("starting (pre-first-frame)", () => {
  it("is a status we trust, not an unknown one", () => {
    expect(statusKey("starting")).toBe("starting");
  });

  it("offers no Reconnect: the open is still in flight, reopening would abort it", () => {
    expect(showReconnect("starting")).toBe(false);
  });

  it("does not blank the meter — there is nothing stale left to hide", () => {
    expect(signalLost("starting")).toBe(false);
  });

  it("does not kill monitoring — the stream may be about to deliver", () => {
    expect(monitoringImpossible("starting")).toBe(false);
  });
});

describe("monitoringImpossible", () => {
  it("is true only when the input device is gone", () => {
    expect(monitoringImpossible("unavailable")).toBe(true);
  });

  it("is false for a stall — frames may resume, and killing a live session on a hiccup is worse than 2s of silence", () => {
    expect(monitoringImpossible("stalled")).toBe(false);
  });

  it("is false for a silent (muted) mic — passing silence through is the truth, not a failure", () => {
    expect(monitoringImpossible("silent")).toBe(false);
  });

  it("is false for ok and for a status we cannot interpret", () => {
    expect(monitoringImpossible("ok")).toBe(false);
    expect(monitoringImpossible("unknown")).toBe(false);
  });

  it("is narrower than signalLost: a stall blanks the meter but must not kill monitoring", () => {
    expect(signalLost("stalled")).toBe(true);
    expect(monitoringImpossible("stalled")).toBe(false);
  });
});

describe("statusKey", () => {
  it("passes the four known statuses through", () => {
    expect(statusKey("ok")).toBe("ok");
    expect(statusKey("silent")).toBe("silent");
    expect(statusKey("stalled")).toBe("stalled");
    expect(statusKey("unavailable")).toBe("unavailable");
  });

  it("maps anything unrecognised to 'unknown' rather than trusting it", () => {
    expect(statusKey(undefined)).toBe("unknown");
    expect(statusKey("")).toBe("unknown");
    expect(statusKey("OK")).toBe("unknown");
    expect(statusKey("garbage")).toBe("unknown");
  });
});

describe("showReconnect", () => {
  it("offers reconnect only when the device is actually broken", () => {
    expect(showReconnect("unavailable")).toBe(true);
    expect(showReconnect("stalled")).toBe(true);
  });

  it("does NOT offer reconnect for a merely quiet mic", () => {
    // A silent mic is usually a muted mic, not a broken one. Reopening the
    // stream would not fix it, and the button would teach the wrong lesson.
    expect(showReconnect("silent")).toBe(false);
    expect(showReconnect("ok")).toBe(false);
    expect(showReconnect("unknown")).toBe(false);
  });
});

describe("signalLost", () => {
  it("is true exactly when the meter and spectrum must stop claiming live audio", () => {
    expect(signalLost("unavailable")).toBe(true);
    expect(signalLost("stalled")).toBe(true);
    // Silence is real signal — a flat line is the TRUTH, not a lie.
    expect(signalLost("silent")).toBe(false);
    expect(signalLost("ok")).toBe(false);
    // Before the first health frame arrives, assume the existing behaviour.
    expect(signalLost("unknown")).toBe(false);
  });
});

describe("formatSampleRate", () => {
  it("groups thousands with a thin space", () => {
    expect(formatSampleRate(48000)).toBe("48 000 Hz");
    expect(formatSampleRate(16000)).toBe("16 000 Hz");
    expect(formatSampleRate(192000)).toBe("192 000 Hz");
  });

  it("leaves sub-thousand rates alone", () => {
    expect(formatSampleRate(800)).toBe("800 Hz");
  });
});

describe("formatFacts", () => {
  it("renders rate, channels and format on one line", () => {
    expect(
      formatFacts({
        name: "Blue Yeti",
        sample_rate: 48000,
        channels: 1,
        sample_format: "F32",
      }),
    ).toBe("48 000 Hz · 1 ch · F32");
  });

  it("pluralises channels", () => {
    expect(
      formatFacts({
        name: "Scarlett 2i2",
        sample_rate: 44100,
        channels: 2,
        sample_format: "I16",
      }),
    ).toBe("44 100 Hz · 2 ch · I16");
  });

  it("returns null when the stream has never opened", () => {
    expect(formatFacts(null)).toBeNull();
  });
});
