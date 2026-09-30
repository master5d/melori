import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { commands } from "../../../bindings";
import { usePassthroughGuard } from "./usePassthroughGuard";
import { monitoringImpossible, type HealthStatusKey } from "./deviceHealthMath";

interface PassthroughToggleProps {
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
  /** Live input health (Epic D2) — an input loss must not leave the toggle ON. */
  healthKey: HealthStatusKey;
}

/**
 * Controlled on/off for live monitoring (passthrough). Session-only backend
 * state: reads it on mount, writes each change, and disables on unmount so a
 * closed Audio section never leaves the mic monitoring. A clip-watch guard
 * auto-disables on sustained feedback.
 */
export const PassthroughToggle: React.FC<PassthroughToggleProps> = ({
  enabled,
  onEnabledChange,
  healthKey,
}) => {
  const { t } = useTranslation();

  useEffect(() => {
    let alive = true;
    void commands.getPassthroughEnabled().then((res) => {
      if (alive && res.status === "ok") onEnabledChange(res.data);
    });
    return () => {
      alive = false;
      // Section unmount: never leave the mic monitoring in the background.
      void commands.setPassthroughEnabled(false);
    };
  }, []);

  usePassthroughGuard(enabled, () => {
    onEnabledChange(false);
    void commands.setPassthroughEnabled(false);
    toast.warning(t("settings.audio.monitor.feedbackWarning"));
  });

  // The output thread can end abnormally after the toggle already reads ON
  // (device disconnect while monitoring, or the capture rate never becoming
  // known) — it emits `monitor-error` so the toggle and the user both learn
  // about it instead of a silent "on with no audio". Mirrors the async
  // listen + alive/cleanup guard in usePassthroughGuard.ts.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;

    void (async () => {
      const un = await listen<string>("monitor-error", () => {
        onEnabledChange(false);
        void commands.setPassthroughEnabled(false);
        toast.warning(t("settings.audio.monitor.deviceError"));
      });
      if (!alive) {
        un();
        return;
      }
      unlisten = un;
    })();

    return () => {
      alive = false;
      if (unlisten) unlisten();
    };
  }, [t, onEnabledChange]);

  // An INPUT-device loss does not kill the output thread (that sink lives on a
  // different device) — MonitorBuf simply underruns into silence, so the toggle
  // would keep reading ON while nothing flows. D2's health signal is what makes
  // this observable; act on it exactly like the clip-guard and `monitor-error`.
  // Narrow on purpose (see monitoringImpossible): a 2s stall may resolve itself.
  useEffect(() => {
    if (!enabled || !monitoringImpossible(healthKey)) return;
    onEnabledChange(false);
    void commands.setPassthroughEnabled(false);
    toast.warning(t("settings.audio.monitor.inputLost"));
  }, [enabled, healthKey, onEnabledChange, t]);

  const handleChange = (checked: boolean) => {
    onEnabledChange(checked);
    void (async () => {
      const res = await commands.setPassthroughEnabled(checked);
      if (res.status === "error") {
        // Revert: the backend rejected the change (e.g. "No input device
        // found") — don't leave the switch showing a state that never
        // actually took effect.
        onEnabledChange(!checked);
        toast.warning(t("settings.audio.monitor.deviceError"));
      }
    })();
  };

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={handleChange}
      label={t("settings.audio.monitor.title")}
      description={t("settings.audio.monitor.description")}
      descriptionMode="inline"
      grouped
    />
  );
};
