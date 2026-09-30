import { useTranslation } from "react-i18next";
import type { SessionDetail } from "../api";
import { formatClock, markLabel, speakerKey } from "./sessionLogic";

export function TranscriptTab({ detail }: { detail: SessionDetail }) {
  const { t } = useTranslation();
  return (
    <div className="divide-y divide-rule border-t border-rule">
      {detail.transcript.length === 0 ? (
        <p className="py-8 font-serif text-lg text-secondary">
          {t("consult.session.emptyTranscript")}
        </p>
      ) : (
        detail.transcript.map((line) => (
          <article
            key={line.i}
            className="grid grid-cols-[72px_90px_1fr] gap-4 py-4"
          >
            <time className="font-mono text-xs text-secondary" dir="ltr">
              {line.source === "mark"
                ? markLabel(line)
                : formatClock(line.start_ms)}
            </time>
            <span className="text-sm text-secondary">
              {line.source === "mark"
                ? ""
                : t(`consult.session.${speakerKey(line.source)}`)}
            </span>
            <p className="m-0 font-serif text-base text-primary">{line.text}</p>
          </article>
        ))
      )}
    </div>
  );
}
