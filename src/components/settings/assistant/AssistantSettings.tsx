import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import {
  SettingContainer,
  SettingsGroup,
  ToggleSwitch,
  Textarea,
} from "@/components/ui";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";

export const AssistantSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();

  const [testInput, setTestInput] = useState("");
  const [testReply, setTestReply] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState("");

  const assistantEnabled = getSetting("assistant_enabled") ?? false;
  const assistantSystemPrompt = getSetting("assistant_system_prompt") ?? "";

  const handleAsk = async () => {
    if (!testInput.trim()) return;

    setIsLoading(true);
    setError("");
    setTestReply("");

    try {
      const result = await commands.assistantAsk(testInput);
      if (result.status === "ok") {
        setTestReply(result.data);
      } else {
        setError(result.error);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleSpeak = async () => {
    if (!testReply) return;
    try {
      await commands.ttsSpeak(testReply, null, 1.0);
    } catch (err) {
      console.error("Failed to speak reply:", err);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.assistant.title")}>
        <ToggleSwitch
          checked={assistantEnabled}
          onChange={(checked) => updateSetting("assistant_enabled", checked)}
          label={t("settings.assistant.enable.label")}
          description={t("settings.assistant.enable.description")}
          descriptionMode="inline"
          grouped={true}
        />

        <SettingContainer
          title={t("settings.assistant.systemPrompt.label")}
          description={t("settings.assistant.systemPrompt.description")}
          layout="stacked"
          grouped={true}
        >
          <Textarea
            value={assistantSystemPrompt}
            onChange={(e) =>
              updateSetting("assistant_system_prompt", e.target.value)
            }
            placeholder={t("settings.assistant.systemPromptPlaceholder")}
            className="min-h-[100px]"
          />
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.assistant.tryIt")}>
        <div className="p-4 rounded-xl border border-surface-raised bg-surface/50 space-y-4">
          <div className="flex gap-2">
            <Input
              value={testInput}
              onChange={(e) => setTestInput(e.target.value)}
              placeholder={t("settings.assistant.ask")}
              className="flex-1"
              onKeyDown={(e) => e.key === "Enter" && handleAsk()}
            />
            <Button
              onClick={handleAsk}
              disabled={isLoading || !testInput.trim()}
              variant="primary"
            >
              {isLoading ? t("common.loading") : t("settings.assistant.ask")}
            </Button>
          </div>

          {error && (
            <div className="text-err text-sm p-2 rounded bg-err/10 border border-err/20">
              {error}
            </div>
          )}

          {testReply && (
            <div className="space-y-3">
              <div className="p-3 rounded bg-surface-raised text-sm whitespace-pre-wrap">
                {testReply}
              </div>
              <Button onClick={handleSpeak} variant="secondary" size="md">
                {t("settings.assistant.speakReply")}
              </Button>
            </div>
          )}
        </div>
      </SettingsGroup>
    </div>
  );
};
