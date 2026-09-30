import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface SpokenListsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const SpokenLists: React.FC<SpokenListsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("spoken_lists_enabled") ?? true;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("spoken_lists_enabled", value)}
        isUpdating={isUpdating("spoken_lists_enabled")}
        label={t("settings.advanced.spokenLists.label")}
        description={t("settings.advanced.spokenLists.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
