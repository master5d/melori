import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface AutoPunctuateProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AutoPunctuate: React.FC<AutoPunctuateProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const autoPunctuate = getSetting("auto_punctuate") ?? true;

    return (
      <ToggleSwitch
        checked={autoPunctuate}
        onChange={(enabled) => updateSetting("auto_punctuate", enabled)}
        isUpdating={isUpdating("auto_punctuate")}
        label={t("settings.advanced.autoPunctuate.label")}
        description={t("settings.advanced.autoPunctuate.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
