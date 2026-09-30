import { describe, it, expect } from "vitest";
import {
  analysisSections,
  toCopyText,
  canAnalyze,
  showSessionWarning,
  canSave,
} from "./analysisView";
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
  subjective: "subj",
  objective: "obj",
  assessment: "ass",
  plan: "pl",
};

describe("analysisSections", () => {
  it("maps a brief to 4 ordered sections", () => {
    const s = analysisSections(brief);
    expect(s.map((x) => x.titleKey)).toEqual([
      "meetingCopilot.analysis.summary",
      "meetingCopilot.analysis.keyPoints",
      "meetingCopilot.analysis.actionItems",
      "meetingCopilot.analysis.openQuestions",
    ]);
    expect(s[1].body).toEqual(["k1", "k2"]);
  });
  it("maps a SOAP to 4 ordered sections", () => {
    const s = analysisSections(soap);
    expect(s.map((x) => x.titleKey)).toEqual([
      "meetingCopilot.analysis.subjective",
      "meetingCopilot.analysis.objective",
      "meetingCopilot.analysis.assessment",
      "meetingCopilot.analysis.plan",
    ]);
    expect(s[3].body).toBe("pl");
  });
});

describe("toCopyText", () => {
  it("includes every brief field", () => {
    const t = toCopyText(brief);
    expect(t).toContain("S");
    expect(t).toContain("- k1");
    expect(t).toContain("- a1");
    expect(t).toContain("- q1");
  });
  it("includes every SOAP field", () => {
    const t = toCopyText(soap);
    for (const v of ["subj", "obj", "ass", "pl"]) expect(t).toContain(v);
  });
});

describe("canAnalyze", () => {
  it("true only with rows and no in-flight call", () => {
    expect(canAnalyze(3, false)).toBe(true);
    expect(canAnalyze(0, false)).toBe(false);
    expect(canAnalyze(3, true)).toBe(false);
  });
});

describe("showSessionWarning", () => {
  it("warns only for a session going to a non-local endpoint", () => {
    expect(showSessionWarning("session", false)).toBe(true);
    expect(showSessionWarning("session", true)).toBe(false);
    expect(showSessionWarning("business", false)).toBe(false);
  });
});

describe("canSave", () => {
  it("true only with transcript rows", () => {
    expect(canSave(1)).toBe(true);
    expect(canSave(0)).toBe(false);
  });
});
