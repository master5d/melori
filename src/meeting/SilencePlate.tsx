import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { countdownState } from "./meetingTools";

export function SilencePlate({
  startedAt,
  lastVoiceAt,
  onContinue,
  onExpire,
}: {
  startedAt: number;
  lastVoiceAt: number | null;
  onContinue: () => void;
  onExpire: () => void;
}) {
  const { t } = useTranslation();
  const [now, setNow] = useState(Date.now());
  const expiredRef = useRef(false);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, []);
  const state = countdownState(startedAt, now, lastVoiceAt);
  useEffect(() => {
    if (state.expired && !expiredRef.current) {
      expiredRef.current = true;
      onExpire();
    }
  }, [state.expired, onExpire]);
  if (state.cancelled || state.expired) return null;
  return (
    <aside className="mc-silence-plate" role="alert">
      <strong>{t("meetingCopilot.silence.title")}</strong>
      <span dir="ltr">
        {t("meetingCopilot.silence.seconds", {
          count: Math.ceil(state.remaining / 1000),
        })}
      </span>
      <button className="mc-btn mc-btn-primary" onClick={onContinue}>
        {t("meetingCopilot.silence.continue")}
      </button>
    </aside>
  );
}
