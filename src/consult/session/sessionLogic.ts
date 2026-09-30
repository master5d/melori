import type {
  NoteVersion,
  Provenance,
  SessionDetail,
  EmailDraft,
  SessionNote,
} from "../api";

type Translation = (key: string, options?: any) => string;

export function mailtoFor(
  subject: string,
  body: string,
  limit = 1800,
): { url: string; bodyInClipboard: boolean } {
  const encodedSubject = encodeURIComponent(subject);
  const encodedBody = encodeURIComponent(body);
  const full = `mailto:?subject=${encodedSubject}&body=${encodedBody}`;
  if (full.length > limit) {
    return {
      url: `mailto:?subject=${encodedSubject}`,
      bodyInClipboard: true,
    };
  }
  return { url: full, bodyInClipboard: false };
}

export function provenanceLine(p: Provenance, t: Translation): string {
  const location = t(
    p.endpoint_local
      ? "consult.session.provenance.local"
      : "consult.session.provenance.remote",
  );
  const chars = new Intl.NumberFormat().format(p.chars_sent);
  return `${p.model} · ${location} · ${t("consult.session.provenance.sentChars", { count: chars })} · ${p.language}`;
}

export function editorNoteAfterRefresh(
  _prevEditing: SessionNote | null,
  _prevCurrentN: number | null,
  nextCurrent: SessionNote | null,
  base: number | null,
): SessionNote | null {
  return base === null ? nextCurrent : _prevEditing;
}

export function speakerKey(source: string): "me" | "others" {
  return source === "me" ? "me" : "others";
}

export async function openEmailWithClipboard(
  mailto: { url: string; bodyInClipboard: boolean },
  body: string,
  deps: {
    writeText: (text: string) => Promise<void>;
    openUrl: (url: string) => Promise<void>;
    markEmailOpened: () => Promise<void>;
    onClipboardFailure: () => void;
  },
): Promise<void> {
  if (mailto.bodyInClipboard) {
    try {
      await deps.writeText(body);
    } catch {
      deps.onClipboardFailure();
    }
  }
  await deps.openUrl(mailto.url);
  await deps.markEmailOpened();
}

export function versionLabel(
  version: Pick<NoteVersion, "n" | "created" | "author">,
  t: Translation,
): string {
  const time = new Date(version.created).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  return `${t("consult.session.version", { n: version.n })} · ${t(`consult.session.authors.${version.author}`, version.author)} · ${time}`;
}

export function provenanceInput(
  p: Provenance,
  detail: SessionDetail,
  t: Translation,
): string {
  if (p.input === "transcript") {
    return t("consult.session.provenance.transcriptInput", {
      count: detail.transcript.length,
    });
  }
  return t("consult.session.provenance.noteInput", {
    n: detail.email?.from_note ?? detail.note?.n ?? "—",
  });
}

export function emailIsStale(
  email: EmailDraft | null,
  versions: NoteVersion[],
): boolean {
  if (!email) return false;
  return email.from_note === null
    ? versions.length > 0
    : versions.some((version) => version.n > email.from_note!);
}

export function parentFor(
  base: number | null,
  current: number | null,
): number | null {
  return base ?? current;
}

export function canDraftEmail(
  detail: SessionDetail,
  perms: string[],
): { ok: boolean; reason: "council" | "noNote" | null } {
  if (!perms.includes("council")) return { ok: false, reason: "council" };
  if (!detail.note) return { ok: false, reason: "noNote" };
  return { ok: true, reason: null };
}

export function saveBlockedReason(
  perms: string[],
  revoked: boolean,
): "retain" | "revoked" | null {
  if (revoked) return "revoked";
  if (!perms.includes("retain")) return "retain";
  return null;
}

export function regenerateNeedsConfirm(email: EmailDraft | null): boolean {
  return email?.author === "practitioner";
}

export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const seconds = total % 60;
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function markLabel(segment: {
  source: string;
  start_ms: number;
}): string {
  return `★ ${formatClock(segment.start_ms)}`;
}
