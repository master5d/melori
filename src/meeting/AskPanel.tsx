import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type AskView } from "@/bindings";
import { askChipLine, visibleAsks } from "./meetingTools";

export function AskPanel({
  asks,
  onAsk,
  focusRef,
}: {
  asks: AskView[];
  onAsk: (question: string, kind: string) => Promise<void>;
  focusRef: React.MutableRefObject<(() => void) | null>;
}) {
  const { t } = useTranslation();
  const [question, setQuestion] = useState("");
  const [expanded, setExpanded] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  focusRef.current = () => inputRef.current?.focus();
  const visible = visibleAsks(asks, expanded);
  const quick = ["recap5", "missed", "agreed"];
  return (
    <section
      className="mc-ask-panel"
      aria-label={t("meetingCopilot.ask.title")}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (question.trim()) {
            void onAsk(question.trim(), "free");
            setQuestion("");
          }
        }}
      >
        <input
          ref={inputRef}
          value={question}
          onChange={(event) => setQuestion(event.target.value)}
          placeholder={t("meetingCopilot.ask.placeholder")}
        />
        <button className="mc-btn mc-btn-primary" type="submit">
          {t("meetingCopilot.ask.send")}
        </button>
      </form>
      <div className="mc-ask-chips">
        {quick.map((kind) => (
          <button
            className="mc-btn"
            key={kind}
            onClick={() => void onAsk("", kind)}
          >
            {t(`meetingCopilot.ask.${kind}`)}
          </button>
        ))}
      </div>
      <div className="mc-ask-list">
        {visible.shown.map((ask) => (
          <article key={`${ask.at}-${ask.elapsed_ms}`}>
            <div className="mc-ask-question">{ask.question}</div>
            <p>{ask.answer}</p>
            <small>{askChipLine(ask.provenance, t)}</small>
          </article>
        ))}
      </div>
      {visible.hidden > 0 && (
        <button
          className="mc-btn mc-btn-ghost"
          onClick={() => setExpanded(true)}
        >
          {t("meetingCopilot.ask.more", { count: visible.hidden })}
        </button>
      )}
    </section>
  );
}

export async function askMeeting(
  question: string,
  kind: string,
): Promise<AskView | null> {
  const result = await commands.askMeeting(question, kind);
  return result.status === "ok" ? result.data : null;
}
