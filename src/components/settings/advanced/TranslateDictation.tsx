import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import type { Lang } from "@/bindings";

// Offline Hy-MT dictation translation. A single dropdown doubles as the on/off
// switch: "No translation" disables it; picking a language enables it and sets the
// target. Reuses the existing transcribe.translate / translateNone i18n keys.
const LANGS: Lang[] = [
  "English",
  "Russian",
  "Ukrainian",
  "Chinese",
  "Spanish",
  "French",
  "German",
  "Italian",
  "Portuguese",
  "Japanese",
  "Korean",
  "Arabic",
  "Turkish",
  "Vietnamese",
  "Polish",
  "Czech",
  "Dutch",
  "Hindi",
  "Persian",
  "Hebrew",
  "Thai",
  "Indonesian",
];

export const TranslateDictation: React.FC<{ grouped?: boolean }> = React.memo(
  () => {
    const { t } = useTranslation();
    const { getSetting, updateSetting } = useSettings();

    const enabled = getSetting("translate_enabled") ?? false;
    const target = getSetting("translate_target") ?? "English";
    const value = enabled ? target : "";

    const onChange = async (v: string) => {
      if (!v) {
        await updateSetting("translate_enabled", false);
      } else {
        await updateSetting("translate_target", v as Lang);
        await updateSetting("translate_enabled", true);
      }
    };

    return (
      <label className="flex items-center justify-between gap-2 text-sm py-2">
        <span>{t("settings.transcribe.translate")}</span>
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="text-sm bg-transparent border border-edge rounded px-2 py-1"
        >
          <option value="">{t("settings.transcribe.translateNone")}</option>
          {LANGS.map((l) => (
            <option key={l} value={l}>
              {l}
            </option>
          ))}
        </select>
      </label>
    );
  },
);

TranslateDictation.displayName = "TranslateDictation";
