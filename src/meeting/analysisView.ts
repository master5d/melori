//! Pure render/clipboard/gating helpers for the C4 analysis pane.
//! No React, no Tauri runtime imports (type-only import is erased) — vitest-hermetic.
import type { MeetingAnalysis } from "@/bindings";

export interface Section {
  titleKey: string;
  body: string | string[];
}

/** Ordered, i18n-keyed sections for the results pane. */
export function analysisSections(a: MeetingAnalysis): Section[] {
  if (a.kind === "brief") {
    return [
      { titleKey: "meetingCopilot.analysis.summary", body: a.summary },
      { titleKey: "meetingCopilot.analysis.keyPoints", body: a.key_points },
      { titleKey: "meetingCopilot.analysis.actionItems", body: a.action_items },
      {
        titleKey: "meetingCopilot.analysis.openQuestions",
        body: a.open_questions,
      },
    ];
  }
  return [
    { titleKey: "meetingCopilot.analysis.subjective", body: a.subjective },
    { titleKey: "meetingCopilot.analysis.objective", body: a.objective },
    { titleKey: "meetingCopilot.analysis.assessment", body: a.assessment },
    { titleKey: "meetingCopilot.analysis.plan", body: a.plan },
  ];
}

/** Plain-text rendering for Copy-to-clipboard. */
export function toCopyText(a: MeetingAnalysis): string {
  const bullets = (xs: string[]) => xs.map((x) => `- ${x}`).join("\n");
  if (a.kind === "brief") {
    return [
      `SUMMARY\n${a.summary}`,
      `KEY POINTS\n${bullets(a.key_points)}`,
      `ACTION ITEMS\n${bullets(a.action_items)}`,
      `OPEN QUESTIONS\n${bullets(a.open_questions)}`,
    ].join("\n\n");
  }
  return [
    `SUBJECTIVE\n${a.subjective}`,
    `OBJECTIVE\n${a.objective}`,
    `ASSESSMENT\n${a.assessment}`,
    `PLAN\n${a.plan}`,
  ].join("\n\n");
}

/** Analyze is enabled only with transcript rows and no call already running. */
export function canAnalyze(rowCount: number, inFlight: boolean): boolean {
  return rowCount > 0 && !inFlight;
}

/** Warn when a Session analysis would be sent to a non-local LLM. */
export function showSessionWarning(
  mode: "business" | "session",
  isLocal: boolean,
): boolean {
  return mode === "session" && !isLocal;
}

/** Save is enabled once there is at least one transcript row. */
export function canSave(rowCount: number): boolean {
  return rowCount > 0;
}
