import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { Input } from "../../ui/Input";
import { useSettings } from "../../../hooks/useSettings";

interface CaptureFolderProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CaptureFolder: React.FC<CaptureFolderProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const folderValue = getSetting("capture_folder") ?? "";
    const phrasesValue = getSetting("capture_trigger_phrases") ?? "";

    return (
      <>
        <SettingContainer
          title={t("settings.capture.dirLabel")}
          description={t("settings.capture.dirDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Input
            type="text"
            value={folderValue}
            onChange={(e) => updateSetting("capture_folder", e.target.value)}
            placeholder={t("settings.capture.dirPlaceholder")}
            disabled={isUpdating("capture_folder")}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.capture.phrasesLabel")}
          description={t("settings.capture.phrasesDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Input
            type="text"
            value={phrasesValue}
            onChange={(e) =>
              updateSetting("capture_trigger_phrases", e.target.value)
            }
            placeholder={t("settings.capture.phrasesPlaceholder")}
            disabled={isUpdating("capture_trigger_phrases")}
          />
        </SettingContainer>
      </>
    );
  },
);
