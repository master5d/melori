import { describe, expect, it } from "vitest";
import {
  canDraftEmail,
  emailIsStale,
  formatClock,
  markLabel,
  mailtoFor,
  parentFor,
  provenanceInput,
  provenanceLine,
  regenerateNeedsConfirm,
  saveBlockedReason,
  editorNoteAfterRefresh,
  openEmailWithClipboard,
  speakerKey,
  versionLabel,
} from "./sessionLogic";

const email = (
  from_note: number | null,
  author: "model" | "practitioner" = "model",
) => ({
  subject: "s",
  body: "b",
  author,
  from_note,
  created: "",
  provenance: null,
  opened_in_mail_at: null,
});
const v = (n: number) => ({
  n,
  created: "",
  author: "model",
  template_id: "dap",
  parent: null,
  provenance: null,
});

describe("mailtoFor", () => {
  it("encodes subject and body", () => {
    expect(mailtoFor("Тема", "Текст & ещё")).toEqual({
      url:
        "mailto:?subject=" +
        encodeURIComponent("Тема") +
        "&body=" +
        encodeURIComponent("Текст & ещё"),
      bodyInClipboard: false,
    });
  });
  it("moves a long cyrillic body to the clipboard", () => {
    const r = mailtoFor("Тема", "я".repeat(400));
    expect(r.bodyInClipboard).toBe(true);
    expect(r.url).toBe("mailto:?subject=" + encodeURIComponent("Тема"));
  });
  it("opens mail even when copying the long body fails", async () => {
    const opened: string[] = [];
    await openEmailWithClipboard(
      { url: "mailto:?subject=s", bodyInClipboard: true },
      "body",
      {
        writeText: async () => {
          throw new Error("Document is not focused");
        },
        openUrl: async (url) => {
          opened.push(url);
        },
        markEmailOpened: async () => {
          opened.push("marked");
        },
        onClipboardFailure: () => {
          opened.push("clipboard-failed");
        },
      },
    );
    expect(opened).toEqual(["clipboard-failed", "mailto:?subject=s", "marked"]);
  });
});

describe("session display helpers", () => {
  const note = (n: number) => ({ n }) as never;
  it("switches to a refreshed current note unless a base version is open", () => {
    expect(editorNoteAfterRefresh(note(1), 1, note(3), null)).toEqual(note(3));
    expect(editorNoteAfterRefresh(note(1), 1, note(3), 1)).toEqual(note(1));
  });
  it("maps transcript speakers", () => {
    expect(speakerKey("me")).toBe("me");
    expect(speakerKey("others")).toBe("others");
  });
  it("labels a version with author and local time", () => {
    const t = (key: string, options?: { n?: number }) =>
      key === "consult.session.version" ? `v${options?.n}` : key;
    expect(
      versionLabel(
        { n: 3, author: "model", created: "2026-09-29T14:02:00" },
        t,
      ),
    ).toContain("v3 · consult.session.authors.model · 14:02");
  });
});
describe("emailIsStale", () => {
  it("is stale when the note moved past from_note", () =>
    expect(emailIsStale(email(2), [v(1), v(2), v(4)])).toBe(true));
  it("is fresh on the latest version", () =>
    expect(emailIsStale(email(2), [v(1), v(2)])).toBe(false));
  it("no email is never stale", () =>
    expect(emailIsStale(null, [v(1)])).toBe(false));
});
describe("rules", () => {
  it("parent is the chosen base, else current", () => {
    expect(parentFor(2, 5)).toBe(2);
    expect(parentFor(null, 5)).toBe(5);
  });
  it("regenerating over a practitioner edit asks first", () => {
    expect(regenerateNeedsConfirm(email(1, "practitioner"))).toBe(true);
    expect(regenerateNeedsConfirm(email(1))).toBe(false);
  });
  it("save is blocked without retain or after revoke", () => {
    expect(saveBlockedReason(["transcript", "council"], false)).toBe("retain");
    expect(saveBlockedReason(["retain"], true)).toBe("revoked");
    expect(saveBlockedReason(["retain"], false)).toBeNull();
  });
  it("email needs council and a note", () => {
    const d = (note: unknown) =>
      ({
        note,
        transcript: [],
        versions: [],
        asks: [],
        runs: [],
        email: null,
        session: {},
      }) as never;
    expect(canDraftEmail(d(null), ["council"])).toEqual({
      ok: false,
      reason: "noNote",
    });
    expect(canDraftEmail(d({}), ["retain"])).toEqual({
      ok: false,
      reason: "council",
    });
    expect(canDraftEmail(d({}), ["council"])).toEqual({
      ok: true,
      reason: null,
    });
  });
  it("formats a clock", () => {
    expect(formatClock(65_000)).toBe("01:05");
    expect(formatClock(3_723_000)).toBe("1:02:03");
  });
  it("labels a mark with a star and meeting time", () => {
    expect(markLabel({ source: "mark", start_ms: 760_000 })).toBe("★ 12:40");
  });
});

describe("provenance", () => {
  const t = (key: string, options?: unknown) =>
    key + JSON.stringify(options ?? {});
  const p = {
    model: "local-floor",
    endpoint_local: true,
    input: "transcript" as const,
    chars_sent: 1234,
    language: "ru",
    attempts: 1,
  };
  it("formats local and remote provenance with translated keys", () => {
    expect(provenanceLine(p, t)).toContain("consult.session.provenance.local");
    expect(provenanceLine({ ...p, endpoint_local: false }, t)).toContain(
      "consult.session.provenance.remote",
    );
  });
  it("describes transcript and note inputs", () => {
    const detail = {
      transcript: [{ i: 1 }, { i: 2 }],
      email: { from_note: 3 },
    } as never;
    expect(provenanceInput(p, detail, t)).toContain("transcript");
    expect(provenanceInput({ ...p, input: "note" }, detail, t)).toContain("3");
  });
});
