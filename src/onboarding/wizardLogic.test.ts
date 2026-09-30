import { describe, expect, it } from "vitest";
import {
  STEPS,
  emptyState,
  canProceed,
  nextStep,
  prevStep,
  progress,
  isOptional,
  ONBOARDING_KEY,
  isOnboardingDone,
  markOnboardingDone,
} from "./wizardLogic";

describe("consult onboarding", () => {
  it("has four steps in order", () => {
    expect(STEPS).toEqual(["engine", "model", "audio", "consent"]);
  });
  it("requires a ready engine", () => {
    expect(canProceed("engine", emptyState)).toBe(false);
    expect(
      canProceed("engine", { ...emptyState, engineStatus: { state: "ready" } }),
    ).toBe(true);
  });
  it("requires model and audio readiness", () => {
    expect(canProceed("model", emptyState)).toBe(false);
    expect(canProceed("model", { ...emptyState, modelReady: true })).toBe(true);
    expect(canProceed("audio", emptyState)).toBe(false);
    expect(canProceed("audio", { ...emptyState, audioReady: true })).toBe(true);
  });
  it("navigates and reports progress", () => {
    expect(prevStep("engine")).toBeNull();
    expect(nextStep("consent")).toBeNull();
    expect(nextStep("engine")).toBe("model");
    expect(prevStep("audio")).toBe("model");
    expect(progress("consent")).toBe(1);
    expect(isOptional("consent")).toBe(false);
  });
  it("an engine that is starting or down does not pass the engine step", () => {
    for (const state of ["starting", "down", ""]) {
      expect(
        canProceed("engine", { ...emptyState, engineStatus: { state } }),
      ).toBe(false);
    }
  });
  it("progress grows monotonically and ends at one", () => {
    const values = STEPS.map(progress);
    for (let i = 1; i < values.length; i++)
      expect(values[i]).toBeGreaterThan(values[i - 1]);
    expect(values[values.length - 1]).toBe(1);
  });
  it("empty storage means not done", () => {
    expect(isOnboardingDone({ getItem: () => null })).toBe(false);
  });
  it("a foreign stored value does not count as done", () => {
    expect(isOnboardingDone({ getItem: () => "true" })).toBe(false);
    expect(isOnboardingDone({ getItem: () => "0" })).toBe(false);
  });
  it("persists completion", () => {
    const bag: Record<string, string> = {};
    markOnboardingDone({
      setItem: (k, v) => {
        bag[k] = v;
      },
    });
    expect(bag[ONBOARDING_KEY]).toBe("1");
    expect(isOnboardingDone({ getItem: (k) => bag[k] ?? null })).toBe(true);
  });
});
