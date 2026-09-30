import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type InputDeviceHealth } from "../../../bindings";
import {
  formatFacts,
  showReconnect,
  type HealthStatusKey,
} from "./deviceHealthMath";

interface Props {
  health: InputDeviceHealth | null;
  statusKey: HealthStatusKey;
}

/**
 * Status dot colour, from the SAME semantic tokens the meter's bar uses
 * (`bg-ok` / `bg-warn` / `bg-err`). Tailwind's generic default-palette scales
 * are NOT available here and would fail the DesOps design lint, which must
 * stay at 0 findings.
 */
const DOT_CLASS: Record<HealthStatusKey, string> = {
  ok: "bg-ok",
  // Neither good news nor bad news yet: the stream is open and we are waiting
  // for its first frame. Colouring it `ok` is exactly the lie this state exists
  // to stop telling.
  starting: "bg-info",
  silent: "bg-warn",
  stalled: "bg-err",
  unavailable: "bg-err",
  unknown: "bg-secondary/40",
};

/**
 * The device-facts + health card (Epic D2). The facts row is always visible,
 * including when everything is fine — the point is that the user can CHECK the
 * microphone, not merely be warned about it.
 */
export const DeviceHealthCard: React.FC<Props> = ({ health, statusKey }) => {
  const { t } = useTranslation();
  const [reconnecting, setReconnecting] = useState(false);

  const facts = formatFacts(health?.facts ?? null);

  const handleReconnect = () => {
    setReconnecting(true);
    void (async () => {
      const res = await commands.reconnectInputDevice();
      setReconnecting(false);
      if (res.status === "error") {
        toast.error(t("settings.audio.deviceHealth.reconnectFailed"));
      }
    })();
  };

  return (
    <div className="px-5 py-4 space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-black text-secondary uppercase tracking-[0.25em]">
          {t("settings.audio.deviceHealth.title")}
        </span>
        {showReconnect(statusKey) && (
          <button
            type="button"
            onClick={handleReconnect}
            disabled={reconnecting}
            className="px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider bg-surface-raised text-secondary hover:text-primary transition-colors disabled:opacity-50"
          >
            {t("settings.audio.deviceHealth.reconnect")}
          </button>
        )}
      </div>

      <div className="rounded-md border border-edge bg-surface px-3 py-2 space-y-1">
        <div className="text-xs font-medium text-primary">
          {health?.facts?.name ?? t("settings.audio.deviceHealth.noDevice")}
        </div>

        <div className="text-[10px] text-secondary/70 tabular-nums">
          {facts ?? t("settings.audio.deviceHealth.noFacts")}
        </div>

        <div className="flex items-center gap-2 text-[10px] text-secondary pt-0.5">
          <span
            className={`inline-block h-2 w-2 rounded-full ${DOT_CLASS[statusKey]}`}
            aria-hidden="true"
          />
          <span>{t(`settings.audio.deviceHealth.status.${statusKey}`)}</span>
        </div>

        {/*
          The backend's `reason` — the one genuinely actionable fact on this card.
          "Access is denied" (a revoked microphone permission), "No input device
          found", and a cpal stream-build failure are three different problems
          that, without this line, all render as the identical constant
          "Device unavailable". It is a raw technical string from cpal/the OS, so
          it is presented as untranslated DETAIL under the status row, never as
          the headline.
        */}
        {health?.reason && (
          <div className="text-[10px] text-secondary/70 leading-relaxed break-words pl-4">
            {health.reason}
          </div>
        )}
      </div>
    </div>
  );
};
