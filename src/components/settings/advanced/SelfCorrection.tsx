import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface SelfCorrectionProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const SelfCorrection: React.FC<SelfCorrectionProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("self_correction_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("self_correction_enabled", value)}
        isUpdating={isUpdating("self_correction_enabled")}
        label={t("settings.advanced.selfCorrection.label")}
        description={t("settings.advanced.selfCorrection.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
