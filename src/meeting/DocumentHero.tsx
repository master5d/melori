import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ClientNote, MeetingAnalysis } from "@/bindings";
import NoteView from "./NoteView";
import { formatElapsed, type Row } from "./copilotLogic";
import { type CouncilState, isLocalWellbeing } from "./councilView";
import CouncilControls, { type LensOption } from "./CouncilControls";
import { canAnalyze, canSave } from "./analysisView";
import type { NoteTemplate } from "@/consult/api";
import {
  notesSections,
  actionSections,
  checklistKey,
  isChecked,
} from "./documentView";

interface DocumentViewProps {
  mode: "business" | "session";
  templates: NoteTemplate[];
  templateId: string;
  hasClient: boolean;
  onTemplateChange: (id: string) => void;
  analysis: MeetingAnalysis | null;
  clientNote?: ClientNote | null;
  analysisError: string | null;
  analyzing: boolean;
  rows: Row[];
  elapsedMs: number;
  destLabel: string;
  warn: boolean;
  wellbeingUrl: string;
  council: CouncilState;
  consulting: boolean;
  lenses: LensOption[];
  selectedLenses: ReadonlySet<string>;
  onToggleLens: (id: string) => void;
  onToggleAllLenses: () => void;
  copied: boolean;
  saving: boolean;
  savedPath: string | null;
  saveError: string | null;
  checked: ReadonlySet<string>;
  onToggleCheck: (key: string) => void;
  onSwitchMode: (m: "business" | "session") => void;
  onAnalyze: () => void;
  onSave: () => void;
  onConsult: () => void;
  onCopy: () => void;
  onClose: () => void;
}

/** Renders one notes section: a string body or a plain bullet list. */
function NotesSection({
  titleKey,
  body,
}: {
  titleKey: string;
  body: string | string[];
}) {
  const { t } = useTranslation();
  return (
    <div className="mc-sec">
      <div className="mc-sec-title">{t(titleKey)}</div>
      {Array.isArray(body) ? (
        <ul className="mc-sec-list">
          {body.map((item, i) => (
            <li key={i}>{item}</li>
          ))}
        </ul>
      ) : (
        <div className="mc-sec-body">{body}</div>
      )}
    </div>
  );
}

