import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface AutoCapitalizeProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AutoCapitalize: React.FC<AutoCapitalizeProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const autoCapitalize = getSetting("auto_capitalize") ?? true;

    return (
      <ToggleSwitch
        checked={autoCapitalize}
        onChange={(enabled) => updateSetting("auto_capitalize", enabled)}
        isUpdating={isUpdating("auto_capitalize")}
        label={t("settings.advanced.autoCapitalize.label")}
        description={t("settings.advanced.autoCapitalize.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
