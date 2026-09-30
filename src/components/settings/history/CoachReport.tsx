import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type Baseline } from "@/bindings";

export type WordCount = { word: string; count: number };
export type CoachMetrics = {
  word_count: number;
  duration_ms: number;
  wpm: number;
  pace_band: "Slow" | "Good" | "Fast";
  fillers: WordCount[];
  filler_total: number;
  filler_rate: number;
  weak_words: WordCount[];
};

export function parseCoachMetrics(
  raw: string | null | undefined,
): CoachMetrics | null {
  if (!raw) return null;
  try {
    return JSON.parse(raw) as CoachMetrics;
  } catch {
    return null; // malformed/old data -> no report
  }
}

export function CoachReport({
  metricsJson,
}: {
  metricsJson: string | null | undefined;
}) {
  const { t } = useTranslation();
  const [baseline, setBaseline] = useState<Baseline | null>(null);
  useEffect(() => {
    commands
      .getCoachBaseline()
      .then((res) => setBaseline(res.status === "ok" ? res.data : null))
      .catch(() => setBaseline(null));
  }, []);
  const m = parseCoachMetrics(metricsJson);
  if (!m) return null;

  return (
    <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-text/60">
      <span>
        {m.wpm > 0 ? `${m.wpm} ${t("settings.coach.wpm")}` : "—"} ·{" "}
        {t(`settings.coach.pace.${m.pace_band.toLowerCase()}`)}
      </span>
      <span>
        {t("settings.coach.fillers")}: {m.filler_total} (
        {m.filler_rate.toFixed(1)}%)
      </span>
      {baseline && m.filler_rate !== baseline.avg_filler_rate && (
        <span className="text-text/50">
          {m.filler_rate < baseline.avg_filler_rate
            ? `↓ ${t("settings.coach.belowAvg")}`
            : `↑ ${t("settings.coach.aboveAvg")}`}
        </span>
      )}
      {m.fillers.slice(0, 6).map((f) => (
        <span key={f.word} className="rounded bg-mid-gray/15 px-1.5 py-0.5">
          {f.word} ×{f.count}
        </span>
      ))}
    </div>
  );
}
