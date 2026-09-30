import React, { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { commands, type PhonemeReport, type ScoreReport } from "@/bindings";
import { SettingsGroup, ToggleSwitch } from "@/components/ui";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";

type Lang = "en-us" | "ru";

export const TutorSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();

  const [reference, setReference] = useState("");
  const [spoken, setSpoken] = useState("");
  const [lang, setLang] = useState<Lang>("en-us");
  const [report, setReport] = useState<PhonemeReport | null>(null);
  const [wordReport, setWordReport] = useState<ScoreReport | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [micError, setMicError] = useState(false);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const tutorEnabled = getSetting("tutor_enabled") ?? false;

  const startRecording = async () => {
    setMicError(false);
    setReport(null);
    setWordReport(null);
    setUnavailable(false);
    chunksRef.current = [];
    let stream: MediaStream | null = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const rec = new MediaRecorder(stream, { mimeType: "audio/webm" });
      const activeStream = stream;
      mediaRecorderRef.current = rec;
      rec.ondataavailable = (e) => {
        if (e.data.size > 0) chunksRef.current.push(e.data);
      };
      rec.onstop = async () => {
        try {
          await transcribe(new Blob(chunksRef.current, { type: "audio/webm" }));
        } finally {
          activeStream.getTracks().forEach((tr) => tr.stop());
        }
      };
      rec.start();
      setIsRecording(true);
    } catch {
      stream?.getTracks().forEach((tr) => tr.stop());
      setMicError(true);
    }
  };

  const stopRecording = () => {
    if (mediaRecorderRef.current && isRecording) {
      mediaRecorderRef.current.stop();
      setIsRecording(false);
    }
  };

  const transcribe = async (blob: Blob) => {
    setIsTranscribing(true);
    try {
      const dir = await commands.getAppDirPath();
      if (dir.status !== "ok") return;
      const tempPath = `${dir.data}/temp_tutor.webm`;
      const bytes = new Uint8Array(await blob.arrayBuffer());
      const { writeFile } = await import("@tauri-apps/plugin-fs");
      await writeFile(tempPath, bytes);
      const res = await commands.transcribeFileToString(
        tempPath,
        lang === "ru" ? "ru" : "en",
        null,
        false,
        null,
        "plain",
        null,
      );
      if (res.status === "ok") setSpoken(res.data.trim());
    } finally {
      setIsTranscribing(false);
    }
  };

  const handleScore = async () => {
    setReport(null);
    setUnavailable(false);
    setWordReport(null);
    const res = await commands.phonemeCompare(reference, spoken, lang);
    if (res.status === "ok") {
      setReport(res.data);
    } else {
      if (res.error === "unavailable") setUnavailable(true);
      else console.error("phoneme_compare failed:", res.error);
      setWordReport(await commands.tutorScore(reference, spoken));
    }
  };

  const cellClass = (status: string) =>
    status === "ok"
      ? "text-ok bg-ok/10 border-ok/20"
      : status === "ins"
        ? "text-warn bg-warn/10 border-warn/20"
        : "text-err bg-err/10 border-err/20";

  return (
    <div className="flex flex-col animate-in fade-in duration-500">
      <SettingsGroup title={t("sidebar.tutor")}>
        <ToggleSwitch
          checked={tutorEnabled}
          onChange={(checked) => updateSetting("tutor_enabled", checked)}
          label={t("settings.tutor.enable.label")}
          description={t("settings.tutor.enable.description")}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.tutor.practice")}>
        <div className="flex flex-col gap-4 p-6">
          <div className="flex gap-2">
            {(["en-us", "ru"] as Lang[]).map((l) => (
              <button
                key={l}
                type="button"
                onClick={() => setLang(l)}
                className={`px-3 py-1.5 rounded-lg text-sm font-bold border ${
                  lang === l
                    ? "text-accent bg-accent/10 border-accent/30"
                    : "text-secondary border-surface-raised"
                }`}
              >
                {l}
              </button>
            ))}
          </div>

          <div className="flex flex-col gap-2">
            <label className="text-xs font-bold text-secondary uppercase tracking-widest ml-1">
              {t("settings.tutor.reference")}
            </label>
            <Input
              value={reference}
              onChange={(e) => setReference(e.target.value)}
              placeholder="..."
              className="bg-ground/50 border-surface-raised"
            />
          </div>

          <div className="flex items-center gap-3">
            <Button
              onClick={isRecording ? stopRecording : startRecording}
              className="py-3 bg-accent hover:bg-accent border-none"
            >
              {isRecording
                ? t("settings.tutor.stopRecording")
                : t("settings.tutor.record")}
            </Button>
            {isTranscribing && (
              <span className="text-sm text-secondary">
                {t("settings.tutor.transcribing")}
              </span>
            )}
            {spoken && !isTranscribing && (
              <span className="text-sm text-primary">
                {t("settings.tutor.spokenHeard", { spoken })}
              </span>
            )}
          </div>
          {micError && (
            <div className="text-sm text-err">
              {t("settings.tutor.micError")}
            </div>
          )}

          <Button
            onClick={handleScore}
            disabled={!reference || !spoken || isRecording || isTranscribing}
            className="mt-1 py-3 bg-accent hover:bg-accent border-none"
          >
            {t("settings.tutor.score")}
          </Button>

          {unavailable && (
            <div className="text-sm text-warn">
              {t("settings.tutor.phonemeUnavailable")}
            </div>
          )}

          {report && (
            <div className="mt-4 flex flex-col gap-5 p-5 bg-surface rounded-2xl border border-accent/20">
              <div className="flex items-center justify-between">
                <span className="text-4xl font-black text-primary tracking-tighter">
                  {report.overall}%
                </span>
                <p className="text-sm text-primary italic max-w-[60%] text-right">
                  {report.note}
                </p>
              </div>
              <div className="flex flex-wrap gap-4 pt-4 border-t border-surface-raised/50">
                {report.words.map((word, wi) => (
                  <div key={wi} className="flex flex-col gap-1">
                    <span className="text-sm font-bold text-primary">
                      {word.text}
                    </span>
                    <div className="flex gap-1">
                      {word.cells.map((cell, ci) => (
                        <span
                          key={ci}
                          className={`px-2 py-1 rounded-md text-sm font-mono border ${cellClass(cell.status)}`}
                          title={
                            cell.said && cell.status !== "ok"
                              ? t("settings.tutor.youSaid", { ipa: cell.said })
                              : undefined
                          }
                        >
                          {cell.ipa}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {wordReport && (
            <div className="mt-2 flex flex-wrap gap-2">
              {wordReport.words.map((w, i) => (
                <div
                  key={i}
                  className={`px-3 py-1.5 rounded-lg text-sm font-bold border ${
                    w.matched
                      ? "text-ok bg-ok/10 border-ok/20"
                      : "text-err bg-err/10 border-err/20"
                  }`}
                >
                  {w.reference}
                </div>
              ))}
            </div>
          )}
        </div>
      </SettingsGroup>
    </div>
  );
};
