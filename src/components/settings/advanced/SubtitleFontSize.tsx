import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { type SubtitleFontSize } from "@/bindings";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";

interface SubtitleFontSizeProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const SubtitleFontSizeSetting: React.FC<SubtitleFontSizeProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();

  const options = [
    {
      value: "small" as SubtitleFontSize,
      label: t("settings.advanced.subtitles.fontSize.options.small"),
    },
    {
      value: "medium" as SubtitleFontSize,
      label: t("settings.advanced.subtitles.fontSize.options.medium"),
    },
    {
      value: "large" as SubtitleFontSize,
      label: t("settings.advanced.subtitles.fontSize.options.large"),
    },
  ];

  const currentValue = getSetting("subtitle_font_size") ?? "medium";

  return (
    <SettingContainer
      title={t("settings.advanced.subtitles.fontSize.label")}
      description={t("settings.advanced.subtitles.fontSize.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <Dropdown
        options={options}
        selectedValue={currentValue}
        onSelect={(value) =>
          updateSetting("subtitle_font_size", value as SubtitleFontSize)
        }
        disabled={false}
      />
    </SettingContainer>
  );
};
