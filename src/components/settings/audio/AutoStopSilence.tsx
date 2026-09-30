import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";

export function AutoStopSilence() {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const configured = getSetting("auto_stop_silence_min") ?? 0;
  const [value, setValue] = useState(String(configured));
  useEffect(() => setValue(String(configured)), [configured]);
  return (
    <label className="flex items-center justify-between gap-4 border-b border-rule py-3 text-sm text-primary">
      <span>
        <span className="block">
          {t("settings.audio.autoStopSilence.title")}
        </span>
        <span className="block text-xs text-secondary">
          {t("settings.audio.autoStopSilence.description")}
        </span>
      </span>
      <input
        className="w-20 border-b border-rule-strong bg-transparent px-1 py-1 text-end"
        type="number"
        min="0"
        step="1"
        value={value}
        aria-label={t("settings.audio.autoStopSilence.title")}
        onChange={(event) => setValue(event.target.value)}
        onBlur={() =>
          void commands.changeAutoStopSilenceSetting(
            Math.max(0, Number(value) || 0),
          )
        }
      />
    </label>
  );
}
