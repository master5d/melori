import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { Slider } from "../../ui/Slider";

interface SubtitleMaxCharsProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const SubtitleMaxChars: React.FC<SubtitleMaxCharsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const maxChars = getSetting("subtitle_max_chars") ?? 140;

  return (
    <Slider
      value={maxChars}
      onChange={(value) =>
        updateSetting("subtitle_max_chars", Math.round(value))
      }
      min={40}
      max={300}
      step={10}
      disabled={isUpdating("subtitle_max_chars")}
      label={t("settings.advanced.subtitles.maxChars.label")}
      description={t("settings.advanced.subtitles.maxChars.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      formatValue={(v) => `${Math.round(v)}`}
    />
  );
};
