import { type FC, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands, type ModelInfo, type VoiceInfo } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { SUPERTONIC_VOICES, showSupertonicVoice } from "./localEngineOptions";

const SUPERTONIC_MODEL_ID = "supertonic-3";

interface DownloadProgressPayload {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

export const TtsSettings: FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const [voices, setVoices] = useState<VoiceInfo[]>([]);
  const [supertonicInfo, setSupertonicInfo] = useState<ModelInfo | null>(null);
  const [supertonicDownloading, setSupertonicDownloading] = useState(false);
  const [supertonicPercent, setSupertonicPercent] = useState(0);

  useEffect(() => {
    commands
      .ttsListVoices()
      .then((res) => setVoices(res.status === "ok" ? res.data : []))
      .catch(() => setVoices([]));
  }, []);

  const refreshSupertonicInfo = () => {
    commands
      .getModelInfo(SUPERTONIC_MODEL_ID)
      .then((res) => {
        const info = res.status === "ok" ? res.data : null;
        setSupertonicInfo(info);
        // Seed from backend so a remount mid-download shows progress,
        // not the download button (final review M7).
        if (info) setSupertonicDownloading(info.is_downloading);
      })
      .catch(() => setSupertonicInfo(null));
  };

  const localEngine = settings?.local_tts_engine ?? "supertonic";

  useEffect(() => {
    if (localEngine !== "supertonic") return;
    refreshSupertonicInfo();
  }, [localEngine]);

  useEffect(() => {
    if (localEngine !== "supertonic") return;

    const unlistenProgress = listen<DownloadProgressPayload>(
      "model-download-progress",
      (event) => {
        if (event.payload.model_id !== SUPERTONIC_MODEL_ID) return;
        setSupertonicDownloading(true);
        setSupertonicPercent(event.payload.percentage);
      },
    );
    const unlistenComplete = listen<string>(
      "model-download-complete",
      (event) => {
        if (event.payload !== SUPERTONIC_MODEL_ID) return;
        setSupertonicDownloading(false);
        setSupertonicPercent(0);
        refreshSupertonicInfo();
      },
    );
    const unlistenFailed = listen<{ model_id: string; error: string }>(
      "model-download-failed",
      (event) => {
        if (event.payload.model_id !== SUPERTONIC_MODEL_ID) return;
        setSupertonicDownloading(false);
        setSupertonicPercent(0);
      },
    );
    const unlistenCancelled = listen<string>(
      "model-download-cancelled",
      (event) => {
        if (event.payload !== SUPERTONIC_MODEL_ID) return;
        setSupertonicDownloading(false);
        setSupertonicPercent(0);
      },
    );

    return () => {
      void unlistenProgress.then((f) => f());
      void unlistenComplete.then((f) => f());
      void unlistenFailed.then((f) => f());
      void unlistenCancelled.then((f) => f());
    };
  }, [localEngine]);

  const downloadSupertonicVoices = () => {
    setSupertonicDownloading(true);
    setSupertonicPercent(0);
    void commands.downloadModel(SUPERTONIC_MODEL_ID);
  };

  if (!settings) return null;

  const enabled = settings.tts_enabled;
  const voiceId = settings.tts_voice_id ?? "";
  const rate = settings.tts_rate ?? 1.0;

  const test = () => {
    void commands.ttsSpeak(
      "Echo speech engine online. Эхо на связи.",
      voiceId || null,
      rate,
    );
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 p-2">
      <div>
        <h2 className="text-lg font-semibold">{t("settings.tts.title")}</h2>
        <p className="text-sm text-text/50">{t("settings.tts.description")}</p>
      </div>

      {/* Enable */}
      <label className="flex items-center justify-between rounded-xl border border-surface-raised p-4">
        <span className="text-sm">{t("settings.tts.speakAnswers")}</span>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => void updateSetting("tts_enabled", e.target.checked)}
        />
      </label>

      {/* Voice */}
      <div className="rounded-xl border border-surface-raised p-4 space-y-2">
        <label className="text-sm block">{t("settings.tts.voice")}</label>
        <select
          className="w-full bg-transparent border border-edge rounded p-2 text-sm"
          value={voiceId}
          onChange={(e) =>
            void updateSetting("tts_voice_id", e.target.value || null)
          }
        >
          <option value="">{t("settings.tts.voiceAuto")}</option>
          {voices.map((v) => (
            <option key={v.id} value={v.id}>
              {v.display_name} ({v.language})
            </option>
          ))}
        </select>
      </div>

      {/* Local engine */}
      <div className="rounded-xl border border-surface-raised p-4 space-y-2">
        <label className="text-sm block">{t("settings.tts.localEngine")}</label>
        <select
          className="w-full bg-transparent border border-edge rounded p-2 text-sm"
          value={settings.local_tts_engine ?? "supertonic"}
          onChange={(e) =>
            void updateSetting("local_tts_engine", e.target.value)
          }
        >
          <option value="supertonic">
            {t("settings.tts.localEngineSupertonic")}
          </option>
          <option value="piper">{t("settings.tts.localEnginePiper")}</option>
          <option value="kitten">{t("settings.tts.localEngineKitten")}</option>
        </select>
        {showSupertonicVoice(settings.local_tts_engine) &&
          (supertonicInfo && !supertonicInfo.is_downloaded ? (
            <div className="space-y-2">
              {supertonicDownloading ? (
                <div className="space-y-1">
                  <div className="h-2 w-full rounded-full bg-surface-raised overflow-hidden">
                    <div
                      className="h-full rounded-full bg-accent transition-[width]"
                      style={{ width: `${supertonicPercent}%` }}
                    />
                  </div>
                  <p className="text-xs text-text/50">
                    {t("settings.tts.downloadingVoices", {
                      percent: Math.round(supertonicPercent),
                    })}
                  </p>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={downloadSupertonicVoices}
                  className="px-3 py-1.5 rounded bg-accent text-surface text-sm"
                >
                  {t("settings.tts.downloadVoices")}
                </button>
              )}
            </div>
          ) : (
            <>
              <label className="text-sm block">
                {t("settings.tts.supertonicVoice")}
              </label>
              <select
                className="w-full bg-transparent border border-edge rounded p-2 text-sm"
                value={settings.supertonic_voice ?? "M1"}
                onChange={(e) =>
                  void updateSetting("supertonic_voice", e.target.value)
                }
              >
                {SUPERTONIC_VOICES.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </>
          ))}
      </div>

      {/* Rate */}
      <div className="rounded-xl border border-surface-raised p-4 space-y-2">
        <label className="text-sm flex items-center justify-between">
          <span>{t("settings.tts.rate")}</span>
          <span className="text-text/50">{rate.toFixed(1)}×</span>
        </label>
        <input
          type="range"
          min={0.5}
          max={2.0}
          step={0.1}
          value={rate}
          onChange={(e) =>
            void updateSetting("tts_rate", parseFloat(e.target.value))
          }
          className="w-full"
        />
      </div>

      <button
        type="button"
        onClick={test}
        className="px-4 py-2 rounded bg-accent text-white text-sm"
      >
        {t("settings.tts.testVoice")}
      </button>
    </div>
  );
};
