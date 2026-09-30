import { useTranslation } from "react-i18next";

export function VoiceLights({
  speaking,
}: {
  speaking: { me: boolean; others: boolean };
}) {
  const { t } = useTranslation();
  return (
    <div
      className="mc-voice-lights"
      aria-label={t("meetingCopilot.voiceActivity")}
    >
      <span className={speaking.me ? "active" : ""}>
        {t("meetingCopilot.me")}
      </span>
      <span className={speaking.others ? "active" : ""}>
        {t("meetingCopilot.others")}
      </span>
    </div>
  );
}
