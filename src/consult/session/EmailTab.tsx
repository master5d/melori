import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { EngineClient, SessionDetail } from "../api";
import { consultApi } from "../api";
import {
  canDraftEmail,
  emailIsStale,
  mailtoFor,
  openEmailWithClipboard,
  regenerateNeedsConfirm,
  saveBlockedReason,
} from "./sessionLogic";
import { ProvenanceChip } from "./ProvenanceChip";

export function EmailTab({
  client,
  clientId,
  sessionId,
  detail,
  onRefresh,
  onError,
}: {
  client: EngineClient;
  clientId: string;
  sessionId: string;
  detail: SessionDetail;
  onRefresh: () => Promise<void>;
  onError: (message: string) => void;
}) {
  const { t } = useTranslation();
  const [subject, setSubject] = useState(detail.email?.subject ?? "");
  const [body, setBody] = useState(detail.email?.body ?? "");
  const [busy, setBusy] = useState(false);
  const [clipboardNotice, setClipboardNotice] = useState<string | null>(null);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const permissions = client.consent.permissions;
  const draft = canDraftEmail(detail, permissions);
  const blocked = saveBlockedReason(permissions, !!client.consent.revoked);
  const generate = async () => {
    setBusy(true);
    try {
      const result = await consultApi.generateEmail(clientId, sessionId);
      setSubject(result.email.subject);
      setBody(result.email.body);
      await onRefresh();
    } catch (cause) {
      onError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setBusy(false);
    }
  };
  const save = async () => {
    setBusy(true);
    try {
      await consultApi.saveEmail(clientId, sessionId, { subject, body });
      await onRefresh();
    } catch (cause) {
      onError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setBusy(false);
    }
  };
  const open = async () => {
    const mailto = mailtoFor(subject, body);
    setClipboardNotice(null);
    try {
      await openEmailWithClipboard(mailto, body, {
        writeText: (text) => navigator.clipboard.writeText(text),
        openUrl,
        markEmailOpened: () => consultApi.markEmailOpened(clientId, sessionId),
        onClipboardFailure: () =>
          setClipboardNotice(t("consult.session.clipboardFailure")),
      });
      await onRefresh();
    } catch (cause) {
      onError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const copy = async () => {
    setClipboardNotice(null);
    try {
      await navigator.clipboard.writeText(`${subject}\n\n${body}`);
    } catch {
      setClipboardNotice(t("consult.session.clipboardFailure"));
    }
  };
  const stale = emailIsStale(detail.email, detail.versions);
  const startGenerate = () => {
    if (
      detail.email &&
      regenerateNeedsConfirm(detail.email) &&
      !confirmReplace
    ) {
      setConfirmReplace(true);
      return;
    }
    setConfirmReplace(false);
    void generate();
  };
  return (
    <section className="max-w-3xl">
      {!draft.ok && (
        <p className="mb-5 border border-rule bg-surface p-3 text-sm text-secondary">
          {t(
            draft.reason === "council"
              ? "consult.session.noCouncil"
              : "consult.session.noNoteEmail",
          )}
        </p>
      )}
      {blocked === "revoked" && (
        <p className="mb-5 border border-err p-3 text-sm text-err">
          {t("consult.session.revokedReadOnly")}
        </p>
      )}
      {stale && detail.email && (
        <p className="mb-5 border border-warn-edge bg-warn-surface p-3 text-sm text-warn">
          {t("consult.session.emailStale", {
            n: detail.email.from_note ?? "—",
          })}
        </p>
      )}
      {detail.email?.provenance && (
        <ProvenanceChip provenance={detail.email.provenance} detail={detail} />
      )}
      {detail.email && (
        <p className="my-3 font-mono text-xs text-secondary">
          {t("consult.session.emailFromNote", {
            n: detail.email.from_note ?? "—",
          })}
        </p>
      )}
      <label className="block">
        <span className="mb-1 block text-sm text-secondary">
          {t("consult.session.subject")}
        </span>
        <input
          value={subject}
          onChange={(event) => setSubject(event.target.value)}
          disabled={blocked === "revoked"}
          className="h-11 w-full border-0 border-b border-rule-strong bg-transparent text-primary outline-none focus:border-accent"
        />
      </label>
      <label className="mt-5 block">
        <span className="mb-1 block text-sm text-secondary">
          {t("consult.session.body")}
        </span>
        <textarea
          value={body}
          onChange={(event) => setBody(event.target.value)}
          disabled={blocked === "revoked"}
          className="min-h-64 w-full resize-y border border-rule bg-transparent p-3 font-serif text-base text-primary outline-none focus:border-accent"
        />
      </label>
      {clipboardNotice && (
        <p className="mt-3 text-sm text-secondary">{clipboardNotice}</p>
      )}
      {confirmReplace && (
        <p className="mt-3 border border-warn-edge bg-warn-surface p-3 text-sm text-warn">
          {t("consult.session.replaceWarning")}
        </p>
      )}
      <div className="mt-5 flex flex-wrap gap-3">
        <button
          type="button"
          disabled={!draft.ok || busy}
          onClick={startGenerate}
          className="h-11 bg-accent px-4 text-on-accent disabled:opacity-50"
        >
          {confirmReplace
            ? t("consult.session.replaceConfirm")
            : detail.email
              ? regenerateNeedsConfirm(detail.email)
                ? t("consult.session.regenerateEmailConfirm")
                : t("consult.session.regenerate")
              : t("consult.session.writeEmail")}
        </button>
        <button
          type="button"
          disabled={blocked !== null || busy || !subject.trim()}
          onClick={() => void save()}
          className="h-11 border border-rule-strong px-4 text-primary disabled:opacity-50"
        >
          {t("consult.session.saveEmail")}
        </button>
        <button
          type="button"
          disabled={!subject.trim() || !body.trim() || busy}
          onClick={() => void open()}
          className="h-11 border border-rule-strong px-4 text-primary disabled:opacity-50"
        >
          {t("consult.session.openMail")}
        </button>
        <button
          type="button"
          disabled={!subject.trim() || !body.trim() || busy}
          onClick={() => void copy()}
          className="h-11 border border-rule-strong px-4 text-primary disabled:opacity-50"
        >
          {t("consult.session.copyEmail")}
        </button>
      </div>
    </section>
  );
}