/** Renders one actions section as an ephemeral checklist (string bodies render as prose). */
function ActionSection({
  titleKey,
  body,
  checked,
  onToggleCheck,
}: {
  titleKey: string;
  body: string | string[];
  checked: ReadonlySet<string>;
  onToggleCheck: (key: string) => void;
}) {
  const { t } = useTranslation();
  if (!Array.isArray(body)) {
    return (
      <div className="mc-sec">
        <div className="mc-sec-title">{t(titleKey)}</div>
        <div className="mc-sec-body">{body}</div>
      </div>
    );
  }
  return (
    <div className="mc-sec">
      <div className="mc-sec-title">{t(titleKey)}</div>
      <ul className="mc-check-list">
        {body.map((item, i) => {
          const key = checklistKey(titleKey, i);
          const on = isChecked(checked, key);
          return (
            <li key={i} className="mc-check-item">
              <label className="mc-check">
                <input
                  type="checkbox"
                  checked={on}
                  onChange={() => onToggleCheck(key)}
                />
                <span className={on ? "mc-check-done" : ""}>{item}</span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Document hero: enhanced Notes/Actions with a collapsible transcript. */
export default function DocumentView({
  mode,
  templates,
  templateId,
  hasClient,
  onTemplateChange,
  analysis,
  clientNote = null,
  analysisError,
  analyzing,
  rows,
  elapsedMs,
  destLabel,
  warn,
  wellbeingUrl,
  council,
  consulting,
  lenses,
  selectedLenses,
  onToggleLens,
  onToggleAllLenses,
  copied,
  saving,
  savedPath,
  saveError,
  checked,
  onToggleCheck,
  onSwitchMode,
  onAnalyze,
  onSave,
  onConsult,
  onCopy,
  onClose,
}: DocumentViewProps) {
  const { t } = useTranslation();
  const [showTranscript, setShowTranscript] = useState(false);
  const hasCouncil =
    council.opinions.length > 0 ||
    council.synthesis !== null ||
    council.error !== null;

  return (
    <div className="mc-root mc-doc">
      <header className="mc-header" data-tauri-drag-region>
        <span className="mc-dot idle mc-done" />
        <span className="mc-rec-label">{t("meetingCopilot.doc.done")}</span>
        <span className="mc-elapsed">{formatElapsed(elapsedMs)}</span>
        <span className="mc-mode">{t("meetingCopilot.doc.notesLabel")}</span>
        <span className="mc-spacer" />
        <button className="mc-btn mc-btn-ghost" onClick={onClose}>
          {t("meetingCopilot.close")}
        </button>
      </header>

      <div className="mc-doc-body">
        <div className="mc-analysis-controls">
          {!clientNote && (
            <div className="mc-modes">
              {hasClient ? (
                <label className="mc-tab active">
                  {t("meetingCopilot.analysis.template")}{" "}
                  <select
                    value={templateId}
                    onChange={(event) => onTemplateChange(event.target.value)}
                  >
                    <option value="soap">
                      {t("meetingCopilot.analysis.soap")}
                    </option>
                    {templates
                      .filter(
                        (template) =>
                          template.id !== "free" && template.id !== "soap",
                      )
                      .map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                  </select>
                </label>
              ) : (
                <>
                  <button
                    className={`mc-tab ${mode === "business" ? "active" : ""}`}
                    onClick={() => onSwitchMode("business")}
                  >
                    {t("meetingCopilot.analysis.business")}
                  </button>
                  <button
                    className={`mc-tab ${mode === "session" ? "active" : ""}`}
                    onClick={() => onSwitchMode("session")}
                  >
                    {t("meetingCopilot.analysis.session")}
                  </button>
                </>
              )}
            </div>
          )}
          <span className="mc-spacer" />
          {!clientNote && (
            <button
              className="mc-btn mc-btn-primary"
              disabled={!canAnalyze(rows.length, analyzing)}
              onClick={onAnalyze}
            >
              {analyzing
                ? t("meetingCopilot.analysis.analyzing")
                : t("meetingCopilot.analysis.analyze")}
            </button>
          )}
          <button
            className="mc-btn mc-btn-ghost"
            disabled={!canSave(rows.length) || saving}
            onClick={onSave}
          >
            {saving
              ? t("meetingCopilot.save.saving")
              : t("meetingCopilot.save.save")}
          </button>
          {(mode === "session" || hasClient) && (
            <button
              className="mc-btn mc-btn-ghost"
              disabled={
                consulting ||
                (rows.length === 0 && !analysis) ||
                (lenses.length > 0 && selectedLenses.size === 0)
              }
              onClick={onConsult}
            >
              {consulting
                ? t("meetingCopilot.council.consulting")
                : lenses.length > 0
                  ? t("meetingCopilot.council.consultN", {
                      n: selectedLenses.size,
                    })
                  : t("meetingCopilot.council.consult")}
            </button>
          )}
        </div>
        {mode === "session" && (
          <CouncilControls
            lenses={lenses}
            selected={selectedLenses}
            onToggle={onToggleLens}
            onToggleAll={onToggleAllLenses}
            council={council}
            consulting={consulting}
          />
        )}

        {destLabel && (
          <div className="mc-dest">
            {t("meetingCopilot.analysis.destination", { label: destLabel })}
          </div>
        )}
        {warn && (
          <div className="mc-warn">
            {t("meetingCopilot.analysis.sessionWarning")}
          </div>
        )}
        {(mode === "session" || hasClient) && wellbeingUrl && (
          <>
            <div className="mc-dest">
              {t("meetingCopilot.council.target", { url: wellbeingUrl })}
            </div>
            {!isLocalWellbeing(wellbeingUrl) && (
              <div className="mc-warn">
                {t("meetingCopilot.council.warning")}
              </div>
            )}
          </>
        )}
        {savedPath && (
          <div className="mc-dest">
            {t("meetingCopilot.save.savedTo", { path: savedPath })}
          </div>
        )}
        {saveError && <div className="mc-warn">{saveError}</div>}

        {analysisError && (
          <div className="mc-analysis-error">
            <span>{analysisError}</span>
            <button className="mc-btn mc-btn-ghost" onClick={onAnalyze}>
              {t("meetingCopilot.analysis.retry")}
            </button>
          </div>
        )}

        {!analysis && !clientNote && !analysisError && (
          <div className="mc-waiting">{t("meetingCopilot.doc.emptyHint")}</div>
        )}

        {clientNote && <NoteView note={clientNote} />}
        {clientNote && !clientNote.stored && (
          <div className="mc-warn">{t("meetingCopilot.notRetained")}</div>
        )}
        {analysis && !clientNote && (
          <>
            <div className="mc-group-title">
              {t("meetingCopilot.doc.notesGroup")}
            </div>
            {notesSections(analysis).map((sec) => (
              <NotesSection
                key={sec.titleKey}
                titleKey={sec.titleKey}
                body={sec.body}
              />
            ))}

            <div className="mc-group-title">
              {t("meetingCopilot.doc.actionsGroup")}
            </div>
            {actionSections(analysis).map((sec) => (
              <ActionSection
                key={sec.titleKey}
                titleKey={sec.titleKey}
                body={sec.body}
                checked={checked}
                onToggleCheck={onToggleCheck}
              />
            ))}

            <button className="mc-btn mc-btn-ghost mc-copy" onClick={onCopy}>
              {copied
                ? t("meetingCopilot.analysis.copied")
                : t("meetingCopilot.analysis.copy")}
            </button>
          </>
        )}
        {clientNote && (
          <button className="mc-btn mc-btn-ghost mc-copy" onClick={onCopy}>
            {copied
              ? t("meetingCopilot.analysis.copied")
              : t("meetingCopilot.analysis.copy")}
          </button>
        )}

        {hasCouncil && (
          <div className="mc-council">
            <div className="mc-group-title">
              {t("meetingCopilot.doc.councilGroup")}
            </div>
            {council.opinions.map((o) => (
              <div key={o.specialist_id} className="mc-sec">
                <div className="mc-sec-title">
                  {o.name} · {o.paradigm}
                </div>
                <div className="mc-sec-body">{o.text}</div>
              </div>
            ))}
            {council.synthesis && (
              <div className="mc-sec">
                <div className="mc-sec-title">
                  {t("meetingCopilot.council.synthesis")}
                </div>
                <div className="mc-sec-body">{council.synthesis.text}</div>
                {council.synthesis.convergences.length > 0 && (
                  <>
                    <div className="mc-sec-title">
                      {t("meetingCopilot.council.convergences")}
                    </div>
                    <ul className="mc-sec-list">
                      {council.synthesis.convergences.map((c, i) => (
                        <li key={i}>{c}</li>
                      ))}
                    </ul>
                  </>
                )}
                {council.synthesis.divergences.length > 0 && (
                  <>
                    <div className="mc-sec-title">
                      {t("meetingCopilot.council.divergences")}
                    </div>
                    <ul className="mc-sec-list">
                      {council.synthesis.divergences.map((d, i) => (
                        <li key={i}>{d}</li>
                      ))}
                    </ul>
                  </>
                )}
              </div>
            )}
            {council.error && <div className="mc-warn">{council.error}</div>}
          </div>
        )}

        <div className="mc-transcript-fold">
          <button
            className="mc-fold-toggle"
            aria-expanded={showTranscript}
            onClick={() => setShowTranscript((v) => !v)}
          >
            {/* Блочная пара, а НЕ disable-next-line: строчная директива привязана
                к следующей строке и ломается, как только prettier переносит тег.
                Ровно так этот файл и упал на code-quality после свипа форматирования. */}
            {/* eslint-disable i18next/no-literal-string -- decorative glyph, aria-hidden */}
            <span
              className={`mc-chevron ${showTranscript ? "open" : ""}`}
              aria-hidden="true"
            >
              ▸
            </span>
            {/* eslint-enable i18next/no-literal-string */}
            {t("meetingCopilot.doc.transcript")}
          </button>
          {showTranscript && (
            <div className="mc-transcript mc-transcript-collapsed">
              {rows.length === 0 ? (
                <div className="mc-waiting">{t("meetingCopilot.waiting")}</div>
              ) : (
                rows.map((r) => (
                  <div key={r.id} className={`mc-row mc-${r.source}`}>
                    <span className="mc-gutter">
                      {t(`meetingCopilot.${r.source}`)}
                    </span>
                    <span className="mc-text">{r.text}</span>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
