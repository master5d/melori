import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { Slider } from "../../ui/Slider";

interface SubtitleRefreshMsProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const SubtitleRefreshMs: React.FC<SubtitleRefreshMsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const refreshMs = getSetting("subtitle_refresh_ms") ?? 300;

  return (
    <Slider
      value={refreshMs}
      onChange={(value) =>
        updateSetting("subtitle_refresh_ms", Math.round(value))
      }
      min={100}
      max={1000}
      step={50}
      disabled={isUpdating("subtitle_refresh_ms")}
      label={t("settings.advanced.subtitles.refreshMs.label")}
      description={t("settings.advanced.subtitles.refreshMs.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      formatValue={(v) => `${Math.round(v)} ms`}
    />
  );
};
