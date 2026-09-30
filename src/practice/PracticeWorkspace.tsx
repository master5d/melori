import { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Flame, Mic, Square, Check, ChevronRight, Trophy } from "lucide-react";
import { COURSES, getCourse } from "@/courses/registry";
import { createCourseStore } from "@/stores/courseStore";
import { usePracticeStore } from "@/stores/practiceStore";
import {
  planFor,
  streakFrom,
  dayNumber,
  localDate,
  SESSION_SIZE,
} from "./logic";
import { BeforeAfterPanel } from "./BeforeAfterPanel";

/**
 * Ежедневная практика (интейк #136, B-23): петля «упражнение → запись →
 * готово → следующее» поверх существующих курсов, с серией (streak) и
 * замерами «до/после». Прогресс упражнений пишется в per-course store —
 * тот же, что видят страницы курсов; дневник дней — в practiceStore.
 */
export function PracticeWorkspace() {
  const { t } = useTranslation();
  const { activeCourseId, dailyLog, setActiveCourse, logDone } =
    usePracticeStore();
  const course = getCourse(activeCourseId) ?? COURSES[0];
  const useCourse = useMemo(() => createCourseStore(course.id), [course.id]);
  const { completedExercises, completeExercise } = useCourse();

  const [tab, setTab] = useState<"session" | "beforeAfter">("session");
  const [doneThisSession, setDoneThisSession] = useState(0);

  const today = localDate(new Date());
  const practicedDays = Object.keys(dailyLog);
  const streak = streakFrom(practicedDays, today);
  const day = dayNumber(practicedDays, today);
  const plan = planFor(course.lessons, completedExercises, SESSION_SIZE);
  const current = plan[0];
  const sessionOver = doneThisSession >= SESSION_SIZE || !current;

  // --- запись текущего упражнения (локальная, для самопрослушивания) --------
  const [isRecording, setIsRecording] = useState(false);
  const [takeUrl, setTakeUrl] = useState<string | null>(null);
  const recRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);

  const startRec = async () => {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const rec = new MediaRecorder(stream);
    recRef.current = rec;
    chunksRef.current = [];
    rec.ondataavailable = (e) => chunksRef.current.push(e.data);
    rec.onstop = () => {
      const blob = new Blob(chunksRef.current, { type: "audio/webm" });
      setTakeUrl(URL.createObjectURL(blob));
      stream.getTracks().forEach((tr) => tr.stop());
    };
    rec.start();
    setIsRecording(true);
  };
  const stopRec = () => {
    recRef.current?.stop();
    setIsRecording(false);
  };

  const markDone = () => {
    if (!current) return;
    completeExercise(current.lesson.id, current.exercise.id);
    logDone(today, current.exercise.id);
    setDoneThisSession((n) => n + 1);
    setTakeUrl(null);
  };

  return (
    <div className="w-full max-w-4xl mx-auto flex flex-col gap-6 animate-in fade-in duration-500">
      {/* Шапка: день, серия, курс */}
      <div className="relative overflow-hidden rounded-3xl border border-surface-raised bg-surface/30 backdrop-blur-2xl p-6 flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 shadow-2xl">
        <div className="flex items-center gap-3.5 z-10">
          <div className="p-3 rounded-2xl bg-gradient-to-tr from-accent/20 to-accent/40 border border-accent/20 text-accent">
            <Flame className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-xl sm:text-2xl font-black text-white tracking-tight leading-none">
              {t("settings.practice.title")}
            </h2>
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider block mt-1.5">
              {t("settings.practice.day", { day })} ·{" "}
              {t("settings.practice.streak", { count: streak })}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-3 z-10 w-full sm:w-auto">
          <select
            value={course.id}
            onChange={(e) => setActiveCourse(e.target.value)}
            className="bg-ground/60 border border-surface-raised rounded-xl px-3 py-2 text-xs font-bold text-white"
          >
            {COURSES.map((c) => (
              <option key={c.id} value={c.id}>
                {t(`sidebar.${c.sidebarKey}`)}
              </option>
            ))}
          </select>
          <div className="flex bg-ground/60 p-1.5 rounded-2xl border border-surface-raised/80">
            {(["session", "beforeAfter"] as const).map((id) => {
              // Невыбранный таб лежит на тёмном bg-ground, не на цветной
              // поверхности — ветки разнесены, чтобы same-line эвристика
              // no-gray-on-color не читала их как «серое на цветном».
              const active = "bg-accent text-white shadow-md";
              const idle = "text-secondary hover:text-white";
              return (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  className={`px-4 py-2 text-xs font-bold rounded-xl transition-all ${tab === id ? active : idle}`}
                >
                  {t(`settings.practice.tab.${id}`)}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {tab === "beforeAfter" && <BeforeAfterPanel />}

      {tab === "session" && (
        <div className="flex flex-col gap-4">
          {/* Прогресс сессии */}
          <div className="flex items-center gap-2">
            {Array.from({ length: SESSION_SIZE }).map((_, i) => (
              <div
                key={i}
                className={`h-1.5 flex-1 rounded-full ${
                  i < doneThisSession ? "bg-ok" : "bg-surface-raised"
                }`}
              />
            ))}
            <span className="text-xs text-secondary font-bold ml-2">
              {Math.min(doneThisSession, SESSION_SIZE)}/{SESSION_SIZE}
            </span>
          </div>

          {sessionOver ? (
            <div className="rounded-2xl border border-ok/20 bg-ok/10 p-8 flex flex-col items-center gap-3 text-center">
              <Trophy className="w-8 h-8 text-ok" />
              <p className="text-lg font-black text-white">
                {current
                  ? t("settings.practice.sessionDone")
                  : t("settings.practice.courseDone")}
              </p>
              <p className="text-xs text-secondary">
                {current
                  ? t("settings.practice.sessionDoneHint", { count: streak })
                  : t("settings.practice.courseDoneHint")}
              </p>
              {current && (
                <button
                  onClick={() => setDoneThisSession(0)}
                  className="mt-2 px-4 py-2 text-xs font-bold rounded-xl bg-surface-raised text-white hover:bg-accent transition-all"
                >
                  {t("settings.practice.oneMore")}
                </button>
              )}
            </div>
          ) : (
            <div className="rounded-2xl border border-surface-raised bg-surface/10 p-6 flex flex-col gap-4">
              <div>
                <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
                  {current.lesson.title}
                </span>
                <h3 className="text-lg font-black text-white mt-1">
                  {current.exercise.name}
                </h3>
                <p className="text-sm text-secondary mt-2">
                  {current.exercise.desc}
                </p>
              </div>
              <div className="rounded-xl bg-ground/60 border border-surface-raised p-4 text-sm text-white">
                {current.exercise.instruction}
              </div>

              <div className="flex items-center gap-3 flex-wrap">
                {!isRecording ? (
                  <button
                    onClick={startRec}
                    className="flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-xl bg-accent text-white hover:bg-accent-hot transition-all"
                  >
                    <Mic className="w-4 h-4" /> {t("settings.practice.record")}
                  </button>
                ) : (
                  <button
                    onClick={stopRec}
                    className="flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-xl bg-err text-white transition-all"
                  >
                    <Square className="w-4 h-4" /> {t("settings.practice.stop")}
                  </button>
                )}
                {takeUrl && <audio controls src={takeUrl} className="h-9" />}
                <div className="flex-1" />
                <button
                  onClick={markDone}
                  className="flex items-center gap-2 px-4 py-2 text-xs font-bold rounded-xl bg-ok/20 border border-ok/30 text-ok hover:bg-ok/30 transition-all"
                >
                  <Check className="w-4 h-4" /> {t("settings.practice.done")}
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
