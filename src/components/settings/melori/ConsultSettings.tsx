import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { consultApi, type Health } from "@/consult/api";
import { useSettings } from "@/hooks/useSettings";

const fieldClass =
  "w-full border-0 border-b-2 border-edge-strong bg-transparent px-0 py-2 font-mono text-sm text-primary outline-none focus:border-accent";

export const EngineSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [base, setBase] = useState("");
  const [model, setModel] = useState("");
  const [corpus, setCorpus] = useState("");
  const [embedModel, setEmbedModel] = useState("");
  const [mirror, setMirror] = useState("");
  const [restarting, setRestarting] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  useEffect(() => {
    setBase(settings?.llm_base_url ?? "");
    setModel(settings?.llm_model ?? "");
    setCorpus(settings?.corpus_dir ?? "");
    setEmbedModel(settings?.llm_embed_model ?? "");
    setMirror(settings?.model_mirror_url ?? "");
  }, [settings]);
  useEffect(() => {
    void consultApi
      .health()
      .then(setHealth)
      .catch(() => setHealth(null));
  }, [restarting]);
  const apply = async () => {
    setRestarting(true);
    const result = await commands.changeLlmSettings(
      base,
      model,
      corpus,
      embedModel,
    );
    if (result.status === "ok") await refreshSettings();
    setRestarting(false);
  };
  const applyMirror = async () => {
    const result = await commands.changeModelMirrorUrlSetting(mirror);
    if (result.status === "ok") await refreshSettings();
  };
  return (
    <div className="w-full max-w-5xl grid grid-cols-1 gap-8 md:grid-cols-2">
      <section className="space-y-4">
        <h1 className="font-display text-2xl text-primary">
          {t("settings.melori.engine.title")}
        </h1>
        <label className="block text-sm text-secondary">
          {t("settings.melori.engine.baseUrl")}
          <input
            className={fieldClass}
            value={base}
            onChange={(e) => setBase(e.target.value)}
          />
        </label>
        <label className="block text-sm text-secondary">
          {t("settings.melori.engine.model")}
          <input
            className={fieldClass}
            value={model}
            onChange={(e) => setModel(e.target.value)}
          />
        </label>
        <label className="block text-sm text-secondary">
          {t("settings.melori.engine.corpus")}
          <input
            className={fieldClass}
            value={corpus}
            onChange={(e) => setCorpus(e.target.value)}
          />
        </label>
        <label className="block text-sm text-secondary">
          {t("settings.melori.engine.embedModel")}
          <input
            className={fieldClass}
            value={embedModel}
            onChange={(e) => setEmbedModel(e.target.value)}
          />
          <span className="mt-1 block text-xs text-secondary">
            {t("settings.melori.engine.embedModelHint")}
          </span>
        </label>
        <p
          className={
            health?.llm_local ? "text-sm text-primary" : "text-sm text-warn"
          }
        >
          {health
            ? health.llm_local
              ? t("settings.melori.engine.local")
              : t("settings.melori.engine.remote")
            : t("settings.melori.engine.checking")}
        </p>
        <button
          type="button"
          onClick={() => void apply()}
          disabled={restarting}
          className="bg-accent px-4 py-2 text-sm text-ground disabled:opacity-50"
        >
          {restarting
            ? t("settings.melori.engine.restarting")
            : t("settings.melori.apply")}
        </button>
        <label className="block text-sm text-secondary">
          {t("settings.melori.engine.mirror")}
          <input
            className={fieldClass}
            value={mirror}
            onChange={(e) => setMirror(e.target.value)}
            onBlur={() => void applyMirror()}
            placeholder="https://…"
          />
        </label>
        <p className="text-xs text-secondary">
          {t("settings.melori.engine.mirrorHint")}
        </p>
      </section>
      <section className="space-y-6">
        <PrivacySettings />
        <AppearanceSettings />
      </section>
    </div>
  );
};

export const PrivacySettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  return (
    <section className="space-y-3">
      <h2 className="font-display text-xl text-primary">
        {t("settings.melori.privacy.title")}
      </h2>
      <label className="flex gap-3 text-sm text-primary">
        <input
          type="checkbox"
          checked={!!settings?.hide_notes_from_screen_share}
          onChange={async (e) => {
            await commands.changeHideNotesFromScreenShareSetting(
              e.target.checked,
            );
            await refreshSettings();
          }}
        />
        {t("settings.melori.privacy.hideNotes")}
      </label>
      <p className="text-xs text-secondary">
        {t("settings.melori.privacy.hint")}
      </p>
    </section>
  );
};

export const AppearanceSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  // settings written before ui_theme existed have no field — that is "system"
  const current = settings?.ui_theme ?? "system";
  return (
    <section className="space-y-3">
      <h2 className="font-display text-xl text-primary">
        {t("settings.melori.appearance.title")}
      </h2>
      <div
        role="radiogroup"
        aria-label={t("settings.melori.appearance.title")}
        className="flex border border-edge-strong"
      >
        {(
          [
            ["system", "system"],
            ["day", "day"],
            ["evening", "evening"],
          ] as const
        ).map(([value, key]) => (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={current === value}
            onClick={async () => {
              await commands.changeUiThemeSetting(value);
              await refreshSettings();
            }}
            className={`px-3 py-2 text-sm ${current === value ? "bg-primary text-ground" : "text-primary"}`}
          >
            {t(`settings.melori.appearance.${key}`)}
          </button>
        ))}
      </div>
    </section>
  );
};
