import React from "react";
import { useTranslation } from "react-i18next";
import { Circle } from "lucide-react";
import { useSettings } from "../hooks/useSettings";
import { commands } from "@/bindings";
import { formatTimecode, meterLit } from "./audioTransport";
import { useAudioTransport } from "./useAudioTransport";

/**
 * Recording-related states the transport bar's ● Rec indicator can reflect.
 *
 * Task 3 wired this up for real: `recording-state`/`mic-level`/
 * `tts-playback` now reach the main window (Tasks 1-2), and
 * `commands.toggleDictation()` is a real Tauri command the Rec button can
 * invoke directly — the "no signal/command reaches the main window" finding
 * from Task 4 no longer holds. `useAudioTransport` derives this state from
 * those events; `recToTransport` (`audioTransport.ts`) is the pure mapping,
 * unit-tested there. `transportView` below stays the pure, DOM-free
 * state -> aria/style mapping tested in `transportBar.test.tsx`.
 */
export type TransportState = "idle" | "recording" | "processing";

export interface TransportViewModel {
  state: TransportState;
  /** i18n key for both the button's aria-label and its title/tooltip. */
  ariaKey: string;
  /** Whether the Rec button should pulse (recording in progress). */
  pulsing: boolean;
  /** Styling bucket the component maps to Tailwind classes. */
  variant: "idle" | "hot" | "busy";
}

/**
 * Pure state -> view mapping — the testable core of Task 4 (brief Step 1).
 * No i18n runtime, no DOM: just data, so it runs in this repo's Node-only
 * vitest environment.
 */
export function transportView(state: TransportState): TransportViewModel {
  switch (state) {
    case "recording":
      return {
        state,
        ariaKey: "shell.deck.transport.recRecordingAria",
        pulsing: true,
        variant: "hot",
      };
    case "processing":
      return {
        state,
        ariaKey: "shell.deck.transport.recProcessingAria",
        pulsing: false,
        variant: "busy",
      };
    case "idle":
    default:
      return {
        state,
        ariaKey: "shell.deck.transport.recIdleAria",
        pulsing: false,
        variant: "idle",
      };
  }
}

const METER_SEGMENT_COUNT = 10;

/** Live meter segments, lit up to `lit` (out of `METER_SEGMENT_COUNT`) from
 * `meterLit` (IN: `micBands`, OUT: `ttsActive`). Purely presentational
 * (aria-hidden): the IN/OUT `<em>` labels above already carry the meaning. */
const MeterSegments: React.FC<{ lit: number }> = ({ lit }) => (
  <span className="flex items-center gap-[2px]" aria-hidden="true">
    {Array.from({ length: METER_SEGMENT_COUNT }, (_, i) => (
      <i
        key={i}
        className={`inline-block w-1 h-2 rounded-[1px] ${
          i < lit ? "bg-accent-hot" : "bg-secondary/25"
        }`}
      />
    ))}
  </span>
);

const REC_BUTTON_CLASSES: Record<TransportViewModel["variant"], string> = {
  idle: "bg-surface-raised text-accent border border-edge hover:border-accent/60",
  hot: "bg-accent-hot text-ground border border-transparent",
  busy: "bg-surface-raised text-secondary border border-edge",
};

/**
 * The Deck skin's persistent transport strip — the DAW-style anchor pinned
 * along the bottom of every module. Plays the role `AudioStatusBar` plays in
 * Rail/Home (Deck does not also mount AudioStatusBar). Anatomy per spec §4 /
 * journal shell mockups: ● Rec, mono timecode, IN
 * monitor, OUT indicator, source select, local-only badge.
 *
 * Live now (Task 3): `useAudioTransport` feeds the Rec state, session
 * timecode, and IN meter from the real `recording-state`/`mic-level` events;
 * the OUT meter lights when `tts-playback` reports `active`. The ● Rec
 * button calls `commands.toggleDictation()` directly — no more navigating to
 * the Dictate module as a workaround. The mic *device* still comes from
 * settings, same source `AudioStatusBar` reads.
 */
export const TransportBar: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const { transport, elapsedMs, micBands, ttsActive } = useAudioTransport();

  const view = transportView(transport);
  const hotkey = settings?.bindings?.transcribe?.current_binding || "—";
  const device =
    settings?.selected_microphone || t("shell.audioBar.defaultDevice");
  const label = t(view.ariaKey, { hotkey });
  const inLit = meterLit(micBands, METER_SEGMENT_COUNT);
  const outLit = ttsActive ? METER_SEGMENT_COUNT : 0;

  return (
    <div
      data-testid="transport-bar"
      role="region"
      aria-label={t("shell.deck.transportLabel")}
      className="shrink-0 h-[74px] flex items-center gap-4 px-5 bg-ground border-t border-edge font-data text-[10.5px] text-secondary"
    >
      <button
        type="button"
        onClick={() => void commands.toggleDictation()}
        aria-label={label}
        title={label}
        className={`w-11 h-11 shrink-0 rounded-full flex items-center justify-center transition-transform duration-300 hover:scale-105 ${
          REC_BUTTON_CLASSES[view.variant]
        } ${view.pulsing ? "animate-pulse" : ""}`}
      >
        <Circle width={14} height={14} fill="currentColor" />
      </button>

      <span
        className="font-data text-lg text-primary tabular-nums shrink-0"
        aria-label={t("shell.deck.transport.timecodeLabel")}
      >
        {formatTimecode(elapsedMs)}
      </span>

      <div className="flex flex-col gap-1 shrink-0">
        <span className="flex items-center gap-1.5">
          <em className="not-italic text-secondary/70 w-6">
            {t("shell.deck.transport.inLabel")}
          </em>
          <MeterSegments lit={inLit} />
        </span>
        <span className="flex items-center gap-1.5">
          <em className="not-italic text-secondary/70 w-6">
            {t("shell.deck.transport.outLabel")}
          </em>
          <MeterSegments lit={outLit} />
        </span>
      </div>

      <span className="border border-edge rounded-lg px-3 py-1.5 bg-surface-raised shrink-0">
        {device}
      </span>

      <span className="shrink-0">
        {t("shell.deck.transport.monitor")}:{" "}
        <span className="text-primary">—</span>
      </span>

      <span className="ms-auto flex items-center gap-1.5 shrink-0">
        <i className="inline-block w-[7px] h-[7px] rounded-full bg-accent" />
        {t("shell.audioBar.localOnly")}
      </span>
    </div>
  );
};
