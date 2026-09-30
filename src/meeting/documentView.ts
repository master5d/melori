//! Pure information-architecture helpers for the Granola-flip Meeting Copilot.
//! No React, no Tauri runtime imports (the type import is erased) — vitest-hermetic.
import type { MeetingAnalysis } from "@/bindings";
import type { Section } from "./analysisView";

/** Backend-driven meeting phase (unchanged contract from MeetingCopilot.tsx). */
export type MeetingPhase =
  | "loading"
  | "consent"
  | "recording"
  | "ended"
  | "error";

/** Which hero/screen the panel renders for a given phase. */
export type MeetingView = "loading" | "consent" | "error" | "live" | "document";

/** Recording → live-flow hero; ended → document hero; other phases pass through. */
export function selectView(phase: MeetingPhase): MeetingView {
  switch (phase) {
    case "recording":
      return "live";
    case "ended":
      return "document";
    case "consent":
      return "consent";
    case "error":
      return "error";
    case "loading":
      return "loading";
  }
}

/** Notes group — narrative sections (brief: summary+key points; SOAP: S/O/A). */
export function notesSections(a: MeetingAnalysis): Section[] {
  if (a.kind === "brief") {
    return [
      { titleKey: "meetingCopilot.analysis.summary", body: a.summary },
      { titleKey: "meetingCopilot.analysis.keyPoints", body: a.key_points },
    ];
  }
  return [
    { titleKey: "meetingCopilot.analysis.subjective", body: a.subjective },
    { titleKey: "meetingCopilot.analysis.objective", body: a.objective },
    { titleKey: "meetingCopilot.analysis.assessment", body: a.assessment },
  ];
}

/** Actions group — actionable lists (brief: action items+open questions; SOAP: plan). */
export function actionSections(a: MeetingAnalysis): Section[] {
  if (a.kind === "brief") {
    return [
      { titleKey: "meetingCopilot.analysis.actionItems", body: a.action_items },
      {
        titleKey: "meetingCopilot.analysis.openQuestions",
        body: a.open_questions,
      },
    ];
  }
  return [{ titleKey: "meetingCopilot.analysis.plan", body: a.plan }];
}

/** Stable checklist key for one action-list item. */
export function checklistKey(titleKey: string, index: number): string {
  return `${titleKey}:${index}`;
}

/** Return a new set with `key` toggled — never mutates the input. */
export function toggleChecked(
  checked: ReadonlySet<string>,
  key: string,
): Set<string> {
  const next = new Set(checked);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  return next;
}

/** Whether an action item is currently ticked. */
export function isChecked(checked: ReadonlySet<string>, key: string): boolean {
  return checked.has(key);
}
