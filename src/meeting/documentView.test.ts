import { describe, it, expect } from "vitest";
import {
  selectView,
  notesSections,
  actionSections,
  checklistKey,
  toggleChecked,
  isChecked,
} from "./documentView";
import type { MeetingAnalysis } from "@/bindings";

const brief: MeetingAnalysis = {
  kind: "brief",
  summary: "S",
  key_points: ["k1", "k2"],
  action_items: ["a1"],
  open_questions: ["q1"],
};
const soap: MeetingAnalysis = {
  kind: "soap",
  subjective: "sub",
  objective: "obj",
  assessment: "ass",
  plan: "pl",
};

describe("selectView", () => {
  it("maps recording to the live hero", () => {
    expect(selectView("recording")).toBe("live");
  });
  it("maps ended to the document hero", () => {
    expect(selectView("ended")).toBe("document");
  });
  it("passes consent, error and loading through unchanged", () => {
    expect(selectView("consent")).toBe("consent");
    expect(selectView("error")).toBe("error");
    expect(selectView("loading")).toBe("loading");
  });
});

describe("notesSections", () => {
  it("groups brief summary + key points as notes", () => {
    expect(notesSections(brief).map((s) => s.titleKey)).toEqual([
      "meetingCopilot.analysis.summary",
      "meetingCopilot.analysis.keyPoints",
    ]);
  });
  it("groups SOAP subjective/objective/assessment as notes", () => {
    expect(notesSections(soap).map((s) => s.titleKey)).toEqual([
      "meetingCopilot.analysis.subjective",
      "meetingCopilot.analysis.objective",
      "meetingCopilot.analysis.assessment",
    ]);
  });
});

describe("actionSections", () => {
  it("groups brief action items + open questions as actions", () => {
    expect(actionSections(brief).map((s) => s.titleKey)).toEqual([
      "meetingCopilot.analysis.actionItems",
      "meetingCopilot.analysis.openQuestions",
    ]);
  });
  it("groups the SOAP plan as the sole action section", () => {
    expect(actionSections(soap).map((s) => s.titleKey)).toEqual([
      "meetingCopilot.analysis.plan",
    ]);
  });
});

describe("checklist helpers", () => {
  it("builds a stable per-item key", () => {
    expect(checklistKey("meetingCopilot.analysis.actionItems", 2)).toBe(
      "meetingCopilot.analysis.actionItems:2",
    );
  });
  it("toggles a key on and off without mutating the input", () => {
    const empty = new Set<string>();
    const on = toggleChecked(empty, "k:0");
    expect(isChecked(on, "k:0")).toBe(true);
    expect(isChecked(empty, "k:0")).toBe(false); // input untouched
    const off = toggleChecked(on, "k:0");
    expect(isChecked(off, "k:0")).toBe(false);
  });
  it("keeps independent keys independent", () => {
    const s = toggleChecked(toggleChecked(new Set(), "k:0"), "k:1");
    expect(isChecked(s, "k:0")).toBe(true);
    expect(isChecked(s, "k:1")).toBe(true);
    expect(isChecked(s, "k:2")).toBe(false);
  });
});
