import { describe, it, expect } from "vitest";
import {
  councilSituation,
  applyCouncilEvent,
  emptyCouncil,
  isLocalWellbeing,
  councilProgress,
  formatDuration,
  initialLensSelection,
} from "./councilView";
import type { MeetingAnalysis, CouncilFrame, ClientNote } from "@/bindings";

const soap: MeetingAnalysis = {
  kind: "soap",
  subjective: "s",
  objective: "o",
  assessment: "a",
  plan: "p",
};
const brief: MeetingAnalysis = {
  kind: "brief",
  summary: "x",
  key_points: [],
  action_items: [],
  open_questions: [],
};

describe("councilSituation", () => {
  it("uses section keys to find field text", () => {
    const note: ClientNote = {
      n: 1,
      template_id: "dap",
      sections: [
        ["data", "Данные"],
        ["assessment", "Оценка"],
        ["plan", "План"],
      ],
      fields: { data: "Факты", assessment: "Формулировка", plan: "Шаги" },
      stored: true,
    };
    expect(councilSituation(null, note)).toBe(
      "Данные:\nФакты\n\nОценка:\nФормулировка\n\nПлан:\nШаги",
    );
  });
  it("returns SOAP text for a soap analysis", () => {
    const s = councilSituation(soap);
    expect(s).toContain("SUBJECTIVE");
    expect(s).toContain("s");
  });
  it("returns null for a brief or no analysis", () => {
    expect(councilSituation(brief)).toBeNull();
    expect(councilSituation(null)).toBeNull();
  });
});

describe("applyCouncilEvent", () => {
  it("accumulates opinions in order, sets synthesis, and marks done", () => {
    const op: CouncilFrame = {
      kind: "opinion",
      opinion: {
        specialist_id: "cbt",
        name: "CBT",
        paradigm: "Cog",
        text: "t1",
      },
    };
    const op2: CouncilFrame = {
      kind: "opinion",
      opinion: {
        specialist_id: "gestalt",
        name: "Gestalt",
        paradigm: "G",
        text: "t2",
      },
    };
    const syn: CouncilFrame = {
      kind: "synthesis",
      synthesis: { text: "S", convergences: ["c"], divergences: ["d"] },
    };
    let st = emptyCouncil;
    st = applyCouncilEvent(st, op);
    st = applyCouncilEvent(st, op2);
    st = applyCouncilEvent(st, syn);
    st = applyCouncilEvent(st, { kind: "done" });
    expect(st.opinions.map((o) => o.specialist_id)).toEqual(["cbt", "gestalt"]);
    expect(st.synthesis?.text).toBe("S");
    expect(st.done).toBe(true);
    expect(st.error).toBeNull();
  });
  it("captures an error frame", () => {
    const st = applyCouncilEvent(emptyCouncil, {
      kind: "error",
      detail: "down",
    });
    expect(st.error).toBe("down");
    expect(st.done).toBe(true);
  });
});

describe("isLocalWellbeing", () => {
  it("true for loopback, false for cloud", () => {
    expect(isLocalWellbeing("http://127.0.0.1:8000")).toBe(true);
    expect(isLocalWellbeing("http://localhost:8000")).toBe(true);
    expect(isLocalWellbeing("https://wellbeing.example.com")).toBe(false);
  });
});

describe("council telemetry", () => {
  const plan: CouncilFrame = {
    kind: "plan",
    total: 3,
    concurrency: 2,
    specialists: ["cbt", "gestalt", "emdr"],
  };

  it("walks plan → lenses → synthesis → done", () => {
    let s = applyCouncilEvent(emptyCouncil, plan, 1000);
    expect(councilProgress(s, 1000)).toMatchObject({
      stage: "lenses",
      done: 0,
      total: 3,
      running: [],
    });
    s = applyCouncilEvent(
      s,
      { kind: "lens_start", specialist_id: "cbt", name: "CBT", index: 1 },
      1500,
    );
    s = applyCouncilEvent(
      s,
      {
        kind: "lens_start",
        specialist_id: "gestalt",
        name: "Gestalt",
        index: 2,
      },
      1600,
    );
    expect(councilProgress(s, 61000)).toMatchObject({
      running: ["CBT", "Gestalt"],
      elapsedMs: 60000,
    });
    s = applyCouncilEvent(
      s,
      { kind: "lens_done", specialist_id: "cbt", elapsed_s: 42.5 },
      44000,
    );
    expect(councilProgress(s, 44000)).toMatchObject({
      done: 1,
      running: ["Gestalt"],
    });
    s = applyCouncilEvent(
      s,
      { kind: "lens_done", specialist_id: "gestalt", elapsed_s: 50 },
      52000,
    );
    s = applyCouncilEvent(
      s,
      { kind: "lens_start", specialist_id: "emdr", name: "EMDR", index: 3 },
      52000,
    );
    s = applyCouncilEvent(
      s,
      { kind: "lens_done", specialist_id: "emdr", elapsed_s: 30 },
      82000,
    );
    s = applyCouncilEvent(s, { kind: "synthesis_start" }, 82000);
    expect(councilProgress(s, 90000).stage).toBe("synthesis");
    s = applyCouncilEvent(s, { kind: "done" }, 120000);
    expect(councilProgress(s, 120000)).toMatchObject({
      stage: "done",
      done: 3,
      total: 3,
    });
  });

  it("an error wins over every other stage", () => {
    const s = applyCouncilEvent(
      applyCouncilEvent(emptyCouncil, plan, 0),
      { kind: "error", detail: "503" },
      5,
    );
    expect(councilProgress(s, 5).stage).toBe("error");
  });

  it("formats elapsed time as m:ss", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(134_900)).toBe("2:14");
  });

  it("restores a stored lens selection, dropping lenses no longer offered", () => {
    const offered = ["cbt", "gestalt", "emdr"];
    expect([...initialLensSelection(offered, ["gestalt", "gone"])]).toEqual([
      "gestalt",
    ]);
    expect([...initialLensSelection(offered, null)]).toEqual(offered);
    expect([...initialLensSelection(offered, ["gone"])]).toEqual(offered);
  });
});
