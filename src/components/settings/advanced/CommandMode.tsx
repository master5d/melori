import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface CommandModeProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CommandMode: React.FC<CommandModeProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("command_mode_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("command_mode_enabled", value)}
        isUpdating={isUpdating("command_mode_enabled")}
        label={t("settings.advanced.commandMode.label")}
        description={t("settings.advanced.commandMode.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
