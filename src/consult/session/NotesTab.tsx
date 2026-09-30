import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SessionDetail, SessionNote } from "../api";
import NoteView from "@/meeting/NoteView";
import { ProvenanceChip } from "./ProvenanceChip";
import {
  editorNoteAfterRefresh,
  parentFor,
  versionLabel,
} from "./sessionLogic";
import { diagnosisWarning } from "@/meeting/meetingTools";

interface Props {
  clientId: string;
  sessionId: string;
  detail: SessionDetail;
  readOnly: boolean;
  saveBlocked: boolean;
  onRefresh: () => Promise<void>;
  onError: (message: string) => void;
}

export function NotesTab({
  clientId,
  sessionId,
  detail,
  readOnly,
  saveBlocked,
  onRefresh,
  onError,
}: Props) {
  const { t, i18n } = useTranslation();
  const note = detail.note;
  const [editing, setEditing] = useState<SessionNote | null>(note);
  const [viewing, setViewing] = useState<SessionNote | null>(null);
  const [base, setBase] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [templates, setTemplates] = useState<{ id: string; name: string }[]>(
    [],
  );
  const [templateId, setTemplateId] = useState(
    note?.template_id ?? detail.session.template_id ?? "soap",
  );
  useEffect(() => {
    if (base === null) {
      setEditing((previous) =>
        editorNoteAfterRefresh(previous, previous?.n ?? null, note, base),
      );
    }
  }, [base, note]);
  useEffect(() => {
    void import("../api")
      .then(({ consultApi }) => consultApi.templates(i18n.language))
      .then((items) => setTemplates(items.filter((item) => item.id !== "free")))
      .catch(() => setTemplates([]));
  }, [i18n.language]);
  useEffect(() => {
    setTemplateId(note?.template_id ?? detail.session.template_id ?? "soap");
  }, [note?.n, detail.session.template_id]);
  const fields = useMemo(() => editing?.fields ?? {}, [editing]);
  const updateField = (key: string, value: string) =>
    setEditing(
      (current) =>
        current && { ...current, fields: { ...current.fields, [key]: value } },
    );
  const save = async () => {
    if (!editing) return;
    setBusy(true);
    try {
      await import("../api").then(({ consultApi }) =>
        consultApi.saveNote(clientId, sessionId, {
          template_id: editing.template_id,
          fields: editing.fields,
          parent: parentFor(base, note?.n ?? null),
        }),
      );
      setBase(null);
      await onRefresh();
    } catch (cause) {
      onError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setBusy(false);
    }
  };
  const generate = async () => {
    setBusy(true);
    try {
      const { consultApi } = await import("../api");
      await consultApi.generateNote(clientId, sessionId, templateId);
      await onRefresh();
    } catch (cause) {
      onError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="grid gap-8 lg:grid-cols-[1fr_220px]">
      <section>
        {viewing ? (
          <div>
            <NoteView note={{ ...viewing, stored: true } as never} />
            <div className="mt-5 flex flex-wrap gap-4">
              <button
                type="button"
                disabled={readOnly || busy}
                onClick={() => {
                  setEditing(viewing);
                  setBase(viewing.n);
                  setViewing(null);
                }}
                className="h-11 bg-accent px-5 text-on-accent disabled:opacity-50"
              >
                {t("consult.session.takeAsBase")}
              </button>
              <button
                type="button"
                onClick={() => setViewing(null)}
                className="h-11 border border-rule-strong px-5 text-primary"
              >
                {t("consult.session.useCurrent")}
              </button>
            </div>
          </div>
        ) : !editing ? (
          <div className="flex flex-col gap-4 py-8">
            <p className="font-serif text-xl text-secondary">
              {t("consult.session.noNote")}
            </p>
            <button
              type="button"
              disabled={readOnly || busy}
              onClick={() => void generate()}
              className="h-11 w-fit bg-accent px-5 text-on-accent disabled:opacity-50"
            >
              {t("consult.session.writeNote")}
            </button>
          </div>
        ) : (
          <>
            <header className="mb-5 flex items-start justify-between gap-4">
              <div>
                <h2 className="font-serif text-3xl text-primary">
                  {t("consult.session.noteTitle", { n: editing.n })}
                </h2>
                {editing.provenance && (
                  <ProvenanceChip
                    provenance={editing.provenance}
                    detail={detail}
                  />
                )}
              </div>
              <div className="flex flex-wrap items-center justify-end gap-3">
                <label className="sr-only" htmlFor="note-template">
                  {t("consult.session.template")}
                </label>
                <select
                  id="note-template"
                  value={templateId}
                  onChange={(event) => setTemplateId(event.target.value)}
                  disabled={readOnly || busy}
                  className="h-10 border-b border-rule-strong bg-transparent text-sm text-primary"
                >
                  {templates.map((template) => (
                    <option key={template.id} value={template.id}>
                      {template.name}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  disabled={readOnly || busy}
                  onClick={() => void generate()}
                  className="h-10 border border-rule-strong px-3 text-sm disabled:opacity-50"
                >
                  {t("consult.session.regenerate")}
                </button>
              </div>
            </header>
            <div className="space-y-5">
              {editing.sections.map(([key, title]) => (
                <label key={key} className="block">
                  <span className="mb-1 block font-serif text-lg text-primary">
                    {title}
                  </span>
                  {(editing.warnings ?? [])
                    .filter((warning) => warning.section === key)
                    .map((warning) => (
                      <span
                        className="mb-1 block text-sm text-accent"
                        key={`${warning.section}-${warning.phrase}`}
                      >
                        {diagnosisWarning(warning.phrase, t)}
                      </span>
                    ))}
                  {readOnly ? (
                    <p className="whitespace-pre-wrap border-b border-rule py-2 font-serif text-base">
                      {fields[key]}
                    </p>
                  ) : (
                    <textarea
                      value={fields[key] ?? ""}
                      onChange={(event) => updateField(key, event.target.value)}
                      className="min-h-24 w-full resize-y border-0 border-b border-rule-strong bg-transparent px-0 py-2 font-serif text-base text-primary outline-none focus:border-accent"
                    />
                  )}
                </label>
              ))}
            </div>
            {!readOnly && (
              <button
                type="button"
                disabled={saveBlocked || busy}
                onClick={() => void save()}
                className="mt-6 h-11 bg-accent px-5 text-on-accent disabled:opacity-50"
              >
                {t("consult.session.saveEdit")}
              </button>
            )}
          </>
        )}
      </section>
      <aside className="border-s border-rule ps-4">
        <h3 className="mb-3 font-mono text-xs uppercase text-secondary">
          {t("consult.session.versions")}
        </h3>
        <div className="space-y-1">
          {detail.versions.map((version) => (
            <button
              key={version.n}
              type="button"
              className="block w-full border-b border-rule py-2 text-start text-sm text-primary hover:text-accent"
              onClick={async () => {
                try {
                  const { consultApi } = await import("../api");
                  setViewing(
                    await consultApi.noteVersion(
                      clientId,
                      sessionId,
                      version.n,
                    ),
                  );
                } catch (cause) {
                  onError(
                    cause instanceof Error
                      ? cause.message
                      : t("consult.errors.request"),
                  );
                }
              }}
            >
              {versionLabel(version, t)}
            </button>
          ))}
        </div>
      </aside>
      {editing && note && editing.n !== note.n && !readOnly && !viewing && (
        <button
          type="button"
          className="text-accent underline"
          onClick={() => {
            setEditing(note);
            setBase(null);
          }}
        >
          {t("consult.session.useCurrent")}
        </button>
      )}
    </div>
  );
}
