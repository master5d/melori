import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import {
  consultApi,
  type ChatSummary,
  type EngineClient,
  type EngineSession,
} from "../api";
import { parseRefs, resolveRef } from "./memoryLogic";

type Props = {
  client: EngineClient;
  sessions: EngineSession[];
  onOpenSession: (sid: string, tab: "transcript" | "notes" | "email") => void;
};

export function ChatTab({ client, sessions, onOpenSession }: Props) {
  const { t } = useTranslation();
  const [chats, setChats] = useState<ChatSummary[]>([]);
  const [current, setCurrent] = useState<number>();
  const [question, setQuestion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const selected = chats.find((chat) => chat.n === current) ?? chats[0];
  const council = client.consent.permissions.includes("council");
  useEffect(() => {
    void consultApi
      .chats(client.id)
      .then(async (items) => {
        setChats(items);
        if (items[0]) await loadChat(items[0].n);
      })
      .catch(() => setChats([]));
  }, [client.id]);
  const loadChat = async (n: number) => {
    try {
      const full = await consultApi.chat(client.id, n);
      setChats((items) => items.map((item) => (item.n === n ? full : item)));
      setCurrent(n);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const ask = async (event: FormEvent) => {
    event.preventDefault();
    if (!question.trim()) return;
    setSending(true);
    setError(null);
    try {
      const response = await consultApi.ask(client.id, question, selected?.n);
      const next = selected
        ? { ...selected, turns: [...selected.turns, response.turn] }
        : {
            n: response.chat ?? 0,
            created: response.turn.at,
            title: question.slice(0, 60),
            turns: [response.turn],
          };
      setChats((items) =>
        selected
          ? items.map((item) => (item.n === next.n ? next : item))
          : [...items, next],
      );
      setCurrent(next.n);
      setQuestion("");
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    } finally {
      setSending(false);
    }
  };
  if (!council)
    return (
      <p className="border border-rule p-5 text-sm text-secondary">
        {t("consult.memory.chatConsent")}
      </p>
    );
  return (
    <div className="grid gap-6 md:grid-cols-[180px_1fr]">
      <aside className="space-y-2">
        <button
          type="button"
          className="text-sm text-accent underline"
          onClick={() => setCurrent(undefined)}
        >
          {t("consult.memory.newChat")}
        </button>
        {chats.map((chat) => (
          <button
            type="button"
            key={chat.n}
            className={`block w-full truncate text-start text-sm ${selected?.n === chat.n ? "font-semibold text-primary" : "text-secondary"}`}
            onClick={() => void loadChat(chat.n)}
          >
            {chat.title}
          </button>
        ))}
      </aside>
      <section>
        <div className="min-h-[180px] space-y-4">
          {selected?.turns?.map((turn, index) => (
            <div key={`${turn.at}-${index}`}>
              <p className="m-0 text-xs text-secondary">{turn.q}</p>
              <p className="m-0 whitespace-pre-wrap pt-1 text-sm text-primary">
                {parseRefs(turn.a).map((part, partIndex) =>
                  "date" in part ? (
                    (() => {
                      const sid = resolveRef(part.date, sessions);
                      return (
                        <button
                          type="button"
                          key={partIndex}
                          className="text-accent underline"
                          onClick={() => sid && onOpenSession(sid, "notes")}
                        >
                          [{part.date}]
                        </button>
                      );
                    })()
                  ) : (
                    <span key={partIndex}>{part.text}</span>
                  ),
                )}
              </p>
            </div>
          ))}
        </div>
        <form
          className="mt-5 flex gap-3 border-t border-rule pt-4"
          onSubmit={(event) => void ask(event)}
        >
          <input
            className="min-w-0 flex-1 border-b border-rule-strong bg-transparent py-2 text-primary"
            value={question}
            onChange={(event) => setQuestion(event.target.value)}
            placeholder={t("consult.memory.chatPlaceholder")}
          />
          <button
            className="bg-accent px-4 py-2 text-sm text-on-accent"
            disabled={sending}
            type="submit"
          >
            {t("consult.memory.ask")}
          </button>
        </form>
        {error && (
          <p role="alert" className="mt-2 text-sm text-err">
            {error}
          </p>
        )}
      </section>
    </div>
  );
}
