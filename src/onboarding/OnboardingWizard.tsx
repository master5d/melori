import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  StepId,
  STEPS,
  WizardState,
  canProceed,
  nextStep,
  prevStep,
  progress,
  markOnboardingDone,
} from "./wizardLogic";

export interface OnboardingWizardProps {
  onClose: () => void;
  onOpenSettings?: (section?: string) => void;
}
export function OnboardingWizard({
  onClose,
  onOpenSettings,
}: OnboardingWizardProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<StepId>("engine");
  const [state, setState] = useState<WizardState>({
    engineStatus: { state: "starting" },
    modelReady: false,
    audioReady: false,
  });
  const settings = useSettingsStore((s) => s.settings);
  useEffect(() => {
    let active = true;
    const poll = async () => {
      try {
        const status = await commands.engineStatus();
        if (active) setState((s) => ({ ...s, engineStatus: status }));
      } catch {
        /* retry */
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 1000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);
  const finish = () => {
    markOnboardingDone(window.localStorage);
    onClose();
  };
  const advance = () => {
    const n = nextStep(step);
    if (n) setStep(n);
    else finish();
  };
  const back = () => {
    const p = prevStep(step);
    if (p) setStep(p);
  };
  const open = (section: string) => onOpenSettings?.(section);
  const ready =
    step === "model"
      ? !!settings?.selected_model
      : step === "audio"
        ? !!settings?.selected_microphone && !!settings?.selected_output_device
        : canProceed(step, state);
  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-ground/80 p-4"
    >
      <div className="w-full max-w-3xl border border-edge bg-surface p-8 text-primary">
        <button
          type="button"
          className="float-right"
          onClick={finish}
          aria-label={t("tray.cancel")}
        >
          <X />
        </button>
        <div className="grid gap-8 md:grid-cols-[220px_1fr]">
          <ol className="border-t-2 border-edge-strong">
            {STEPS.map((id, i) => (
              <li
                key={id}
                className={`border-b border-edge py-4 text-sm ${id === step ? "font-semibold text-accent" : "text-secondary"}`}
              >
                {String(i + 1).padStart(2, "0")} ·{" "}
                {t(`onboarding.consult.${id}`)}
              </li>
            ))}
          </ol>
          <section className="space-y-5">
            <p className="font-mono text-xs text-secondary">
              {t("onboarding.stepOf", {
                current: STEPS.indexOf(step) + 1,
                total: STEPS.length,
              })}
            </p>
            <h1 className="font-display text-4xl">
              {t(`onboarding.consult.${step}Title`)}
            </h1>
            <p className="font-display text-lg text-secondary">
              {t(`onboarding.consult.${step}Hint`)}
            </p>
            {step === "engine" && state.engineStatus.state !== "ready" && (
              <p className="text-sm text-warn">
                {t("onboarding.consult.engineWait")}
              </p>
            )}
            {step === "consent" && (
              <p className="text-sm text-secondary">
                {t("onboarding.consult.consentBody")}
              </p>
            )}
            {(step === "model" || step === "audio") && (
              <button
                type="button"
                onClick={() => open(step)}
                className="border border-edge-strong px-4 py-2 text-sm"
              >
                {t("onboarding.openSettings")}
              </button>
            )}
            {step === "consent" && (
              <button
                type="button"
                onClick={() => open("clients")}
                className="bg-accent px-4 py-2 text-sm text-ground"
              >
                {t("onboarding.consult.firstClient")}
              </button>
            )}
            <div className="flex justify-between border-t border-edge pt-5">
              <button
                type="button"
                onClick={back}
                disabled={!prevStep(step)}
                className="text-sm disabled:opacity-30"
              >
                {t("onboarding.back")}
              </button>
              <button
                type="button"
                onClick={advance}
                disabled={!ready}
                className="bg-accent px-4 py-2 text-sm text-ground disabled:opacity-40"
              >
                {step === "consent"
                  ? t("onboarding.finish")
                  : t("onboarding.next")}
              </button>
            </div>
            <div className="h-1 bg-edge">
              <div
                className="h-full bg-accent"
                style={{ width: `${progress(step) * 100}%` }}
              />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
