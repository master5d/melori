import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { EngineClient, SessionDetail } from "../api";
import { consultApi } from "../api";
import { NotesTab } from "./NotesTab";
import { TranscriptTab } from "./TranscriptTab";
import { AsksTab } from "./AsksTab";
import { EmailTab } from "./EmailTab";
import { formatClock, saveBlockedReason } from "./sessionLogic";

type Tab = "notes" | "transcript" | "asks" | "email";
export type SessionTab = Tab;
export function SessionPage({
  client,
  sessionId,
  onBack,
  initialTab = "notes",
}: {
  client: EngineClient;
  sessionId: string;
  onBack: () => void;
  initialTab?: SessionTab;
}) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [tab, setTab] = useState<Tab>(initialTab);
  const [error, setError] = useState<string | null>(null);
  const refresh = async () => {
    try {
      setDetail(await consultApi.sessionDetail(client.id, sessionId));
      setError(null);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  useEffect(() => {
    void refresh();
  }, [client.id, sessionId]);
  if (!detail)
    return (
      <p className="py-12 font-serif text-xl text-secondary">
        {error ?? t("consult.journal.loading")}
      </p>
    );
  const readOnly = !!client.consent.revoked;
  const saveBlocked =
    saveBlockedReason(client.consent.permissions, readOnly) !== null;
  const session = detail.session;
  const duration =
    session.duration_ms == null ? null : formatClock(session.duration_ms);
  return (
    <section className="w-full">
      <button
        type="button"
        className="mb-6 font-mono text-sm text-accent underline"
        onClick={onBack}
      >
        {t("consult.session.back", { alias: client.alias })}
      </button>
      <header className="border-b-2 border-rule-strong pb-4">
        <div
          className="flex flex-wrap items-baseline gap-x-5 gap-y-2 font-mono text-xs text-secondary"
          dir="ltr"
        >
          <span>{session.date}</span>
          <span>
            {t(`consult.session.meetingTypes.${session.meeting_type}`, {
              defaultValue: session.meeting_type,
            })}
          </span>
          {duration && <span>{duration}</span>}
          <span>
            {t(`consult.session.status.${session.status}`, session.status)}
          </span>
        </div>
        <h1 className="mt-2 font-serif text-4xl font-semibold text-primary">
          {client.alias}
        </h1>
      </header>
      {readOnly && (
        <p className="mt-4 border border-err p-3 text-sm text-err">
          {t("consult.session.revokedReadOnly")}
        </p>
      )}
      {error && (
        <p
          role="alert"
          className="mt-4 border border-err bg-err-surface p-3 text-sm text-err"
        >
          {error}
        </p>
      )}
      <nav
        className="mt-6 flex gap-6 border-b border-rule"
        aria-label={t("consult.session.tabsLabel")}
      >
        {(["notes", "transcript", "asks", "email"] as const).map((item) => (
          <button
            key={item}
            type="button"
            onClick={() => setTab(item)}
            className={`border-b-2 px-1 py-3 text-sm ${tab === item ? "border-edge-strong text-primary" : "border-transparent text-secondary"}`}
          >
            {t(`consult.session.tabs.${item}`)}
          </button>
        ))}
      </nav>
      <main className="pt-7">
        {tab === "notes" && (
          <NotesTab
            clientId={client.id}
            sessionId={sessionId}
            detail={detail}
            readOnly={readOnly}
            saveBlocked={saveBlocked}
            onRefresh={refresh}
            onError={setError}
          />
        )}
        {tab === "transcript" && <TranscriptTab detail={detail} />}
        {tab === "asks" && <AsksTab detail={detail} />}
        {tab === "email" && (
          <EmailTab
            client={client}
            clientId={client.id}
            sessionId={sessionId}
            detail={detail}
            onRefresh={refresh}
            onError={setError}
          />
        )}
      </main>
    </section>
  );
}
