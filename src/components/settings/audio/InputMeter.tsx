import React from "react";
import { useTranslation } from "react-i18next";
import { useInputMonitor } from "./useInputMonitor";
import { dbfsToFraction } from "./meterMath";

const SCALE_TICKS = [-60, -40, -20, -12, -6, 0];

interface Props {
  /** The device is gone (`unavailable`/`stalled`). Not set for `silent`. */
  signalLost?: boolean;
}

/**
 * Live input meter for the Audio console: an RMS bar with a peak-hold marker,
 * a clip LED, and a dBFS scale, driven by the real cpal stream via
 * useInputMonitor. Warm Studio semantic tokens only.
 *
 * `signalLost` is the external truth this component used to lack: when the
 * device dies mid-session the level events simply stop, `active` stays true,
 * and the bar keeps showing the last levels it received as if they were live.
 * Rendering the "no signal" branch instead of the bar is what stops that lie —
 * there is no stale value left on screen to misread.
 */
export const InputMeter: React.FC<Props> = ({ signalLost = false }) => {
  const { t } = useTranslation();
  // Passed DOWN into the hook, not just used for the branch below: blanking the
  // bar is not enough on its own, because the hook's levels would still hold
  // their pre-death values and repaint them the instant the meter un-blanks
  // (which happens on a successful reopen, before cpal's first buffer lands).
  const { rmsDbfs, holdDbfs, clipping, active, error } =
    useInputMonitor(signalLost);

  const rmsPct = dbfsToFraction(rmsDbfs) * 100;
  const holdPct = dbfsToFraction(holdDbfs) * 100;
  // Bar hue: warn as we approach clip (~-6 dBFS), err on clip.
  const barColor = clipping ? "bg-err" : rmsDbfs > -6 ? "bg-warn" : "bg-ok";

  return (
    <div className="px-5 py-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[11px] font-black text-secondary uppercase tracking-[0.25em]">
          {t("settings.audio.meter.title")}
        </span>
        <span
          className={`text-[10px] font-bold uppercase tracking-wider ${
            clipping && !signalLost ? "text-err" : "text-secondary/60"
          }`}
        >
          {t("settings.audio.meter.clip")}
        </span>
      </div>

      {error || signalLost ? (
        <p className="text-xs text-err/90 leading-relaxed">
          {error
            ? t("settings.audio.meter.noDevice")
            : t("settings.audio.deviceHealth.noSignal")}
        </p>
      ) : (
        <>
          <div className="relative h-3 rounded-full bg-surface overflow-hidden">
            <div
              className={`absolute inset-y-0 left-0 ${barColor} transition-[width] duration-75`}
              style={{ width: `${rmsPct}%` }}
            />
            {active && holdDbfs > -100 && (
              <div
                className="absolute inset-y-0 w-0.5 bg-accent"
                style={{ left: `${holdPct}%` }}
              />
            )}
          </div>
          <div className="relative mt-1 h-3 text-[9px] text-secondary/50 tabular-nums">
            {SCALE_TICKS.map((db) => (
              <span
                key={db}
                className="absolute -translate-x-1/2"
                style={{ left: `${dbfsToFraction(db) * 100}%` }}
              >
                {db}
              </span>
            ))}
          </div>
          {!active && (
            <p className="text-[10px] text-secondary/50 mt-1">
              {t("settings.audio.meter.idle")}
            </p>
          )}
        </>
      )}
    </div>
  );
};
