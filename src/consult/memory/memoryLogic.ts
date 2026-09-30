import type { RecapData, RecapResponse } from "../api";

export type MemoryTab = "transcript" | "notes" | "email";

export interface SearchIndexState {
  sessions: number;
  chunks: number;
  vectors_missing: number;
  meaning_available: boolean;
  meaning_error: string | null;
}

export function highlightParts(
  snippet: string,
): { text: string; hit: boolean }[] {
  const parts: { text: string; hit: boolean }[] = [];
  const pattern = /\[\[(.*?)\]\]/gs;
  let cursor = 0;
  for (const match of snippet.matchAll(pattern)) {
    const start = match.index ?? 0;
    if (start > cursor)
      parts.push({ text: snippet.slice(cursor, start), hit: false });
    parts.push({ text: match[1], hit: true });
    cursor = start + match[0].length;
  }
  if (cursor < snippet.length)
    parts.push({ text: snippet.slice(cursor), hit: false });
  return parts.length ? parts : [{ text: snippet, hit: false }];
}

export function parseRefs(
  answer: string,
): ({ text: string } | { date: string })[] {
  const result: ({ text: string } | { date: string })[] = [];
  const pattern = /\[(\d{4}-\d{2}-\d{2})\]/g;
  let cursor = 0;
  for (const match of answer.matchAll(pattern)) {
    const start = match.index ?? 0;
    if (start > cursor) result.push({ text: answer.slice(cursor, start) });
    result.push({ date: match[1] });
    cursor = start + match[0].length;
  }
  if (cursor < answer.length) result.push({ text: answer.slice(cursor) });
  return result.length ? result : [{ text: answer }];
}

export function resolveRef(
  date: string,
  sessions: { id: string; date: string }[],
): string | null {
  return (
    sessions
      .filter((session) => session.date === date)
      .sort((a, b) => a.id.localeCompare(b.id))[0]?.id ?? null
  );
}

export function indexLine(
  index: SearchIndexState | null,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (!index) return "";
  if (index.meaning_error)
    return t("consult.memory.meaningUnavailable", {
      reason: index.meaning_error,
    });
  if (index.vectors_missing > 0 && !index.meaning_available) {
    return t("consult.memory.indexing", {
      current: index.chunks - index.vectors_missing,
      total: index.chunks,
    });
  }
  return "";
}

export function daysAgoLine(
  days: number,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  return days === 0
    ? t("consult.memory.today")
    : t("consult.memory.daysAgo", { count: days });
}

export function tabForHit(kind: string): MemoryTab {
  if (kind === "transcript") return "transcript";
  if (kind === "email") return "email";
  return "notes";
}

/** POST …/recap answers `{recap, stored, model_stale}`; the block reads the saved recap from `model`. */
export function withGeneratedRecap(
  current: RecapResponse | null,
  generated: { recap: RecapData; model_stale: boolean },
): RecapResponse {
  return {
    quick: current?.quick ?? null,
    model: generated.recap,
    model_stale: generated.model_stale,
  };
}
