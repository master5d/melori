import { useState } from "react";
import { useTranslation } from "react-i18next";
import { consultApi, type EngineClient, type SearchResponse } from "../api";
import { highlightParts, indexLine, tabForHit } from "./memoryLogic";

export function SearchTab({
  client,
  onOpenSession,
}: {
  client: EngineClient;
  onOpenSession: (sid: string, tab: "transcript" | "notes" | "email") => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const council = client.consent.permissions.includes("council");
  // spec §4: "Both" by default; without council only words are possible
  const [mode, setMode] = useState<"words" | "meaning" | "both">(
    council ? "both" : "words",
  );
  const [data, setData] = useState<SearchResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!query.trim()) return;
    setError(null);
    try {
      setData(await consultApi.search(client.id, query, mode));
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  return (
    <div className="space-y-5">
      <form
        className="flex flex-wrap gap-3"
        onSubmit={(event) => void run(event)}
      >
        <input
          className="min-w-[240px] flex-1 border-b border-rule-strong bg-transparent px-1 py-2 text-primary"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("consult.memory.searchPlaceholder")}
        />
        <select
          className="border border-rule bg-transparent px-3 text-primary"
          value={mode}
          onChange={(event) => setMode(event.target.value as typeof mode)}
        >
          {(["words", "meaning", "both"] as const).map((item) => (
            <option
              key={item}
              value={item}
              disabled={item !== "words" && !council}
            >
              {t(`consult.memory.mode.${item}`)}
            </option>
          ))}
        </select>
        <button
          className="bg-accent px-4 py-2 text-sm text-on-accent"
          type="submit"
        >
          {t("consult.memory.search")}
        </button>
      </form>
      {!council && (
        <p className="text-xs text-secondary">
          {t("consult.memory.wordsOnly")}
        </p>
      )}
      {data && (
        <p className="text-xs text-secondary">{indexLine(data.index, t)}</p>
      )}
      {error && (
        <p role="alert" className="text-sm text-err">
          {error}
        </p>
      )}
      <div className="divide-y divide-rule">
        {data?.results.map((result) => (
          <button
            type="button"
            key={result.chunk_id}
            className="block w-full py-4 text-start hover:bg-surface"
            onClick={() =>
              onOpenSession(result.session_id, tabForHit(result.kind))
            }
          >
            <div className="font-mono text-xs text-secondary">
              {result.session_id.slice(0, 10)} ·{" "}
              {result.label || t(`consult.memory.kind.${result.kind}`)}
            </div>
            <p className="m-0 whitespace-pre-wrap pt-1 text-sm text-primary">
              {highlightParts(result.snippet).map((part, index) =>
                part.hit ? (
                  <mark key={index} className="bg-warn-surface">
                    {part.text}
                  </mark>
                ) : (
                  <span key={index}>{part.text}</span>
                ),
              )}
            </p>
          </button>
        ))}
      </div>
    </div>
  );
}
