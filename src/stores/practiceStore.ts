import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * Дневник ежедневной практики (интейк #136, B-23) — ОДИН на приложение,
 * поверх per-course прогресса (`courseStore`): курс помнит, ЧТО пройдено,
 * дневник — В КАКИЕ ДНИ занимались. Серия и «День N» вычисляются из
 * дневника чистыми функциями `practice/logic.ts`.
 */
export interface PracticeState {
  /** Курс, из которого строится план дня. */
  activeCourseId: string;
  /** YYYY-MM-DD → id упражнений, завершённых в этот день. */
  dailyLog: Record<string, string[]>;
  setActiveCourse: (courseId: string) => void;
  logDone: (date: string, exerciseId: string) => void;
}

export const usePracticeStore = create<PracticeState>()(
  persist(
    (set) => ({
      activeCourseId: "dictionTraining",
      dailyLog: {},

      setActiveCourse: (courseId) => set({ activeCourseId: courseId }),

      logDone: (date, exerciseId) =>
        set((state) => {
          const day = state.dailyLog[date] ?? [];
          if (day.includes(exerciseId)) return state;
          return {
            dailyLog: { ...state.dailyLog, [date]: [...day, exerciseId] },
          };
        }),
    }),
    { name: "echo-practice-store" },
  ),
);
