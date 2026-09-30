import type { AskProvenance } from "@/bindings";

type Translation = (key: string, options?: Record<string, unknown>) => string;

export function visibleAsks<T>(asks: T[], expanded: boolean, keep = 2) {
  const shown = expanded ? asks : asks.slice(0, keep);
  return { shown, hidden: Math.max(0, asks.length - shown.length) };
}

export function countdownState(
  startedAt: number,
  now: number,
  lastVoiceAt: number | null,
  total = 60_000,
) {
  const cancelled = lastVoiceAt !== null && lastVoiceAt > startedAt;
  const remaining = Math.max(0, total - Math.max(0, now - startedAt));
  return { remaining, cancelled, expired: !cancelled && remaining === 0 };
}

function clock(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

export function askChipLine(p: AskProvenance, t: Translation): string {
  const location = t(
    p.endpoint_local
      ? "consult.session.provenance.local"
      : "consult.session.provenance.remote",
  );
  const suffix = p.truncated
    ? ` · ${t("consult.session.provenance.truncated")}`
    : "";
  return `${t("consult.session.askUntil", { time: clock(p.until_ms) })} · ${t("consult.session.provenance.sentChars", { count: new Intl.NumberFormat().format(p.chars_sent) })} · ${p.model} · ${location}${suffix}`;
}

export function diagnosisWarning(phrase: string, t: Translation): string {
  return t("consult.session.diagnosisWarning", { phrase });
}
