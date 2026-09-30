import { open } from "@tauri-apps/plugin-dialog";
import { type FC } from "react";
import { useTranslation } from "react-i18next";
import { ProgressBar } from "@/components/ui/ProgressBar";
import {
  useTranscribeStore,
  type TranscribeFormat,
} from "@/stores/transcribeStore";

// Hy-MT target languages (ISO code -> English name). The backend lower-cases the
// code via Lang::from_code, so casing here is cosmetic.
const TRANSLATE_LANGS: ReadonlyArray<[string, string]> = [
  ["en", "English"],
  ["ru", "Russian"],
  ["uk", "Ukrainian"],
  ["zh", "Chinese"],
  ["zh-Hant", "Traditional Chinese"],
  ["es", "Spanish"],
  ["fr", "French"],
  ["de", "German"],
  ["it", "Italian"],
  ["pt", "Portuguese"],
  ["ja", "Japanese"],
  ["ko", "Korean"],
  ["ar", "Arabic"],
  ["tr", "Turkish"],
  ["vi", "Vietnamese"],
  ["pl", "Polish"],
  ["cs", "Czech"],
  ["nl", "Dutch"],
  ["hi", "Hindi"],
  ["fa", "Persian"],
  ["he", "Hebrew"],
  ["th", "Thai"],
  ["id", "Indonesian"],
];

export const TranscribeFile: FC = () => {
  const { t } = useTranslation();
  const {
    path,
    timestamps,
    format,
    diarize,
    speakers,
    outputPath,
    busy,
    result,
    error,
    progress,
    cancelling,
    savedTo,
    setPath,
    setTimestamps,
    setFormat,
    setDiarize,
    setSpeakers,
    setOutputPath,
    translateTarget,
    setTranslateTarget,
    run,
    cancel,
  } = useTranscribeStore();

  const pickFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Audio/Video",
          extensions: [
            "mp3",
            "wav",
            "m4a",
            "mp4",
            "mkv",
            "mov",
            "flac",
            "ogg",
            "webm",
          ],
        },
      ],
    });
    if (typeof selected === "string") setPath(selected);
  };

  const pickOutput = async () => {
    const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
    const ext = timestamps ? format : "md";
    const base = path
      ? path
          .replace(/\.[^/.]+$/, "")
          .split(/[\\/]/)
          .pop()
      : "transcript";
    const target = await saveDialog({ defaultPath: `${base}.${ext}` });
    if (typeof target === "string") setOutputPath(target);
  };

  const save = async () => {
    const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    const target = await saveDialog({
      defaultPath: `transcript.${timestamps ? format : "md"}`,
    });
    if (typeof target === "string") await writeTextFile(target, result);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4 p-2">
      <h2 className="text-lg font-semibold">
        {t("settings.transcribe.title")}
      </h2>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={pickFile}
          className="px-3 py-2 rounded bg-accent text-white text-sm"
        >
          {t("settings.transcribe.pickFile")}
        </button>
        <span className="text-xs text-text/60 truncate">
          {path ?? t("settings.transcribe.noFile")}
        </span>
      </div>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={pickOutput}
          className="px-3 py-2 rounded border border-edge text-sm"
        >
          {t("settings.transcribe.saveTo")}
        </button>
        <span className="text-xs text-text/60 truncate">
          {outputPath ?? ""}
        </span>
      </div>

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={timestamps}
          onChange={(e) => setTimestamps(e.target.checked)}
        />
        {t("settings.transcribe.timestamps")}
      </label>

      {timestamps && (
        <select
          value={format}
          onChange={(e) => setFormat(e.target.value as TranscribeFormat)}
          className="text-sm bg-transparent border border-edge rounded px-2 py-1"
        >
          <option value="inline">{t("settings.transcribe.fmtInline")}</option>
          <option value="srt">SRT</option>
          <option value="vtt">VTT</option>
          <option value="json">{t("settings.transcribe.fmtJson")}</option>
          <option value="karaoke">{t("settings.transcribe.fmtKaraoke")}</option>
        </select>
      )}

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={diarize}
          onChange={(e) => setDiarize(e.target.checked)}
        />
        {t("settings.transcribe.diarize")}
      </label>

      {diarize && (
        <input
          type="number"
          min={1}
          placeholder={t("settings.transcribe.speakersPlaceholder")}
          value={speakers}
          onChange={(e) => setSpeakers(e.target.value)}
          className="text-sm bg-transparent border border-edge rounded px-2 py-1 w-40"
        />
      )}

      {diarize && (
        <p className="text-xs text-text/50 whitespace-pre-line leading-relaxed border-l-2 border-accent-hot/40 pl-3">
          {t("settings.transcribe.diarizeCliNote")}
        </p>
      )}

      <label className="flex items-center gap-2 text-sm">
        {t("settings.transcribe.translate")}
        <select
          value={translateTarget ?? ""}
          onChange={(e) => setTranslateTarget(e.target.value || null)}
          className="text-sm bg-transparent border border-edge rounded px-2 py-1"
        >
          <option value="">{t("settings.transcribe.translateNone")}</option>
          {TRANSLATE_LANGS.map(([code, name]) => (
            <option key={code} value={code}>
              {name}
            </option>
          ))}
        </select>
      </label>

      {busy && progress && (
        <div className="space-y-2">
          <ProgressBar
            percent={progress.percent}
            label={t(
              `settings.transcribe.progress.${progress.phase === "loading_model" ? "loadingModel" : progress.phase}`,
            )}
          />
          <button
            type="button"
            onClick={cancel}
            disabled={cancelling}
            className="px-3 py-1.5 rounded border border-err/60 text-err text-xs disabled:opacity-40"
          >
            {t("settings.transcribe.cancel")}
          </button>
        </div>
      )}
      {cancelling && !busy && (
        <div className="text-sm text-text/60">
          {t("settings.transcribe.cancelled")}
        </div>
      )}

      <div className="flex gap-2">
        <button
          type="button"
          disabled={!path || busy}
          onClick={run}
          className="px-3 py-2 rounded bg-ok text-white text-sm disabled:opacity-40"
        >
          {busy
            ? t("settings.transcribe.working")
            : t("settings.transcribe.run")}
        </button>
        {result && (
          <>
            <button
              type="button"
              onClick={() => navigator.clipboard.writeText(result)}
              className="px-3 py-2 rounded border border-edge text-sm"
            >
              {t("settings.transcribe.copy")}
            </button>
            <button
              type="button"
              onClick={save}
              className="px-3 py-2 rounded border border-edge text-sm"
            >
              {t("settings.transcribe.saveAs")}
            </button>
          </>
        )}
      </div>

      {error && <div className="text-sm text-err">{error}</div>}
      {savedTo && (
        <div className="text-sm text-ok">
          {t("settings.transcribe.savedTo", { path: savedTo })}
        </div>
      )}
      {result && (
        <textarea
          readOnly
          value={result}
          className="w-full h-64 text-xs font-mono bg-black/30 border border-surface-raised rounded p-2"
        />
      )}
    </div>
  );
};
