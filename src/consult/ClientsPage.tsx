import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import {
  consultApi,
  type ClientDetail,
  type EngineClient,
  type Health,
  type PurgeReport,
} from "./api";
import { ClientCard } from "./ClientCard";
import { NewClientForm } from "./NewClientForm";
import { SessionPage, type SessionTab } from "./session/SessionPage";
import {
  clientsPageState,
  normalizeEngineState,
  type DiskEncryption,
  type EngineState,
} from "./clientsPageState";

function normalizeDisk(value: string | null | undefined): DiskEncryption {
  return value === "on" || value === "off" || value === "unknown"
    ? value
    : null;
}

const BANNER_TEXT: Record<
  ReturnType<typeof clientsPageState>["banners"][number],
  { title: string; message: string }
> = {
  "engine-down": {
    title: "consult.journal.engineDown",
    message: "consult.journal.engineDownMessage",
  },
  "disk-off": {
    title: "consult.journal.diskOff",
    message: "consult.journal.diskOffMessage",
  },
  "disk-unknown": {
    title: "consult.journal.diskUnknown",
    message: "consult.journal.diskUnknownMessage",
  },
  purged: {
    title: "consult.journal.purged",
    message: "consult.journal.purgedMessage",
  },
};

export function ClientsPage() {
  const { t } = useTranslation();
  const [clients, setClients] = useState<EngineClient[] | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [purgeReport, setPurgeReport] = useState<PurgeReport | null>(null);
  const [diskEncryption, setDiskEncryption] = useState<DiskEncryption>(null);
  const [engine, setEngine] = useState<EngineState>("starting");
  const [selected, setSelected] = useState<EngineClient | null>(null);
  const [selectedSession, setSelectedSession] = useState<{
    id: string;
    tab: SessionTab;
  } | null>(null);
  const [newClient, setNewClient] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<string, ClientDetail | null>>(
    {},
  );
  const load = async () => {
    let status: { state: string; disk_encryption: string | null } | null = null;
    try {
      status = await commands.engineStatus();
      setEngine(normalizeEngineState(status.state));
      setDiskEncryption(normalizeDisk(status.disk_encryption));
    } catch {
      setEngine("down");
    }
    try {
      const items = await consultApi.listClients();
      setClients(items);
      const loaded = await Promise.all(
        items.map(
          async (client) =>
            [
              client.id,
              await consultApi.getClient(client.id).catch(() => null),
            ] as const,
        ),
      );
      setDetails(Object.fromEntries(loaded));
      setError(null);
    } catch (cause) {
      setEngine("down");
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
    const [healthReport, expired] = await Promise.all([
      consultApi.health().catch(() => null),
      consultApi.purgeReport().catch(() => null),
    ]);
    setHealth(healthReport);
    setPurgeReport(expired);
    if (healthReport)
      setDiskEncryption(normalizeDisk(healthReport.disk_encryption));
    if (status?.state) setEngine(normalizeEngineState(status.state));
  };
  useEffect(() => {
    void load();
  }, []);
  const state = clientsPageState({
    engine,
    diskEncryption,
    clients,
    purged: purgeReport?.deleted ?? [],
  });
  if (newClient)
    return (
      <NewClientForm
        onCreated={() => {
          setNewClient(false);
          void load();
        }}
        onCancel={() => setNewClient(false)}
      />
    );
  if (selected)
    return (
      <div className="w-full">
        {!selectedSession && (
          <button
            type="button"
            className="mb-7 font-mono text-sm text-accent underline"
            onClick={() => {
              setSelected(null);
              void load();
            }}
          >
            {t("consult.actions.back")}
          </button>
        )}
        {selectedSession ? (
          <SessionPage
            client={selected}
            sessionId={selectedSession.id}
            initialTab={selectedSession.tab}
            onBack={() => setSelectedSession(null)}
          />
        ) : (
          <ClientCard
            client={selected}
            onOpenSession={(id, tab = "notes") =>
              setSelectedSession({ id, tab })
            }
            onChanged={() => {
              setSelected(null);
              void load();
            }}
          />
        )}
      </div>
    );
  const createHint = state.canCreate
    ? undefined
    : t("consult.journal.createDisabled");
  return (
    <section className="w-full">
      {state.banners.map((banner) => (
        <div
          key={banner}
          role="alert"
          className={`flex items-center gap-4 border px-[18px] py-3.5 text-sm ${banner === "engine-down" ? "border-err bg-err-surface" : banner === "purged" ? "border-rule bg-surface" : "border-warn-edge bg-warn-surface"}`}
        >
          <span
            className={`font-mono text-xs ${banner === "engine-down" ? "text-err" : "text-warn"}`}
          >
            {t(BANNER_TEXT[banner].title)}
          </span>
          <span className="text-primary">
            {t(BANNER_TEXT[banner].message, {
              count: purgeReport?.deleted.length ?? 0,
            })}
          </span>
        </div>
      ))}
      <header className="mt-7 flex items-end justify-between gap-6 border-b-2 border-rule-strong pb-[18px]">
        <div>
          <div className="font-mono text-xs uppercase tracking-[0.08em] text-secondary">
            {t("consult.journal.volume")}
          </div>
          <h1 className="mt-1 font-serif text-[52px] font-semibold leading-none text-primary">
            {t("consult.clients.title")}
          </h1>
        </div>
        <button
          type="button"
          disabled={!state.canCreate}
          title={createHint}
          aria-describedby={!state.canCreate ? "new-client-hint" : undefined}
          className="h-11 border-0 bg-accent px-5 text-sm font-medium text-on-accent disabled:cursor-not-allowed disabled:opacity-50"
          onClick={() => setNewClient(true)}
        >
          {t("consult.actions.newClient")}
        </button>
      </header>
      {!state.canCreate && (
        <p id="new-client-hint" className="sr-only">
          {createHint}
        </p>
      )}
      <div
        className="flex gap-7 py-2 font-mono text-xs text-secondary"
        role="status"
      >
        <span>{t(`consult.journal.engine.${engine}`)}</span>
        <span>{t(`consult.journal.disk.${diskEncryption ?? "unknown"}`)}</span>
        {health && (
          <span className={health.llm_local ? undefined : "text-warn"}>
            {health.llm_local
              ? t("consult.journal.llm.local")
              : t("consult.journal.llm.remote")}
          </span>
        )}
      </div>
      {error && engine !== "down" && (
        <p
          role="alert"
          className="mt-3 border border-err bg-err-surface p-3 text-sm text-err"
        >
          {error}
        </p>
      )}
      {state.view === "loading" && (
        <p className="py-12 font-serif text-xl text-secondary">
          {t("consult.journal.loading")}
        </p>
      )}
      {state.view === "empty" && (
        <section className="mt-10 flex max-w-[560px] flex-col gap-[18px]">
          <p className="font-serif text-2xl leading-snug text-primary">
            {t("consult.journal.emptyLead")}
          </p>
          <p className="font-serif text-[17px] leading-relaxed text-secondary">
            {t("consult.journal.emptyBody")}
          </p>
        </section>
      )}
      {state.view === "list" && (
        <div className="mt-3">
          <div className="grid grid-cols-12 gap-6 border-b border-rule pb-2 font-mono text-xs uppercase tracking-[0.06em] text-secondary">
            <span className="col-span-4">
              {t("consult.journal.tableClient")}
            </span>
            <span className="col-span-5">
              {t("consult.journal.tableConsent")}
            </span>
            <span className="col-span-2">
              {t("consult.journal.tableLastSession")}
            </span>
            <span className="col-span-1 text-right">
              {t("consult.journal.tableSessions")}
            </span>
          </div>
          {clients?.map((client) => {
            const sessions = details[client.id]?.sessions ?? [];
            const last = sessions[sessions.length - 1];
            return (
              <button
                type="button"
                key={client.id}
                className="group grid w-full cursor-pointer grid-cols-12 items-baseline gap-6 border-b border-rule py-5 text-start text-primary transition-colors hover:bg-surface"
                onClick={() => setSelected(client)}
              >
                <span
                  dir="auto"
                  className={`col-span-4 min-w-0 truncate font-serif text-2xl group-hover:text-accent group-hover:underline group-focus-visible:text-accent underline-offset-4 ${!client.consent.active ? "text-secondary line-through" : ""}`}
                >
                  {client.alias}
                </span>
                <span
                  className={`col-span-5 truncate text-[15px] ${client.consent.active ? "" : "text-err"}`}
                >
                  {client.consent.active
                    ? client.consent.permissions
                        .map((permission) =>
                          t(`consult.permissions.${permission}`),
                        )
                        .join(" · ")
                    : t("consult.journal.revoked")}
                </span>
                <span className="col-span-2 font-mono text-sm" dir="ltr">
                  {last?.date || "—"}
                </span>
                <span className="col-span-1 text-right font-mono text-sm">
                  {sessions.length}
                </span>
              </button>
            );
          })}
        </div>
      )}
      <p
        dir="auto"
        className="mt-auto max-w-[620px] pt-16 font-serif text-[15px] italic leading-relaxed text-secondary"
      >
        {t("consult.journal.privacyNote")}
      </p>
    </section>
  );
}
