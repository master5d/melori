import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import type { Snippet } from "@/bindings";

interface SnippetsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const Snippets: React.FC<SnippetsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [trigger, setTrigger] = useState("");
    const [text, setText] = useState("");
    const snippets = getSetting("snippets") || [];

    const handleAdd = () => {
      const trimmedTrigger = trigger.trim();
      const trimmedText = text.trim();
      if (!trimmedTrigger || !trimmedText) return;
      if (
        snippets.some(
          (s) => s.trigger.toLowerCase() === trimmedTrigger.toLowerCase(),
        )
      ) {
        toast.error(
          t("settings.advanced.snippets.duplicate", {
            trigger: trimmedTrigger,
          }),
        );
        return;
      }
      updateSetting("snippets", [
        ...snippets,
        { trigger: trimmedTrigger, text: trimmedText },
      ] as Snippet[]);
      setTrigger("");
      setText("");
    };

    const handleRemove = (triggerToRemove: string) => {
      updateSetting(
        "snippets",
        snippets.filter((s) => s.trigger !== triggerToRemove),
      );
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAdd();
      }
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.snippets.title")}
          description={t("settings.advanced.snippets.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className="max-w-32"
              value={trigger}
              onChange={(e) => setTrigger(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.snippets.triggerPlaceholder")}
              variant="compact"
              disabled={isUpdating("snippets")}
            />
            <Input
              type="text"
              className="max-w-48"
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={t("settings.advanced.snippets.textPlaceholder")}
              variant="compact"
              disabled={isUpdating("snippets")}
            />
            <Button
              onClick={handleAdd}
              disabled={
                !trigger.trim() || !text.trim() || isUpdating("snippets")
              }
              variant="primary"
              size="md"
            >
              {t("settings.advanced.snippets.add")}
            </Button>
          </div>
        </SettingContainer>
        {snippets.length > 0 && (
          <div
            className={`px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"} flex flex-col gap-1`}
          >
            {snippets.map((s) => (
              <div
                key={s.trigger}
                className="flex items-center justify-between gap-2 text-sm"
              >
                <span className="truncate">
                  <span className="font-medium">{s.trigger}</span>
                  <span className="text-mid-gray"> → {s.text}</span>
                </span>
                <Button
                  onClick={() => handleRemove(s.trigger)}
                  disabled={isUpdating("snippets")}
                  variant="secondary"
                  size="sm"
                  className="inline-flex items-center cursor-pointer shrink-0"
                  aria-label={t("settings.advanced.snippets.remove", {
                    trigger: s.trigger,
                  })}
                >
                  <svg
                    className="w-3 h-3"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </Button>
              </div>
            ))}
          </div>
        )}
      </>
    );
  },
);
