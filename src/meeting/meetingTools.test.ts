import { describe, expect, it } from "vitest";
import {
  countdownState,
  visibleAsks,
  askChipLine,
  diagnosisWarning,
} from "./meetingTools";

describe("meeting tools", () => {
  it("shows the two newest asks and counts the rest", () => {
    expect(visibleAsks([1, 2, 3, 4], false)).toEqual({
      shown: [1, 2],
      hidden: 2,
    });
    expect(visibleAsks([1, 2, 3, 4], true)).toEqual({
      shown: [1, 2, 3, 4],
      hidden: 0,
    });
  });
  it("counts down, is cancelled by speech, expires at zero", () => {
    expect(countdownState(0, 20_000, null)).toEqual({
      remaining: 40_000,
      cancelled: false,
      expired: false,
    });
    expect(countdownState(0, 20_000, 10_000).cancelled).toBe(true);
    expect(countdownState(0, 60_000, null).expired).toBe(true);
    expect(countdownState(10_000, 20_000, 5_000).cancelled).toBe(false);
  });
  it("formats ask provenance including truncation and locality", () => {
    const t = (key: string, options?: Record<string, unknown>) =>
      `${key}:${JSON.stringify(options ?? {})}`;
    expect(
      askChipLine(
        {
          model: "local-floor",
          endpoint_local: true,
          input: "transcript",
          chars_sent: 1234,
          until_ms: 125000,
          truncated: true,
          language: "ru",
          attempts: 1,
        },
        t,
      ),
    ).toContain("02:05");
    expect(
      askChipLine(
        {
          model: "remote",
          endpoint_local: false,
          input: "transcript",
          chars_sent: 2,
          until_ms: 0,
          truncated: false,
          language: "en",
          attempts: 1,
        },
        t,
      ),
    ).toContain("consult.session.provenance.remote");
  });
  it("formats a diagnosis warning", () => {
    const t = (key: string, options?: Record<string, unknown>) =>
      `${key} ${String(options?.phrase ?? "")}`;
    expect(diagnosisWarning("синдром", t)).toContain("синдром");
  });
});
