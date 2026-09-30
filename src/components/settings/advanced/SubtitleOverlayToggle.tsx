import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface SubtitleOverlayToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const SubtitleOverlayToggle: React.FC<SubtitleOverlayToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("subtitle_overlay") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("subtitle_overlay", value)}
        isUpdating={isUpdating("subtitle_overlay")}
        label={t("settings.advanced.subtitles.enable.label")}
        description={t("settings.advanced.subtitles.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
