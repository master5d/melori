import { useTranslation } from "react-i18next";
import type { SessionDetail } from "../api";
import { askChipLine } from "@/meeting/meetingTools";
import { formatClock } from "./sessionLogic";

export function AsksTab({ detail }: { detail: SessionDetail }) {
  const { t } = useTranslation();
  return detail.asks.length === 0 ? (
    <p className="py-8 font-serif text-lg text-secondary">
      {t("consult.session.emptyAsks")}
    </p>
  ) : (
    <div className="space-y-3">
      {detail.asks.map((ask, index) => (
        <div
          key={index}
          className="border-b border-rule py-3 font-serif text-primary"
        >
          <div className="flex gap-3">
            <time className="font-mono text-xs text-secondary" dir="ltr">
              {formatClock(ask.elapsed_ms)}
            </time>
            <div>
              <div className="text-xs text-secondary">{ask.kind}</div>
              <h3 className="m-0 text-lg">{ask.question}</h3>
              <p className="whitespace-pre-wrap">{ask.answer}</p>
              <small>{askChipLine(ask.provenance, t)}</small>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
