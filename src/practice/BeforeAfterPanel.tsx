import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { Mic, Square, GitCompareArrows } from "lucide-react";

interface PracticeTake {
  path: string;
  label: string;
  created_unix: number;
}

/**
 * «До/после» (паттерн школы: слышимое доказательство прогресса): одна и та же
 * 60-секундная диагностика — свободное чтение — записывается в первый день
 * («Точка А») и повторяется по мере занятий. Первая запись и последняя играются
 * рядом. Файлы сохраняет `practice_save_take` (app-data/practice, asset-scope).
 */
export function BeforeAfterPanel() {
  const { t } = useTranslation();
  const [takes, setTakes] = useState<PracticeTake[]>([]);
  const [isRecording, setIsRecording] = useState(false);
  const [saving, setSaving] = useState(false);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const refresh = () =>
    invoke<PracticeTake[]>("practice_takes")
      .then(setTakes)
      .catch(() => setTakes([]));
  useEffect(() => {
    refresh();
  }, []);

  const startRec = async () => {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const rec = new MediaRecorder(stream);
    recRef.current = rec;
    chunksRef.current = [];
    rec.ondataavailable = (e) => chunksRef.current.push(e.data);
    rec.onstop = async () => {
      stream.getTracks().forEach((tr) => tr.stop());
      setSaving(true);
      try {
        const blob = new Blob(chunksRef.current, { type: "audio/webm" });
        const data = Array.from(new Uint8Array(await blob.arrayBuffer()));
        await invoke("practice_save_take", { label: "diagnostic", data });
        await refresh();
      } finally {
        setSaving(false);
      }
    };
    rec.start();
    setIsRecording(true);
  };
  const stopRec = () => {
    recRef.current?.stop();
    setIsRecording(false);
  };

  const baseline = takes[0];
  const latest = takes.length > 1 ? takes[takes.length - 1] : undefined;
  const fmtDate = (unix: number) => new Date(unix * 1000).toLocaleDateString();

  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-2xl border border-surface-raised bg-surface/10 p-6 flex flex-col gap-3">
        <div className="flex items-center gap-3">
          <GitCompareArrows className="w-5 h-5 text-accent" />
          <h3 className="text-lg font-black text-white">
            {t("settings.practice.beforeAfterTitle")}
          </h3>
        </div>
        <p className="text-sm text-secondary">
          {t("settings.practice.beforeAfterHint")}
        </p>
        <div className="flex items-center gap-3">
          {!isRecording ? (
            <button
              onClick={startRec}
              disabled={saving}
              className="flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-xl bg-accent text-white hover:bg-accent-hot transition-all disabled:opacity-50"
            >
              <Mic className="w-4 h-4" />
              {t(
                baseline
                  ? "settings.practice.recordCheck"
                  : "settings.practice.recordBaseline",
              )}
            </button>
          ) : (
            <button
              onClick={stopRec}
              className="flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-xl bg-err text-white transition-all"
            >
              <Square className="w-4 h-4" /> {t("settings.practice.stop")}
            </button>
          )}
          {saving && <span className="text-xs text-secondary">…</span>}
        </div>
      </div>

      {!baseline ? (
        <p className="text-xs text-secondary text-center py-4">
          {t("settings.practice.noTakes")}
        </p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="rounded-2xl border border-surface-raised bg-surface/10 p-5 flex flex-col gap-2">
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
              {t("settings.practice.baseline")} ·{" "}
              {fmtDate(baseline.created_unix)}
            </span>
            <audio
              controls
              src={convertFileSrc(baseline.path)}
              className="w-full h-9"
            />
          </div>
          <div className="rounded-2xl border border-surface-raised bg-surface/10 p-5 flex flex-col gap-2">
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
              {latest
                ? `${t("settings.practice.latest")} · ${fmtDate(latest.created_unix)}`
                : t("settings.practice.latestPending")}
            </span>
            {latest ? (
              <audio
                controls
                src={convertFileSrc(latest.path)}
                className="w-full h-9"
              />
            ) : (
              <p className="text-xs text-secondary">
                {t("settings.practice.latestHint")}
              </p>
            )}
          </div>
        </div>
      )}

      {takes.length > 2 && (
        <p className="text-[10px] text-secondary text-center">
          {t("settings.practice.allTakes", { count: takes.length })}
        </p>
      )}
    </div>
  );
}
