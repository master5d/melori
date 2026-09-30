import React from "react";
import { useTranslation } from "react-i18next";
import { Mic } from "lucide-react";
import { useSettings } from "../hooks/useSettings";
import { meterLit } from "./audioTransport";
import { useAudioTransport } from "./useAudioTransport";

const MIC_PIP_SEGMENTS = 4;

/**
 * Global audio status strip — mic device, active STT model, and the
 * push-to-talk/toggle hotkey, pinned along the bottom of Rail (and Home in
 * Task 3). Per spec §4 this is the one piece of chrome visible from every
 * module.
 *
 * Live mic level IS wired here (Task 3): `mic-level`/`recording-state` now
 * reach the main window (Tasks 1-2), so `useAudioTransport` drives a small
 * lit pip alongside the device name and the recording dot goes hot while
 * `transport === "recording"`.
 */
export const AudioStatusBar: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const { recState, transport, micBands } = useAudioTransport();

  const device =
    settings?.selected_microphone || t("shell.audioBar.defaultDevice");
  const model =
    recState === "preparing"
      ? t("shell.audioBar.loadingModel")
      : settings?.selected_model || t("shell.audioBar.noModel");
  const hotkey = settings?.bindings?.transcribe?.current_binding || "—";
  const lit = meterLit(micBands, MIC_PIP_SEGMENTS);

  return (
    <div
      data-testid="audio-status-bar"
      className="shrink-0 h-11 flex items-center gap-4 px-5 bg-ground border-t border-edge font-data text-[10.5px] text-secondary"
    >
      <span className="flex items-center gap-1.5 text-primary">
        <Mic
          width={13}
          height={13}
          className={
            transport === "recording" ? "text-accent-hot" : "text-accent"
          }
        />
        {device}
        <span className="flex items-center gap-[2px]" aria-hidden="true">
          {Array.from({ length: MIC_PIP_SEGMENTS }, (_, i) => (
            <i
              key={i}
              className={`inline-block w-[3px] h-[7px] rounded-[1px] ${
                i < lit ? "bg-accent-hot" : "bg-secondary/25"
              }`}
            />
          ))}
        </span>
      </span>
      <span>
        {t("shell.audioBar.model")}: {model}
      </span>
      <span>
        {t("shell.audioBar.hotkey")}:{" "}
        <span className="border border-edge rounded px-1">{hotkey}</span>
      </span>
      <span className="ms-auto flex items-center gap-1.5">
        <i
          className={`inline-block w-[7px] h-[7px] rounded-full ${
            transport === "recording" ? "bg-accent-hot" : "bg-accent"
          }`}
        />
        {t("shell.audioBar.localOnly")}
      </span>
    </div>
  );
};
