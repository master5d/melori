import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, events } from "@/bindings";
import type { AskView, ClientNote, MeetingAnalysis } from "@/bindings";
import {
  decideMount,
  appendRow,
  sourceMode,
  formatElapsed,
  type Row,
} from "./copilotLogic";
import {
  toCopyText,
  canAnalyze,
  showSessionWarning,
  canSave,
} from "./analysisView";
import {
  councilSituation,
  applyCouncilEvent,
  emptyCouncil,
  initialLensSelection,
  isLocalWellbeing,
  type CouncilState,
} from "./councilView";
import LiveView from "./LiveView";
import DocumentView from "./DocumentHero";
import { selectView, toggleChecked } from "./documentView";
import "./MeetingCopilot.css";
import {
  consultApi,
  type CouncilSpecialist,
  type EngineClient,
  type NoteTemplate,
} from "@/consult/api";
import { councilTabVisible, protectionBadgeVisible } from "./meetingView";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { PANEL_H, PANEL_W, PILL_H, isDragStart } from "./meetingView";
import { noteCopyText } from "./NoteView";
import { RecapPlate } from "./RecapPlate";
import { AskPanel } from "./AskPanel";
import { VoiceLights } from "./VoiceLights";
import { SilencePlate } from "./SilencePlate";

type Phase =
  | "loading"
  | "consent"
  | "client-select"
  | "recording"
  | "ended"
  | "error";
type Mode = "business" | "session";

