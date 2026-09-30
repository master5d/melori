import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Sparkles, Send, RotateCcw, Settings2 } from "lucide-react";
import { useLinguaStore } from "@/stores/linguaStore";
import {
  LINGUA_LANGUAGES,
  PROFILE_QUESTIONS,
  profileToPrompt,
} from "./profile";

/**
 * LINGVÆTICA (B-25, интейк #139): диалоговый тьютор сакральных языков с
 * персоной «поле языковой настройки». Анкета лингвопрофиля (структура Галкина)
 * проходится один раз и питает системный промпт каждого хода.
 */
export function LinguaTutor() {
  const { t } = useTranslation();
  const s = useLinguaStore();
  if (!s.profileDone) return <ProfileWizard />;
  return <TutorChat />;
}

function ProfileWizard() {
  const { t } = useTranslation();
  const { languageId, answers, setLanguage, setAnswer, finishProfile } =
    useLinguaStore();
  const [step, setStep] = useState(0); // 0 = язык, дальше вопросы
  const total = PROFILE_QUESTIONS.length + 1;
  const q = step > 0 ? PROFILE_QUESTIONS[step - 1] : null;

  const toggle = (qid: string, opt: string, multi: boolean | undefined) => {
    const cur = answers[qid] ?? [];
    if (multi) {
      setAnswer(
        qid,
        cur.includes(opt) ? cur.filter((x) => x !== opt) : [...cur, opt],
      );
    } else {
      setAnswer(qid, [opt]);
    }
  };

  const canNext = step === 0 || (q && (answers[q.id]?.length ?? 0) > 0);

  return (
    <div className="w-full max-w-2xl mx-auto flex flex-col gap-5 animate-in fade-in duration-500">
      <div className="rounded-3xl border border-surface-raised bg-surface/30 p-6">
        <div className="flex items-center gap-3">
          <Sparkles className="w-6 h-6 text-accent" />
          <div>
            <h2 className="text-xl font-black text-white">
              {t("settings.lingua.wizardTitle")}
            </h2>
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
              {step + 1} / {total}
            </span>
          </div>
        </div>
      </div>

      <div className="rounded-2xl border border-surface-raised bg-surface/10 p-6 flex flex-col gap-4">
        {step === 0 ? (
          <>
            <h3 className="text-lg font-black text-white">
              {t("settings.lingua.pickLanguage")}
            </h3>
            <div className="flex flex-col gap-2">
              {LINGUA_LANGUAGES.map((l) => (
                <button
                  key={l.id}
                  onClick={() => setLanguage(l.id)}
                  className={`text-left px-4 py-3 rounded-xl border text-sm font-bold transition-all ${
                    languageId === l.id
                      ? "bg-accent text-white border-accent"
                      : "bg-ground/60 text-white border-surface-raised hover:bg-surface/40"
                  }`}
                >
                  {l.label}
                </button>
              ))}
            </div>
          </>
        ) : (
          q && (
            <>
              <h3 className="text-lg font-black text-white">{q.question}</h3>
              <div className="flex flex-col gap-2">
                {q.options.map((opt) => {
                  const on = (answers[q.id] ?? []).includes(opt);
                  return (
                    <button
                      key={opt}
                      onClick={() => toggle(q.id, opt, q.multi)}
                      className={`text-left px-4 py-3 rounded-xl border text-sm transition-all ${
                        on
                          ? "bg-accent text-white border-accent font-bold"
                          : "bg-ground/60 text-white border-surface-raised hover:bg-surface/40"
                      }`}
                    >
                      {opt}
                    </button>
                  );
                })}
              </div>
              {q.multi && (
                <p className="text-[10px] text-secondary">
                  {t("settings.lingua.multiHint")}
                </p>
              )}
            </>
          )
        )}

        <div className="flex justify-between mt-2">
          <button
            onClick={() => setStep((n) => Math.max(0, n - 1))}
            disabled={step === 0}
            className="px-4 py-2 text-xs font-bold rounded-xl bg-surface-raised text-white disabled:opacity-40"
          >
            {t("settings.lingua.back")}
          </button>
          {step < total - 1 ? (
            <button
              onClick={() => setStep((n) => n + 1)}
              disabled={!canNext}
              className="px-4 py-2 text-xs font-bold rounded-xl bg-accent text-white disabled:opacity-40"
            >
              {t("settings.lingua.next")}
            </button>
          ) : (
            <button
              onClick={finishProfile}
              disabled={!canNext}
              className="px-4 py-2 text-xs font-bold rounded-xl bg-ok/20 border border-ok/30 text-ok disabled:opacity-40"
            >
              {t("settings.lingua.finish")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function TutorChat() {
  const { t } = useTranslation();
  const {
    languageId,
    answers,
    messages,
    pushMessage,
    clearSession,
    resetProfile,
  } = useLinguaStore();
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const lang =
    LINGUA_LANGUAGES.find((l) => l.id === languageId) ?? LINGUA_LANGUAGES[0];

  const send = async () => {
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    setError(null);
    pushMessage({ role: "user", text });
    setBusy(true);
    try {
      const transcript = messages
        .map((m) => `${m.role === "user" ? "Ученик" : "Тьютор"}: ${m.text}`)
        .join("\n");
      const reply = await invoke<string>("lingua_chat", {
        languageId: lang.id,
        profile: profileToPrompt(lang.label, answers),
        transcript,
        userMessage: text,
      });
      pushMessage({ role: "tutor", text: reply });
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="w-full max-w-3xl mx-auto flex flex-col gap-4 animate-in fade-in duration-500">
      <div className="rounded-3xl border border-surface-raised bg-surface/30 p-5 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Sparkles className="w-5 h-5 text-accent" />
          <div>
            {/* eslint-disable-next-line i18next/no-literal-string -- product name */}
            <h2 className="text-lg font-black text-white">LINGVÆTICA</h2>
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
              {lang.label}
            </span>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={clearSession}
            title={t("settings.lingua.newSession")}
            className="p-2 rounded-xl bg-ground/60 border border-surface-raised text-secondary hover:text-white"
          >
            <RotateCcw className="w-4 h-4" />
          </button>
          <button
            onClick={resetProfile}
            title={t("settings.lingua.redoProfile")}
            className="p-2 rounded-xl bg-ground/60 border border-surface-raised text-secondary hover:text-white"
          >
            <Settings2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      <div className="flex flex-col gap-3 min-h-[200px]">
        {messages.length === 0 && (
          <p className="text-sm text-secondary text-center py-8">
            {t("settings.lingua.emptyHint")}
          </p>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className={`rounded-2xl px-4 py-3 text-sm whitespace-pre-wrap max-w-[85%] ${
              m.role === "user"
                ? "self-end bg-accent/20 border border-accent/30 text-white"
                : "self-start bg-surface/20 border border-surface-raised text-white"
            }`}
          >
            {m.text}
          </div>
        ))}
        {busy && (
          <div className="self-start text-xs text-secondary animate-pulse px-2">
            {t("settings.lingua.thinking")}
          </div>
        )}
        {error && (
          <div className="self-start text-xs text-err px-2 whitespace-pre-wrap">
            {error}
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      <div className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          placeholder={t("settings.lingua.inputPlaceholder")}
          className="flex-1 bg-ground/60 border border-surface-raised rounded-xl px-4 py-3 text-sm text-white placeholder:text-secondary focus:outline-none focus:border-accent"
        />
        <button
          onClick={send}
          disabled={busy || !input.trim()}
          className="px-4 rounded-xl bg-accent text-white disabled:opacity-40"
        >
          <Send className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
