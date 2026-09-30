export type StepId = "engine" | "model" | "audio" | "consent";
export const STEPS: readonly StepId[] = ["engine", "model", "audio", "consent"];
export interface WizardState {
  engineStatus: { state: string };
  modelReady: boolean;
  audioReady: boolean;
}
export const emptyState: WizardState = {
  engineStatus: { state: "down" },
  modelReady: false,
  audioReady: false,
};
export function isOptional(_step: StepId): boolean {
  return false;
}
export function canProceed(step: StepId, s: WizardState): boolean {
  if (step === "engine") return s.engineStatus.state === "ready";
  if (step === "model") return s.modelReady;
  if (step === "audio") return s.audioReady;
  return true;
}
export function nextStep(step: StepId): StepId | null {
  const i = STEPS.indexOf(step);
  return i >= 0 && i < STEPS.length - 1 ? STEPS[i + 1] : null;
}
export function prevStep(step: StepId): StepId | null {
  const i = STEPS.indexOf(step);
  return i > 0 ? STEPS[i - 1] : null;
}
export function progress(step: StepId): number {
  const i = STEPS.indexOf(step);
  return i < 0 ? 0 : (i + 1) / STEPS.length;
}
export const ONBOARDING_KEY = "echo.onboarding.completed.v1";
export function isOnboardingDone(storage: Pick<Storage, "getItem">): boolean {
  return storage.getItem(ONBOARDING_KEY) === "1";
}
export function markOnboardingDone(storage: Pick<Storage, "setItem">): void {
  storage.setItem(ONBOARDING_KEY, "1");
}
