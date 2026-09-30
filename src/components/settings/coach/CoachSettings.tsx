import { type FC, type ReactNode, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type CoachDashboard, type TrendWindow } from "@/bindings";
import { Sparkline } from "../../ui/Sparkline";

const Card: FC<{ label: string; value: string; children?: ReactNode }> = ({
  label,
  value,
  children,
}) => (
  <div className="rounded-xl border border-surface-raised p-4">
    <div className="text-2xl font-bold">{value}</div>
    <div className="text-xs text-text/50">{label}</div>
    <div className="text-xs mt-1">{children}</div>
  </div>
);

export const CoachSettings: FC = () => {
  const { t } = useTranslation();
  const [window, setWindow] = useState<TrendWindow>("Days30");
  const [data, setData] = useState<CoachDashboard | null>(null);

  useEffect(() => {
    commands
      .getCoachDashboard(window)
      .then((res) => setData(res.status === "ok" ? res.data : null))
      .catch(() => setData(null));
  }, [window]);

  if (!data || (data.summary.session_count === 0 && data.trend.length === 0)) {
    return (
      <div className="max-w-3xl w-full mx-auto p-6 text-text/60">
        {t("settings.coach.empty")}
      </div>
    );
  }

  const { summary, trend, current_streak, best_streak } = data;
  const wpmDelta = summary.avg_wpm - summary.prev_avg_wpm;
  const fillerDelta = summary.avg_filler_rate - summary.prev_avg_filler_rate;
  const arrow = (d: number, lowerBetter = false) =>
    d === 0 ? "" : (lowerBetter ? d < 0 : d > 0) ? "↑" : "↓";
  const good = (d: number, lowerBetter = false) =>
    (lowerBetter ? d < 0 : d > 0) ? "text-ok" : "text-accent-hot";

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 p-2">
      {/* Period summary */}
      <div className="grid grid-cols-3 gap-3">
        <Card label={t("settings.coach.avgWpm")} value={`${summary.avg_wpm}`}>
          {summary.prev_session_count > 0 && (
            <span className={good(wpmDelta)}>
              {arrow(wpmDelta)} {Math.abs(wpmDelta)}{" "}
              {t("settings.coach.vsPrev")}
            </span>
          )}
        </Card>
        <Card
          label={t("settings.coach.fillerRate")}
          value={`${summary.avg_filler_rate.toFixed(1)}%`}
        >
          {summary.prev_session_count > 0 && (
            <span className={good(fillerDelta, true)}>
              {arrow(fillerDelta, true)} {Math.abs(fillerDelta).toFixed(1)}{" "}
              {t("settings.coach.vsPrev")}
            </span>
          )}
        </Card>
        <Card
          label={t("settings.coach.sessions")}
          value={`${summary.session_count}`}
        >
          <span className="text-text/50">{t("settings.coach.thisWeek")}</span>
        </Card>
      </div>

      {/* Trend */}
      <div className="rounded-xl border border-surface-raised p-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-semibold">
            {t("settings.coach.trend")}
          </span>
          <div className="flex gap-1 text-xs">
            {(["Days7", "Days30", "All"] as TrendWindow[]).map((w) => (
              <button
                key={w}
                type="button"
                onClick={() => setWindow(w)}
                className={`px-2 py-1 rounded ${
                  window === w
                    ? "bg-accent text-white"
                    : "text-secondary hover:text-primary"
                }`}
              >
                {t(
                  w === "Days7"
                    ? "settings.coach.windowDays7"
                    : w === "Days30"
                      ? "settings.coach.windowDays30"
                      : "settings.coach.windowAll",
                )}
              </button>
            ))}
          </div>
        </div>
        <div className="text-xs text-text/50">{t("settings.coach.wpm")}</div>
        <Sparkline
          values={trend.map((p) => p.avg_wpm)}
          color="var(--color-accent)"
        />
        <div className="text-xs text-text/50 mt-2">
          {t("settings.coach.fillerRate")}
        </div>
        <Sparkline
          values={trend.map((p) => p.avg_filler_rate)}
          color="var(--color-ok)"
        />
      </div>

      {/* Streak */}
      <div className="rounded-xl border border-surface-raised p-4 text-sm">
        🔥 <b>{t("settings.coach.streakDays", { count: current_streak })}</b>
        <span className="text-text/50 ml-2">
          {t("settings.coach.streakBest", { count: best_streak })}
        </span>
      </div>
    </div>
  );
};
