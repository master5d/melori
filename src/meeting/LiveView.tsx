import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { formatElapsed, type Row } from "./copilotLogic";

interface LiveViewProps {
  isRecording: boolean;
  elapsedMs: number;
  modeKey: "both" | "micOnly";
  rows: Row[];
  onStop: () => void;
  onClose: () => void;
}

/** Recording hero: drag-region header + the live transcript flow.
 *  Presentational — all session state lives in MeetingCopilot. */
export default function LiveView({
  isRecording,
  elapsedMs,
  modeKey,
  rows,
  onStop,
  onClose,
}: LiveViewProps) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [rows]);

  return (
    <div className="mc-root">
      <header className="mc-header" data-tauri-drag-region>
        <span className={`mc-dot ${isRecording ? "rec" : "idle"}`} />
        <span className="mc-rec-label">
          {isRecording
            ? t("meetingCopilot.recording")
            : t("meetingCopilot.idle")}
        </span>
        {isRecording && (
          <span className="mc-elapsed">{formatElapsed(elapsedMs)}</span>
        )}
        <span className="mc-mode">{t(`meetingCopilot.${modeKey}`)}</span>
        <span className="mc-spacer" />
        {isRecording ? (
          <button className="mc-btn mc-btn-stop" onClick={onStop}>
            {t("meetingCopilot.stop")}
          </button>
        ) : (
          <button className="mc-btn mc-btn-ghost" onClick={onClose}>
            {t("meetingCopilot.close")}
          </button>
        )}
      </header>

      <div className="mc-transcript" ref={scrollRef}>
        {rows.length === 0 ? (
          <div className="mc-waiting">{t("meetingCopilot.waiting")}</div>
        ) : (
          rows.map((r) => (
            <div key={r.id} className={`mc-row mc-${r.source}`}>
              <span className="mc-gutter">
                {t(`meetingCopilot.${r.source}`)}
              </span>
              <span className="mc-text">{r.text}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
