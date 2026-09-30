import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { consultApi, type RecapResponse } from "@/consult/api";
import { daysAgoLine } from "@/consult/memory/memoryLogic";

export function RecapPlate({ clientId }: { clientId: string }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const [data, setData] = useState<RecapResponse | null>(null);
  useEffect(() => {
    void consultApi
      .recap(clientId)
      .then(setData)
      .catch(() => setData(null));
  }, [clientId]);
  if (!data?.quick && !data?.model) return null;
  return (
    <section className="mx-4 mb-3 border border-rule bg-surface p-3 text-sm">
      <button
        type="button"
        className="font-semibold text-primary"
        onClick={() => setOpen((value) => !value)}
      >
        {open ? "▾" : "▸"} {t("consult.memory.recapTitle")}
      </button>
      {open && (
        <div className="mt-2 space-y-1 text-primary">
          {data.quick && (
            <>
              <div className="font-mono text-xs text-secondary">
                {data.quick.date} · {daysAgoLine(data.quick.days_ago, t)}
              </div>
              {data.quick.plan && (
                <p className="m-0">
                  {data.quick.plan.title}: {data.quick.plan.text}
                </p>
              )}
              {data.quick.email_subject && (
                <p className="m-0">
                  {t("consult.memory.email")}: {data.quick.email_subject}
                </p>
              )}
            </>
          )}
          {data.model?.points.map((point) => (
            <p className="m-0" key={point}>
              • {point}
            </p>
          ))}
        </div>
      )}
    </section>
  );
}
