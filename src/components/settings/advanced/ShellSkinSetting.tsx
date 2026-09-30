import React from "react";
import { useTranslation } from "react-i18next";
import {
  PanelLeft,
  LayoutGrid,
  GalleryHorizontal,
  type LucideIcon,
} from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "../../../hooks/useSettings";

type ShellSkinOption = "rail" | "home" | "deck";

const OPTIONS: { value: ShellSkinOption; icon: LucideIcon }[] = [
  { value: "rail", icon: PanelLeft },
  { value: "home", icon: LayoutGrid },
  { value: "deck", icon: GalleryHorizontal },
];

interface ShellSkinSettingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** 3-way radio (with preview icons) for the Warm Studio suite shell skin —
 * "rail" | "home" | "deck". All three are implemented and selectable: "rail"
 * (Phase A Task 2), "home" (Task 3), and "deck" (Task 4). */
export const ShellSkinSetting: React.FC<ShellSkinSettingProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const selected = (getSetting("shell_skin") || "home") as ShellSkinOption;

    return (
      <SettingContainer
        title={t("settings.advanced.shellSkin.title")}
        description={t("settings.advanced.shellSkin.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        tooltipPosition="bottom"
      >
        <div
          role="radiogroup"
          aria-label={t("settings.advanced.shellSkin.title")}
          className="flex gap-2"
        >
          {OPTIONS.map(({ value, icon: Icon }) => {
            const isActive = selected === value;
            return (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={isActive}
                disabled={isUpdating("shell_skin")}
                onClick={() => updateSetting("shell_skin", value)}
                className={`flex flex-col items-center gap-1.5 rounded-xl border px-3 py-2.5 text-xs transition-colors ${
                  isActive
                    ? "border-accent bg-accent/10 text-accent"
                    : "border-edge text-secondary hover:text-primary hover:border-accent/40"
                }`}
              >
                <Icon width={18} height={18} />
                <span>{t(`settings.advanced.shellSkin.options.${value}`)}</span>
              </button>
            );
          })}
        </div>
      </SettingContainer>
    );
  },
);
