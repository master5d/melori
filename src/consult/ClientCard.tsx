import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events } from "@/bindings";
import {
  consultApi,
  type ClientDetail,
  type EngineClient,
  type NoteTemplate,
} from "./api";
import { canStartSession } from "./sessionStart";
import {
  allSelected,
  pruneSelection,
  toggleAll,
  toggleOne,
} from "./sessionSelection";
import { shouldStopOnRevoke } from "./meetingClient";
import { RecapBlock } from "./memory/RecapBlock";
import { SearchTab } from "./memory/SearchTab";
import { ChatTab } from "./memory/ChatTab";

interface Props {
  client: EngineClient;
  onChanged: () => void;
  onOpenSession: (
    sessionId: string,
    tab?: "transcript" | "notes" | "email",
  ) => void;
}

export function ClientCard({ client, onChanged, onOpenSession }: Props) {
  const { t, i18n } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [confirmAlias, setConfirmAlias] = useState("");
  const [includeAnalysis, setIncludeAnalysis] = useState(false);
  const [detail, setDetail] = useState<ClientDetail | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  // deleting is irreversible: the first press arms, the second deletes
  const [armed, setArmed] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [templates, setTemplates] = useState<NoteTemplate[]>([]);
  const [templateId, setTemplateId] = useState(client.template_id ?? "soap");
  const [tab, setTab] = useState<"sessions" | "search" | "chat">("sessions");
  const reload = () => setReloadKey((k) => k + 1);
  useEffect(() => {
    void consultApi
      .getClient(client.id)
      .then(setDetail)
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error ? cause.message : t("consult.errors.request"),
        ),
      );
  }, [client.id, t, reloadKey]);
  useEffect(() => {
    void consultApi
      .templates(i18n.language)
      .then(setTemplates)
      .catch(() => setTemplates([]));
  }, []);
  // a session started, stopped or saved in the meeting panel shows up here without
  // leaving the card: reload on meeting start/stop and when the window regains focus
  useEffect(() => {
    const un = events.meetingStateEvent.listen(() => reload());
    window.addEventListener("focus", reload);
    return () => {
      un.then((f) => f());
      window.removeEventListener("focus", reload);
    };
  }, []);
  const sessionIds = (detail?.sessions ?? []).map((session) => session.id);
  useEffect(() => {
    setSelected((current) => pruneSelection(sessionIds, current));
  }, [detail]);
  const deleteSessions = async (ids: string[], armKey: string) => {
    if (armed !== armKey) {
      setArmed(armKey);
      return;
    }
    setArmed(null);
    try {
      for (const id of ids) await consultApi.deleteSession(client.id, id);
      setSelected(new Set());
      reload();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
      reload();
    }
  };
  const revoke = async () => {
    try {
      const binding = await commands.clientBinding();
      await consultApi.revokeClient(client.id);
      if (shouldStopOnRevoke(binding, client.id)) await commands.stopMeeting();
      onChanged();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  // start a meeting bound to this client and reveal the panel; until the
  // "this records everyone" notice is acknowledged the panel shows it first
  const startSession = async () => {
    try {
      const settings = await commands.getAppSettings();
      const acked =
        settings.status === "ok" && !!settings.data.meeting_consent_acked;
      if (acked) {
        const res = await commands.startClientMeeting(client.id);
        if (res.status === "error") {
          setError(res.error);
          return;
        }
      }
      await commands.showMeetingCopilot();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const remove = async () => {
    if (confirmAlias !== client.alias) return;
    try {
      await consultApi.deleteClient(client.id);
      onChanged();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  // WebView2 ignores <a download> on blobs: save through the native dialog
  const saveAs = async (name: string, text: string) => {
    const res = await commands.saveTextFileDialog(name, text);
    if (res.status === "error") throw new Error(res.error);
  };
  const downloadConsent = async () => {
    try {
      const result = await consultApi.consentTemplate(client.id);
      await saveAs(`${client.alias}-consent.txt`, result.text);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const exportSession = async (sessionId: string) => {
    try {
      const result = await consultApi.exportSession(
        client.id,
        sessionId,
        includeAnalysis,
      );
      await saveAs(result.filename, result.markdown);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const attachSigned = async (file: File) => {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      let binary = "";
      bytes.forEach((byte) => {
        binary += String.fromCharCode(byte);
      });
      await consultApi.signedConsent(client.id, file.name, btoa(binary));
      onChanged();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const revoked = !client.consent.active;
  return (
    <article className="grid w-full grid-cols-1 gap-8 xl:grid-cols-12">
      <section className="flex min-w-0 flex-col gap-[22px] xl:col-span-8">
        <header className="flex items-baseline justify-between gap-5 border-b-2 border-rule-strong pb-3.5">
          <h1
            dir="auto"
            className={`min-w-0 break-words font-serif text-5xl font-semibold ${revoked ? "text-secondary line-through" : "text-primary"}`}
          >
            {client.alias}
          </h1>
          <div className="flex shrink-0 items-baseline gap-5">
            <span className="font-mono text-xs text-secondary">
              {t("consult.card.sessionsCount", {
                count: detail?.sessions.length ?? 0,
              })}
            </span>
            <button
              type="button"
              disabled={!canStartSession(client)}
              title={
                canStartSession(client)
                  ? undefined
                  : t("consult.card.newSessionUnavailable")
              }
              onClick={() => void startSession()}
              className="h-11 bg-accent px-5 text-sm font-semibold text-on-accent hover:bg-accent-hover disabled:opacity-50"
            >
              {t("consult.card.newSession")}
            </button>
          </div>
        </header>
        {revoked && (
          <p className="font-serif text-[17px] leading-relaxed text-primary">
            {t("consult.journal.readOnly")}
          </p>
        )}
        <RecapBlock client={client} />
        <nav
          className="flex gap-6 border-b border-rule"
          aria-label={t("consult.memory.tabsLabel")}
        >
          {(["sessions", "search", "chat"] as const).map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setTab(item)}
              className={`border-b-2 px-1 py-3 text-sm ${tab === item ? "border-edge-strong text-primary" : "border-transparent text-secondary"}`}
            >
              {t(`consult.memory.tabs.${item}`)}
            </button>
          ))}
        </nav>
        {tab === "search" && detail && (
          <SearchTab client={client} onOpenSession={onOpenSession} />
        )}
        {tab === "chat" && detail && (
          <ChatTab
            client={client}
            sessions={detail.sessions}
            onOpenSession={onOpenSession}
          />
        )}
        {tab !== "sessions" && !detail && (
          <p className="text-sm text-secondary">
            {t("consult.journal.loading")}
          </p>
        )}
        {tab === "sessions" && (
          <>
            {sessionIds.length > 0 && (
              <div
                role="toolbar"
                aria-label={t("consult.card.sessionsToolbar")}
                className="flex items-center gap-5 border-b border-rule pb-3 text-sm"
              >
                <label className="flex cursor-pointer items-center gap-2 text-primary">
                  <input
                    type="checkbox"
                    className="accent-accent"
                    checked={allSelected(sessionIds, selected)}
                    onChange={() =>
                      setSelected(toggleAll(sessionIds, selected))
                    }
                  />
                  {t("consult.card.selectAll")}
                </label>
                <span className="flex-1" />
                <button
                  type="button"
                  disabled={selected.size === 0}
                  onClick={() => void deleteSessions([...selected], "batch")}
                  className="h-10 border border-err px-4 text-err disabled:opacity-40"
                >
                  {armed === "batch"
                    ? t("consult.card.deleteConfirm", { count: selected.size })
                    : t("consult.card.deleteSelected", {
                        count: selected.size,
                      })}
                </button>
              </div>
            )}
            <div className="border-t border-rule">
              {(detail?.sessions ?? []).map((session) => (
                <article
                  key={session.id}
                  className="grid grid-cols-1 gap-5 border-b border-rule py-5 md:grid-cols-8 md:gap-6"
                  onClick={() => onOpenSession(session.id)}
                >
                  <div className="flex flex-col gap-1 md:col-span-2">
                    <label className="flex cursor-pointer items-center gap-2">
                      <input
                        type="checkbox"
                        className="accent-accent"
                        aria-label={t("consult.card.selectSession", {
                          date: session.date,
                        })}
                        checked={selected.has(session.id)}
                        onChange={() =>
                          setSelected(toggleOne(selected, session.id))
                        }
                        onClick={(event) => event.stopPropagation()}
                      />
                      <button
                        type="button"
                        className="font-mono text-[13px] text-accent underline"
                        dir="ltr"
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenSession(session.id);
                        }}
                      >
                        {session.date}
                      </button>
                    </label>
                    <span className="font-serif text-lg font-semibold text-primary">
                      {t(
                        `consult.session.meetingTypes.${session.meeting_type}`,
                        {
                          defaultValue: session.meeting_type,
                        },
                      )}
                    </span>
                    {session.status === "unmigrated" && (
                      <span className="text-xs text-secondary">
                        {t("consult.card.unmigrated")}
                      </span>
                    )}
                  </div>
                  <p className="m-0 font-serif text-base leading-relaxed text-primary md:col-span-4">
                    {session.note || t("consult.journal.noNote")}
                  </p>
                  <aside className="flex flex-col gap-2 border-s border-rule ps-3 text-sm md:col-span-2">
                    <button
                      type="button"
                      className="text-start text-accent underline"
                      onClick={(event) => {
                        event.stopPropagation();
                        void exportSession(session.id);
                      }}
                    >
                      {t("consult.actions.export")}
                    </button>
                    <button
                      type="button"
                      className="text-start text-err underline"
                      onClick={(event) => {
                        event.stopPropagation();
                        void deleteSessions([session.id], session.id);
                      }}
                    >
                      {armed === session.id
                        ? t("consult.card.deleteOneConfirm")
                        : t("consult.card.deleteOne")}
                    </button>
                  </aside>
                </article>
              ))}
            </div>
          </>
        )}
      </section>
      <aside
        aria-label={t("consult.journal.consent")}
        className="flex h-fit flex-col gap-4 border border-rule bg-surface p-6 xl:col-span-4"
      >
        <h2 className="font-serif text-[22px] font-semibold text-primary">
          {t("consult.journal.consent")}
        </h2>
        <ul className="m-0 flex list-none flex-col gap-2 p-0 text-[15px]">
          {(["transcript", "video", "council", "retain"] as const).map(
            (permission) => (
              <li
                key={permission}
                className={
                  !client.consent.permissions.includes(permission)
                    ? "text-secondary"
                    : "text-primary"
                }
              >
                {client.consent.permissions.includes(permission) ? "✓" : "—"}{" "}
                {t(`consult.permissions.${permission}`)}
              </li>
            ),
          )}
        </ul>
        <div className="font-mono text-xs leading-relaxed text-secondary">
          {client.consent.date || t("consult.form.date")}
        </div>
        <label className="flex flex-col gap-1.5 text-sm text-secondary">
          {t("consult.card.template")}
          <select
            className="h-11 border-b border-rule-strong bg-transparent text-primary"
            value={templateId}
            onChange={(event) => {
              // stays on the card: onChanged closes it (meant for revoke/delete)
              const previous = templateId;
              setTemplateId(event.target.value);
              void consultApi
                .setClientTemplate(client.id, event.target.value)
                .catch((cause: unknown) => {
                  setTemplateId(previous);
                  setError(
                    cause instanceof Error
                      ? cause.message
                      : t("consult.errors.request"),
                  );
                });
            }}
          >
            {templates
              .filter((template) => template.id !== "free")
              .map((template) => (
                <option key={template.id} value={template.id}>
                  {template.name}
                </option>
              ))}
          </select>
        </label>
        <div className="flex flex-col gap-2.5 border-t border-rule pt-4">
          <button
            type="button"
            className="h-11 border border-rule-strong bg-transparent text-primary"
            onClick={() => void downloadConsent()}
          >
            {t("consult.actions.downloadConsent")}
          </button>
          <label className="flex h-11 cursor-pointer items-center justify-center border border-rule-strong bg-transparent text-primary focus-within:outline focus-within:outline-2 focus-within:outline-accent">
            {t("consult.actions.attachSigned")}
            <input
              type="file"
              className="sr-only"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) void attachSigned(file);
              }}
            />
          </label>
          <button
            type="button"
            disabled={revoked}
            className="h-11 border border-err bg-transparent text-err disabled:opacity-50"
            onClick={() => void revoke()}
          >
            {t("consult.actions.revoke")}
          </button>
        </div>
      </aside>
      {revoked && (
        <aside className="border border-err p-6 xl:col-start-9 xl:col-span-4">
          <h2 className="font-serif text-xl font-semibold text-primary">
            {t("consult.journal.deleteTitle")}
          </h2>
          <p className="mt-3 text-sm leading-relaxed text-secondary">
            {t("consult.journal.deleteDescription")}
          </p>
          <label className="mt-4 flex flex-col gap-1.5 text-sm text-secondary">
            {t("consult.form.alias")}
            <input
              className="h-11 border border-rule-strong bg-surface px-3 text-primary"
              placeholder={client.alias}
              value={confirmAlias}
              onChange={(event) => setConfirmAlias(event.target.value)}
            />
          </label>
          <button
            type="button"
            disabled={confirmAlias !== client.alias}
            className="mt-4 h-11 w-full bg-err text-on-accent disabled:opacity-50"
            onClick={() => void remove()}
          >
            {t("consult.journal.deletePermanent")}
          </button>
        </aside>
      )}
      {error && (
        <p role="alert" className="text-sm text-err xl:col-span-8">
          {error}
        </p>
      )}
    </article>
  );
}
