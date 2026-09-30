import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { consultApi, type EngineClient, type RecapResponse } from "../api";
import { daysAgoLine, withGeneratedRecap } from "./memoryLogic";

export function RecapBlock({ client }: { client: EngineClient }) {
  const { t } = useTranslation();
  const [data, setData] = useState<RecapResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const canModel = client.consent.permissions.includes("council");
  const load = () => {
    void consultApi
      .recap(client.id)
      .then(setData)
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error ? cause.message : t("consult.errors.request"),
        ),
      );
  };
  useEffect(load, [client.id]);
  const generate = async () => {
    setLoading(true);
    setError(null);
    try {
      const generated = await consultApi.generateRecap(client.id);
      setData((current) => withGeneratedRecap(current, generated));
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setLoading(false);
    }
  };
  const quick = data?.quick;
  return (
    <section
      className="border border-rule bg-surface p-5"
      aria-label={t("consult.memory.recapTitle")}
    >
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h2 className="font-serif text-2xl font-semibold text-primary">
          {t("consult.memory.recapTitle")}
        </h2>
        {canModel && (
          <button
            type="button"
            className="text-sm text-accent underline"
            disabled={loading}
            onClick={() => void generate()}
          >
            {loading
              ? t("consult.memory.generating")
              : t("consult.memory.generate")}
          </button>
        )}
      </div>
      {!quick && !data?.model && (
        <p className="mt-3 text-sm text-secondary">
          {t("consult.memory.empty")}
        </p>
      )}
      {quick && (
        <div className="mt-4 space-y-2 text-sm text-primary">
          <div className="font-mono text-xs text-secondary">
            {quick.date} · {daysAgoLine(quick.days_ago, t)}
          </div>
          {quick.plan && (
            <p>
              <span className="font-semibold">{quick.plan.title}:</span>{" "}
              {quick.plan.text}
            </p>
          )}
          {quick.email_subject && (
            <p>
              <span className="font-semibold">
                {t("consult.memory.email")}:
              </span>{" "}
              {quick.email_subject}
            </p>
          )}
        </div>
      )}
      {data?.model && (
        <div className="mt-4 border-t border-rule pt-3">
          <div className="mb-2 text-xs text-secondary">
            {data.model.created}
            {data.model_stale ? ` · ${t("consult.memory.stale")}` : ""}
          </div>
          <ul className="m-0 list-disc space-y-1 ps-5 text-sm text-primary">
            {data.model.points.map((point) => (
              <li key={point}>{point}</li>
            ))}
          </ul>
        </div>
      )}
      {error && (
        <p role="alert" className="mt-3 text-sm text-err">
          {error}
        </p>
      )}
    </section>
  );
}
