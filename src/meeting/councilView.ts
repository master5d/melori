//! Pure helpers for the C5b "Consult council" pane. No React, no Tauri runtime imports.
import type {
  MeetingAnalysis,
  CouncilFrame,
  CouncilOpinion,
  CouncilSynthesis,
  ClientNote,
} from "@/bindings";
import { toCopyText } from "./analysisView";
import { noteCopyText } from "./NoteView";

/** The text sent to the council: the client note or SOAP note when present, else null (→ server uses transcript). */
export function councilSituation(
  analysis: MeetingAnalysis | null,
  note: ClientNote | null = null,
): string | null {
  if (note) return noteCopyText(note);
  return analysis && analysis.kind === "soap" ? toCopyText(analysis) : null;
}

export interface LensProgress {
  id: string;
  name: string;
  /** ms timestamp when the lens started; null while it waits for a slot */
  startedAt: number | null;
  /** seconds the lens took, once done */
  elapsedS: number | null;
}

export interface CouncilState {
  opinions: CouncilOpinion[];
  synthesis: CouncilSynthesis | null;
  error: string | null;
  done: boolean;
  /** stage telemetry from the engine's plan / lens_* / synthesis_start frames */
  startedAt: number | null;
  concurrency: number | null;
  lenses: LensProgress[];
  synthesisStartedAt: number | null;
}

export const emptyCouncil: CouncilState = {
  opinions: [],
  synthesis: null,
  error: null,
  done: false,
  startedAt: null,
  concurrency: null,
  lenses: [],
  synthesisStartedAt: null,
};

const updateLens = (
  lenses: LensProgress[],
  id: string,
  patch: Partial<LensProgress>,
): LensProgress[] =>
  lenses.some((l) => l.id === id)
    ? lenses.map((l) => (l.id === id ? { ...l, ...patch } : l))
    : [...lenses, { id, name: id, startedAt: null, elapsedS: null, ...patch }];

/** Fold one council stream frame into the accumulating state. */
export function applyCouncilEvent(
  state: CouncilState,
  frame: CouncilFrame,
  now: number = Date.now(),
): CouncilState {
  switch (frame.kind) {
    case "plan":
      return {
        ...state,
        startedAt: now,
        concurrency: frame.concurrency,
        lenses: frame.specialists.map((id) => ({
          id,
          name: id,
          startedAt: null,
          elapsedS: null,
        })),
      };
    case "lens_start":
      return {
        ...state,
        lenses: updateLens(state.lenses, frame.specialist_id, {
          name: frame.name,
          startedAt: now,
        }),
      };
    case "lens_done":
      return {
        ...state,
        lenses: updateLens(state.lenses, frame.specialist_id, {
          elapsedS: frame.elapsed_s,
        }),
      };
    case "synthesis_start":
      return { ...state, synthesisStartedAt: now };
    case "opinion":
      return { ...state, opinions: [...state.opinions, frame.opinion] };
    case "synthesis":
      return { ...state, synthesis: frame.synthesis };
    case "done":
      return { ...state, done: true };
    case "error":
      return { ...state, error: frame.detail, done: true };
  }
}

/** True when the wellbeing URL targets the local machine (PHI should stay local). */
export function isLocalWellbeing(url: string): boolean {
  return /\/\/(localhost|127\.0\.0\.1|\[::1\])(:|\/|$)/.test(url.toLowerCase());
}

export type CouncilStage = "idle" | "lenses" | "synthesis" | "done" | "error";

export interface CouncilProgress {
  stage: CouncilStage;
  done: number;
  total: number;
  running: string[];
  elapsedMs: number;
}

/** What the telemetry line shows: stage, lenses done/total, which run now, time so far. */
export function councilProgress(
  state: CouncilState,
  now: number,
): CouncilProgress {
  const total = state.lenses.length;
  const done = state.lenses.filter((l) => l.elapsedS !== null).length;
  const running = state.lenses
    .filter((l) => l.startedAt !== null && l.elapsedS === null)
    .map((l) => l.name);
  const stage: CouncilStage = state.error
    ? "error"
    : state.done
      ? "done"
      : state.synthesisStartedAt !== null
        ? "synthesis"
        : state.startedAt !== null
          ? "lenses"
          : "idle";
  const elapsedMs =
    state.startedAt === null ? 0 : Math.max(0, now - state.startedAt);
  return { stage, done, total, running, elapsedMs };
}

/** m:ss for the telemetry line. */
export function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Lens selection: a stored list filtered to the lenses the engine offers; empty → all. */
export function initialLensSelection(
  offered: readonly string[],
  stored: readonly string[] | null,
): Set<string> {
  const valid = (stored ?? []).filter((id) => offered.includes(id));
  return new Set(valid.length > 0 ? valid : offered);
}
