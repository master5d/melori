import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type CouncilState,
  councilProgress,
  formatDuration,
} from "./councilView";

// status marks: pictograms, not UI copy
const RUNNING_MARK = "●";

export interface LensOption {
  id: string;
  name: string;
  paradigm: string;
}

interface Props {
  lenses: LensOption[];
  selected: ReadonlySet<string>;
  onToggle: (id: string) => void;
  onToggleAll: () => void;
  council: CouncilState;
  consulting: boolean;
}

/** Lens picker (before the council) and stage telemetry (while it runs). */
export default function CouncilControls({
  lenses,
  selected,
  onToggle,
  onToggleAll,
  council,
  consulting,
}: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!consulting) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [consulting]);

  const progress = councilProgress(council, now);
  const showTelemetry = consulting || progress.stage !== "idle";
  const allOn = lenses.length > 0 && lenses.every((l) => selected.has(l.id));

  return (
    <div className="mc-council-controls">
      {lenses.length > 0 && !consulting && (
        <div className="mc-lenses">
          <button
            type="button"
            className="mc-lenses-toggle"
            aria-expanded={open}
            onClick={() => setOpen((v) => !v)}
          >
            {t("meetingCopilot.council.lenses", {
              selected: selected.size,
              total: lenses.length,
            })}{" "}
            {open ? "▾" : "▸"}
          </button>
          {open && (
            <div className="mc-lenses-list">
              <label className="mc-check">
                <input type="checkbox" checked={allOn} onChange={onToggleAll} />
                <span>{t("meetingCopilot.council.allLenses")}</span>
              </label>
              {lenses.map((lens) => (
                <label key={lens.id} className="mc-check" title={lens.paradigm}>
                  <input
                    type="checkbox"
                    checked={selected.has(lens.id)}
                    onChange={() => onToggle(lens.id)}
                  />
                  <span>{lens.name}</span>
                </label>
              ))}
            </div>
          )}
        </div>
      )}

      {showTelemetry && progress.total > 0 && (
        <div className="mc-council-progress" role="status" aria-live="polite">
          <div className="mc-council-line">
            {progress.stage === "synthesis"
              ? t("meetingCopilot.council.stageSynthesis", {
                  done: progress.done,
                  total: progress.total,
                })
              : progress.stage === "done"
                ? t("meetingCopilot.council.stageDone", {
                    total: progress.total,
                  })
                : progress.stage === "error"
                  ? t("meetingCopilot.council.stageError")
                  : t("meetingCopilot.council.stageLenses", {
                      done: progress.done,
                      total: progress.total,
                    })}
            {progress.running.length > 0 &&
              ` · ${t("meetingCopilot.council.running", {
                names: progress.running.join(", "),
              })}`}
            <span className="mc-council-clock" dir="ltr">
              {" · "}
              {formatDuration(progress.elapsedMs)}
            </span>
            {council.concurrency !== null && (
              <span className="mc-council-meta">
                {" · "}
                {t("meetingCopilot.council.parallel", {
                  n: council.concurrency,
                })}
              </span>
            )}
          </div>
          <ul className="mc-council-lenses">
            {council.lenses.map((lens) => {
              const state =
                lens.elapsedS !== null
                  ? "done"
                  : lens.startedAt !== null
                    ? "running"
                    : "waiting";
              return (
                <li key={lens.id} className={`mc-lens mc-lens-${state}`}>
                  <span aria-hidden="true">
                    {state === "done" ? "✓" : state === "running" ? "●" : "○"}
                  </span>
                  <span className="mc-lens-name">{lens.name}</span>
                  <span className="mc-lens-time" dir="ltr">
                    {state === "done"
                      ? `${Math.round(lens.elapsedS ?? 0)} s`
                      : state === "running" && lens.startedAt !== null
                        ? formatDuration(now - lens.startedAt)
                        : ""}
                  </span>
                </li>
              );
            })}
            {progress.stage === "synthesis" && (
              <li className="mc-lens mc-lens-running">
                <span aria-hidden="true">{RUNNING_MARK}</span>
                <span className="mc-lens-name">
                  {t("meetingCopilot.council.synthesis")}
                </span>
                <span className="mc-lens-time" dir="ltr">
                  {council.synthesisStartedAt !== null
                    ? formatDuration(now - council.synthesisStartedAt)
                    : ""}
                </span>
              </li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
