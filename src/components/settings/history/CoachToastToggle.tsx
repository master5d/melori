import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface CoachToastToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CoachToastToggle: React.FC<CoachToastToggleProps> = React.memo(
  ({ descriptionMode = "inline", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("coach_toast_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("coach_toast_enabled", value)}
        isUpdating={isUpdating("coach_toast_enabled")}
        label={t("settings.coach.toastEnabled.title")}
        description={t("settings.coach.toastEnabled.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