export default function MeetingCopilot() {
  const { t, i18n } = useTranslation();
  const [phase, setPhase] = useState<Phase>("loading");
  const phaseRef = useRef<Phase>("loading");
  phaseRef.current = phase;
  const [rows, setRows] = useState<Row[]>([]);
  const [loopbackActive, setLoopbackActive] = useState(false);
  const [elapsedMs, setElapsedMs] = useState(0);
  const startedAtRef = useRef<number | null>(null);

  // C4 analysis state
  const [mode, setMode] = useState<Mode>("business");
  const [analysis, setAnalysis] = useState<MeetingAnalysis | null>(null);
  const [clientNote, setClientNote] = useState<ClientNote | null>(null);
  const [templateId, setTemplateId] = useState("soap");
  const [templates, setTemplates] = useState<NoteTemplate[]>([]);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [destLabel, setDestLabel] = useState<string>("");
  const [destLocal, setDestLocal] = useState(false);
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [clients, setClients] = useState<EngineClient[]>([]);
  const [selectedClientId, setSelectedClientId] = useState("");
  const [confirmingCouncil, setConfirmingCouncil] = useState(false);
  const isRecording = phase === "recording";

  const [council, setCouncil] = useState<CouncilState>(emptyCouncil);
  const [lenses, setLenses] = useState<CouncilSpecialist[]>([]);
  const [selectedLenses, setSelectedLenses] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const [consulting, setConsulting] = useState(false);
  const [wellbeingUrl, setWellbeingUrl] = useState("");
  const [checked, setChecked] = useState<ReadonlySet<string>>(new Set());
  const [hideNotesFromScreenShare, setHideNotesFromScreenShare] =
    useState(false);
  const [activeTab, setActiveTab] = useState<
    "transcript" | "analysis" | "council"
  >("transcript");
  const [collapsed, setCollapsed] = useState(false);
  const [confirmingNew, setConfirmingNew] = useState(false);
  const [asks, setAsks] = useState<AskView[]>([]);
  const [speaking, setSpeaking] = useState({ me: false, others: false });
  const [silenceStartedAt, setSilenceStartedAt] = useState<number | null>(null);
  const [lastVoiceAt, setLastVoiceAt] = useState<number | null>(null);
  const askFocusRef = useRef<(() => void) | null>(null);
  const shortcutHandlersRef = useRef<{
    ask: (question: string, kind: string) => Promise<void>;
    runAnalyze: () => Promise<void>;
    copyAnalysis: () => Promise<void>;
    runConsult: () => Promise<void>;
  }>({
    ask: async () => {},
    runAnalyze: async () => {},
    copyAnalysis: async () => {},
    runConsult: async () => {},
  });

  // undecorated window: the pill row is the title bar
  const dragByPill = (e: React.MouseEvent) => {
    if (isDragStart(e.button, e.target as Element))
      void getCurrentWindow().startDragging();
  };
  const toggleCollapsed = async () => {
    const next = !collapsed;
    setCollapsed(next);
    await getCurrentWindow().setSize(
      new LogicalSize(PANEL_W, next ? PILL_H : PANEL_H),
    );
  };

  const beginCapture = async () => {
    const res = selectedClientId
      ? await commands.startClientMeeting(selectedClientId)
      : await commands.startMeeting();
    if (res.status === "error") {
      setPhase("error");
      return;
    }
    startedAtRef.current = Date.now();
    setPhase("recording");
  };

  // lens choice persists per viewer; the engine's list decides what is offered
  const LENS_KEY = "melori.council.lenses";
  useEffect(() => {
    consultApi
      .councilSpecialists()
      .then((offered) => {
        setLenses(offered);
        let stored: string[] | null = null;
        try {
          stored = JSON.parse(localStorage.getItem(LENS_KEY) ?? "null");
        } catch {
          stored = null;
        }
        setSelectedLenses(
          initialLensSelection(
            offered.map((l) => l.id),
            stored,
          ),
        );
      })
      .catch(() => setLenses([]));
  }, []);

  useEffect(() => {
    const unVoice = events.meetingVoiceEvent.listen((event) => {
      const { source, speaking: isSpeaking } = event.payload;
      setSpeaking((current) => ({ ...current, [source]: isSpeaking }));
      if (isSpeaking) {
        setLastVoiceAt(Date.now());
        setSilenceStartedAt(null);
      }
    });
    const unSilence = events.meetingSilenceEvent.listen(() => {
      // the 60 s countdown starts when the warning arrives, not when the silence began
      setSilenceStartedAt(Date.now());
      setLastVoiceAt(null);
    });
    const unShortcut = events.meetingShortcutEvent.listen((event) => {
      // through the ref: the listener outlives renders, the handlers must see current state
      const h = shortcutHandlersRef.current;
      const action = event.payload.action;
      if (action === "meeting_ask_focus") askFocusRef.current?.();
      else if (
        action === "meeting_ask_recap5" ||
        action === "meeting_ask_missed" ||
        action === "meeting_ask_agreed"
      )
        void h.ask("", action.replace("meeting_ask_", ""));
      else if (action === "meeting_analyze") void h.runAnalyze();
      else if (action === "meeting_copy_note") void h.copyAnalysis();
      else if (action === "meeting_council") void h.runConsult();
    });
    return () => {
      unVoice.then((dispose) => dispose());
      unSilence.then((dispose) => dispose());
      unShortcut.then((dispose) => dispose());
    };
  }, []);
  const rememberLenses = (next: Set<string>) => {
    setSelectedLenses(next);
    try {
      localStorage.setItem(LENS_KEY, JSON.stringify([...next]));
    } catch {
      /* storage may be unavailable — the choice just isn't remembered */
    }
  };
  const toggleLens = (id: string) => {
    const next = new Set(selectedLenses);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    rememberLenses(next);
  };
  const toggleAllLenses = () =>
    rememberLenses(
      lenses.every((l) => selectedLenses.has(l.id))
        ? new Set()
        : new Set(lenses.map((l) => l.id)),
    );

  // значок «скрыто из показа» следует настройке и при открытой панели
  useEffect(() => {
    const un = listen<{ setting?: string; value?: unknown }>(
      "settings-changed",
      (e) => {
        if (e.payload?.setting === "hide_notes_from_screen_share")
          setHideNotesFromScreenShare(e.payload.value === true);
      },
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    (async () => {
      const status = await commands.meetingStatus();
      const settings = await commands.getAppSettings();
      const consentAcked =
        settings.status === "ok"
          ? !!settings.data.meeting_consent_acked
          : false;
      if (settings.status === "ok")
        setWellbeingUrl(settings.data.wellbeing_url ?? "");
      if (settings.status === "ok")
        setHideNotesFromScreenShare(
          !!settings.data.hide_notes_from_screen_share,
        );
      const d = decideMount(status, consentAcked);
      setLoopbackActive(d.loopbackActive);
      try {
        const dest = await commands.meetingLlmDestination();
        setDestLabel(dest.label);
        setDestLocal(dest.is_local);
      } catch {
        /* destination line is best-effort */
      }
      if (d.action === "reattach") {
        await followStartedMeeting();
      } else if (d.action === "consent") {
        setPhase("consent");
      } else {
        try {
          const nextClients = await consultApi.listClients();
          setClients(nextClients.filter((client) => client.consent.active));
          setTemplates(await consultApi.templates(i18n.language));
        } catch {
          setClients([]);
        }
        setPhase("client-select");
      }
    })();
  }, []);

  useEffect(() => {
    const unSeg = events.segmentEvent.listen((e) => {
      setRows((prev) => appendRow(prev, e.payload.segment, 200));
    });
    const unState = events.meetingStateEvent.listen((e) => {
      setLoopbackActive(e.payload.loopback_active);
      if (!e.payload.active) setPhase((p) => (p === "error" ? p : "ended"));
      else if (phaseRef.current !== "recording") void followStartedMeeting();
    });
    return () => {
      unSeg.then((f) => f());
      unState.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const un = events.councilEvent.listen((e) => {
      setCouncil((s) => applyCouncilEvent(s, e.payload.frame));
      if (e.payload.frame.kind === "done" || e.payload.frame.kind === "error") {
        setConsulting(false);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (phase !== "recording") return;
    const id = setInterval(() => {
      if (startedAtRef.current !== null) {
        setElapsedMs(Date.now() - startedAtRef.current);
      }
    }, 1000);
    return () => clearInterval(id);
  }, [phase]);

  useEffect(() => {
    if (phase === "ended") setActiveTab("analysis");
  }, [phase]);

  const acceptConsent = async () => {
    await commands.setMeetingConsentAcked();
    setPhase("client-select");
    try {
      setClients(
        (await consultApi.listClients()).filter(
          (client) => client.consent.active,
        ),
      );
      setTemplates(await consultApi.templates(i18n.language));
    } catch {
      setClients([]);
    }
  };
  const cancelConsent = async () => {
    await commands.hideMeetingCopilot();
  };
  // Stop ends capture but keeps the panel: analysis and Save live on the ended view
  const stop = async () => {
    await commands.stopMeeting();
  };
  const ask = async (question: string, kind: string) => {
    if (!selectedClientId || !showCouncilTab) return;
    const result = await commands.askMeeting(question, kind);
    if (result.status === "ok") setAsks((current) => [result.data, ...current]);
  };
  const close = async () => {
    // an ended client meeting with an unsaved transcript is saved before the panel goes
    if (phase === "ended" && rows.length > 0 && !savedPath) {
      const binding = await commands.clientBinding();
      if (binding) {
        setSaving(true);
        const res = await commands.saveClientMeeting();
        setSaving(false);
        if (res.status === "error") {
          setSaveError(res.error);
          return;
        }
        setSavedPath(t("meetingCopilot.saved"));
      }
    }
    await commands.hideMeetingCopilot();
  };
  // after a meeting has ended: back to client selection with a clean panel;
  // an unsaved transcript needs a second, explicit click
  const resetSession = () => {
    setConfirmingNew(false);
    setRows([]);
    setElapsedMs(0);
    startedAtRef.current = null;
    setAnalysis(null);
    setClientNote(null);
    setAnalysisError(null);
    setAnalyzing(false);
    setCopied(false);
    setSaving(false);
    setSavedPath(null);
    setSaveError(null);
    setSelectedClientId("");
    setConfirmingCouncil(false);
    setCouncil(emptyCouncil);
    setConsulting(false);
    setChecked(new Set());
    setActiveTab("transcript");
    setAsks([]);
    setSpeaking({ me: false, others: false });
    setSilenceStartedAt(null);
    setLastVoiceAt(null);
  };
  // a meeting started from the client card: clean panel, bound client, recording
  const followStartedMeeting = async () => {
    resetSession();
    const binding = await commands.clientBinding();
    if (binding) {
      try {
        setClients(await consultApi.listClients());
      } catch {
        /* the label falls back to "Notes" */
      }
      try {
        // a meeting started from the client card arrives here, not via client-select
        setTemplates(await consultApi.templates(i18n.language));
      } catch {
        /* the picker keeps SOAP only */
      }
      setSelectedClientId(binding.client_id);
    }
    startedAtRef.current = Date.now();
    setPhase("recording");
  };
  const newMeeting = async () => {
    if (rows.length > 0 && !savedPath && !confirmingNew) {
      setConfirmingNew(true);
      return;
    }
    resetSession();
    try {
      setClients(
        (await consultApi.listClients()).filter(
          (client) => client.consent.active,
        ),
      );
    } catch {
      setClients([]);
    }
    setPhase("client-select");
  };

  useEffect(() => {
    const client = clients.find((item) => item.id === selectedClientId);
    if (client) setTemplateId(client.template_id ?? "soap");
  }, [clients, selectedClientId]);

  const runAnalyze = async () => {
    if (!canAnalyze(rows.length, analyzing)) return;
    setAnalyzing(true);
    setAnalysisError(null);
    setCopied(false);
    setChecked(new Set());
    const binding = await commands.clientBinding();
    if (binding) {
      const res = await commands.generateClientNote(templateId);
      if (res.status === "ok") setClientNote(res.data);
      else {
        setAnalysis(null);
        setClientNote(null);
        setAnalysisError(res.error);
      }
    } else {
      const res = await commands.analyzeMeeting(mode);
      if (res.status === "ok") setAnalysis(res.data);
      else {
        setAnalysis(null);
        setClientNote(null);
        setAnalysisError(res.error);
      }
    }
    setAnalyzing(false);
  };

  const copyAnalysis = async () => {
    if (!analysis && !clientNote) return;
    try {
      await navigator.clipboard.writeText(
        clientNote ? noteCopyText(clientNote) : toCopyText(analysis!),
      );
      setCopied(true);
    } catch {
      /* clipboard best-effort */
    }
  };

  const switchMode = (m: Mode) => {
    if (m === mode) return;
    setMode(m);
    setAnalysis(null);
    setAnalysisError(null);
  };

  const toggleCheck = (key: string) => setChecked((c) => toggleChecked(c, key));

  const runSave = async () => {
    if (!canSave(rows.length) || saving) return;
    setSaving(true);
    setSaveError(null);
    setSavedPath(null);
    const binding = await commands.clientBinding();
    if (binding) {
      const res = await commands.saveClientMeeting();
      if (res.status === "ok") {
        setSavedPath(
          res.data.retained
            ? t("meetingCopilot.saved")
            : t("meetingCopilot.notRetained"),
        );
        if (!res.data.retained) setSaveError(t("meetingCopilot.notRetained"));
      } else setSaveError(res.error);
    } else {
      const res = await commands.saveMeeting(mode, analysis);
      if (res.status === "ok") setSavedPath(res.data);
      else setSaveError(res.error);
    }
    setSaving(false);
  };

  const runConsult = async () => {
    if (consulting) return;
    const binding = await commands.clientBinding();
    if (binding) {
      try {
        if (!(await consultApi.health()).llm_local) {
          setConfirmingCouncil(true);
          return;
        }
      } catch {
        setConfirmingCouncil(true);
        return;
      }
    } else if (!destLocal) {
      setConfirmingCouncil(true);
      return;
    }
    await runConsultConfirmed();
  };

  shortcutHandlersRef.current = { ask, runAnalyze, copyAnalysis, runConsult };
  const runConsultConfirmed = async () => {
    setConfirmingCouncil(false);
    setCouncil(emptyCouncil);
    setConsulting(true);
    const res = await commands.consultCouncil(
      councilSituation(analysis, clientNote),
      lenses.length > 0
        ? lenses.filter((l) => selectedLenses.has(l.id)).map((l) => l.id)
        : null,
    );
    if (res.status === "error") {
      setCouncil((s) => ({ ...s, error: res.error, done: true }));
      setConsulting(false);
    }
  };

  if (phase === "consent") {
    return (
      <div className="mc-root mc-consent">
        <div className="mc-consent-title">
          {t("meetingCopilot.consentTitle")}
        </div>
        <div className="mc-consent-body">{t("meetingCopilot.consentBody")}</div>
        <div className="mc-consent-actions">
          <button className="mc-btn mc-btn-ghost" onClick={cancelConsent}>
            {t("meetingCopilot.consentCancel")}
          </button>
          <button className="mc-btn mc-btn-primary" onClick={acceptConsent}>
            {t("meetingCopilot.consentAccept")}
          </button>
        </div>
      </div>
    );
  }

  if (phase === "client-select") {
    return (
      <div className="mc-root mc-consent">
        <div className="mc-consent-title">
          {t("meetingCopilot.chooseClient")}
        </div>
        <select
          className="mc-select"
          value={selectedClientId}
          onChange={(event) => setSelectedClientId(event.target.value)}
        >
          <option value="">{t("meetingCopilot.noClient")}</option>
          {clients.map((client) => (
            <option key={client.id} value={client.id}>
              {client.alias}
            </option>
          ))}
        </select>
        <div className="mc-consent-actions">
          <button className="mc-btn mc-btn-ghost" onClick={cancelConsent}>
            {t("meetingCopilot.consentCancel")}
          </button>
          <button
            className="mc-btn mc-btn-primary"
            onClick={() => void beginCapture()}
          >
            {t("meetingCopilot.start")}
          </button>
        </div>
      </div>
    );
  }

  if (phase === "error") {
    return (
      <div className="mc-root mc-error">
        <div className="mc-error-msg">{t("meetingCopilot.error")}</div>
        <button className="mc-btn mc-btn-ghost" onClick={close}>
          {t("meetingCopilot.close")}
        </button>
      </div>
    );
  }

  const modeKey = sourceMode(loopbackActive);
  const warn = showSessionWarning(mode, destLocal);
  const view = selectView(phase);
  const selectedClient = clients.find(
    (client) => client.id === selectedClientId,
  );
  const showCouncilTab = councilTabVisible({
    engine: selectedClient ? "ready" : "starting",
    permissions: selectedClient?.consent.permissions ?? [],
  });
  const clientLabel =
    selectedClient?.alias ?? t("meetingCopilot.doc.notesLabel");

  if (confirmingCouncil)
    return (
      <div className="mc-root mc-consent">
        <div className="mc-consent-title">
          {t("meetingCopilot.remoteConfirmTitle")}
        </div>
        <div className="mc-consent-body">
          {t("meetingCopilot.clientRemoteConfirmBody")}
        </div>
        <div className="mc-consent-actions">
          <button
            className="mc-btn mc-btn-ghost"
            onClick={() => setConfirmingCouncil(false)}
          >
            {t("meetingCopilot.consentCancel")}
          </button>
          <button
            className="mc-btn mc-btn-primary"
            onClick={() => void runConsultConfirmed()}
          >
            {t("meetingCopilot.consentAccept")}
          </button>
        </div>
      </div>
    );

  if (saveError && phase === "ended") {
    return (
      <div className={`mc-root mc-shell${collapsed ? " mc-collapsed" : ""}`}>
        <div className="mc-pill" role="status" onMouseDown={dragByPill}>
          <span className="mc-pill-time" dir="ltr">
            {t("meetingCopilot.doc.done")} · {formatElapsed(elapsedMs)}
          </span>
          <span className="mc-pill-client">{clientLabel}</span>
        </div>
        <section
          className="mc-panel mc-save-error"
          aria-label={t("meetingCopilot.saveErrorTitle")}
        >
          <div className="mc-error-msg">
            {t("meetingCopilot.saveErrorTitle")}
          </div>
          <p>{saveError}</p>
          <div className="mc-panel-actions">
            <button
              className="mc-btn mc-btn-primary"
              onClick={() => void runSave()}
            >
              {t("meetingCopilot.saveRetry")}
            </button>
            <button
              className="mc-btn mc-btn-ghost"
              onClick={() => {
                setSaveError(null);
                setActiveTab("transcript");
              }}
            >
              {t("meetingCopilot.showTranscript")}
            </button>
          </div>
        </section>
      </div>
    );
  }

  if (view === "live" || view === "loading" || activeTab === "transcript") {
    return (
      <div className={`mc-root mc-shell${collapsed ? " mc-collapsed" : ""}`}>
        <div className="mc-pill" role="status" onMouseDown={dragByPill}>
          <span className={`mc-dot ${isRecording ? "rec" : "idle"}`} />
          <span className="mc-pill-time" dir="ltr">
            {t(
              isRecording
                ? "meetingCopilot.recording"
                : "meetingCopilot.doc.done",
            )}{" "}
            · {formatElapsed(elapsedMs)}
          </span>
          <span className="mc-pill-client">{clientLabel}</span>
          {protectionBadgeVisible(hideNotesFromScreenShare) && (
            <span className="mc-protection">
              {t("meetingCopilot.protectionBadge")}
            </span>
          )}
          <span className="mc-spacer" />
          <button
            type="button"
            className="mc-btn mc-btn-ghost"
            aria-label={t(
              collapsed ? "meetingCopilot.expand" : "meetingCopilot.collapse",
            )}
            title={t(
              collapsed ? "meetingCopilot.expand" : "meetingCopilot.collapse",
            )}
            onClick={toggleCollapsed}
          >
            {collapsed ? "▢" : "–"}
          </button>
          {!isRecording && phase === "ended" && (
            <button
              type="button"
              className="mc-btn mc-btn-primary"
              onClick={newMeeting}
            >
              {t(
                confirmingNew
                  ? "meetingCopilot.newMeetingUnsaved"
                  : "meetingCopilot.newMeeting",
              )}
            </button>
          )}
          {isRecording ? (
            <button className="mc-btn mc-btn-stop" onClick={stop}>
              {t("meetingCopilot.stop")}
            </button>
          ) : (
            <button className="mc-btn mc-btn-ghost" onClick={close}>
              {t("meetingCopilot.close")}
            </button>
          )}
        </div>
        {phase !== "recording" && (
          <MeetingTabs
            activeTab={activeTab}
            showCouncil={showCouncilTab}
            onChange={setActiveTab}
          />
        )}
        <div className="mc-consent-line">{t("meetingCopilot.consentBody")}</div>
        {isRecording && selectedClientId && (
          <RecapPlate clientId={selectedClientId} />
        )}
        {isRecording && <VoiceLights speaking={speaking} />}
        {isRecording && selectedClientId && showCouncilTab && (
          <AskPanel asks={asks} onAsk={ask} focusRef={askFocusRef} />
        )}
        {isRecording && silenceStartedAt !== null && (
          <SilencePlate
            startedAt={silenceStartedAt}
            lastVoiceAt={lastVoiceAt}
            onContinue={() => {
              void commands.resetSilence();
              setSilenceStartedAt(null);
              setLastVoiceAt(null);
            }}
            onExpire={stop}
          />
        )}
        <div className="mc-panel mc-panel-content">
          <LiveView
            isRecording={isRecording}
            elapsedMs={elapsedMs}
            modeKey={modeKey}
            rows={rows}
            onStop={stop}
            onClose={close}
          />
        </div>
      </div>
    );
  }

  return (
    <div className={`mc-root mc-shell${collapsed ? " mc-collapsed" : ""}`}>
      <div className="mc-pill" role="status" onMouseDown={dragByPill}>
        <span className="mc-dot idle mc-done" />
        <span className="mc-pill-time" dir="ltr">
          {t("meetingCopilot.doc.done")} · {formatElapsed(elapsedMs)}
        </span>
        <span className="mc-pill-client">{clientLabel}</span>
        {protectionBadgeVisible(hideNotesFromScreenShare) && (
          <span className="mc-protection">
            {t("meetingCopilot.protectionBadge")}
          </span>
        )}
        <span className="mc-spacer" />
        <button
          type="button"
          className="mc-btn mc-btn-ghost"
          aria-label={t(
            collapsed ? "meetingCopilot.expand" : "meetingCopilot.collapse",
          )}
          title={t(
            collapsed ? "meetingCopilot.expand" : "meetingCopilot.collapse",
          )}
          onClick={toggleCollapsed}
        >
          {collapsed ? "▢" : "–"}
        </button>
        <button
          type="button"
          className="mc-btn mc-btn-primary"
          onClick={newMeeting}
        >
          {t(
            confirmingNew
              ? "meetingCopilot.newMeetingUnsaved"
              : "meetingCopilot.newMeeting",
          )}
        </button>
        <button className="mc-btn mc-btn-ghost" onClick={close}>
          {t("meetingCopilot.close")}
        </button>
      </div>
      <MeetingTabs
        activeTab={activeTab}
        showCouncil={showCouncilTab}
        onChange={setActiveTab}
      />
      <div className="mc-consent-line">{t("meetingCopilot.consentBody")}</div>
      <div className="mc-panel mc-panel-content">
        <DocumentView
          mode={mode}
          templates={templates}
          templateId={templateId}
          hasClient={Boolean(selectedClientId)}
          onTemplateChange={setTemplateId}
          analysis={analysis}
          clientNote={clientNote}
          analysisError={analysisError}
          analyzing={analyzing}
          rows={rows}
          elapsedMs={elapsedMs}
          destLabel={destLabel}
          warn={warn}
          wellbeingUrl={wellbeingUrl}
          council={council}
          consulting={consulting}
          lenses={lenses}
          selectedLenses={selectedLenses}
          onToggleLens={toggleLens}
          onToggleAllLenses={toggleAllLenses}
          copied={copied}
          saving={saving}
          savedPath={savedPath}
          saveError={saveError}
          checked={checked}
          onToggleCheck={toggleCheck}
          onSwitchMode={switchMode}
          onAnalyze={runAnalyze}
          onSave={runSave}
          onConsult={runConsult}
          onCopy={copyAnalysis}
          onClose={close}
        />
      </div>
    </div>
  );
}

function MeetingTabs({
  activeTab,
  showCouncil,
  onChange,
}: {
  activeTab: "transcript" | "analysis" | "council";
  showCouncil: boolean;
  onChange: (tab: "transcript" | "analysis" | "council") => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="mc-tabs" role="tablist">
      <button
        role="tab"
        aria-selected={activeTab === "transcript"}
        className={activeTab === "transcript" ? "active" : ""}
        onClick={() => onChange("transcript")}
      >
        {t("meetingCopilot.doc.transcript")}
      </button>
      <button
        role="tab"
        aria-selected={activeTab === "analysis"}
        className={activeTab === "analysis" ? "active" : ""}
        onClick={() => onChange("analysis")}
      >
        {t("meetingCopilot.doc.notesLabel")}
      </button>
      {showCouncil && (
        <button
          role="tab"
          aria-selected={activeTab === "council"}
          className={activeTab === "council" ? "active" : ""}
          onClick={() => onChange("council")}
        >
          {t("meetingCopilot.council.consult")}
        </button>
      )}
    </div>
  );
}
