import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface DevDictionaryProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const DevDictionary: React.FC<DevDictionaryProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("dev_dictionary_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("dev_dictionary_enabled", value)}
        isUpdating={isUpdating("dev_dictionary_enabled")}
        label={t("settings.advanced.devDictionary.label")}
        description={t("settings.advanced.devDictionary.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
