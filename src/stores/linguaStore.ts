import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * LINGVÆTICA (B-25): язык, ответы анкеты лингвопрофиля и текущая сессия
 * диалога. Диалог персистится тоже — «поле настройки» не должно забывать
 * нить между перезапусками; новая сессия начинается явно кнопкой.
 */
export interface LinguaMessage {
  role: "user" | "tutor";
  text: string;
}

export interface LinguaState {
  languageId: string;
  answers: Record<string, string[]>;
  profileDone: boolean;
  messages: LinguaMessage[];
  setLanguage: (id: string) => void;
  setAnswer: (questionId: string, values: string[]) => void;
  finishProfile: () => void;
  resetProfile: () => void;
  pushMessage: (m: LinguaMessage) => void;
  clearSession: () => void;
}

export const useLinguaStore = create<LinguaState>()(
  persist(
    (set) => ({
      languageId: "san",
      answers: {},
      profileDone: false,
      messages: [],

      setLanguage: (id) => set({ languageId: id }),
      setAnswer: (questionId, values) =>
        set((s) => ({ answers: { ...s.answers, [questionId]: values } })),
      finishProfile: () => set({ profileDone: true }),
      resetProfile: () =>
        set({ profileDone: false, answers: {}, messages: [] }),
      pushMessage: (m) => set((s) => ({ messages: [...s.messages, m] })),
      clearSession: () => set({ messages: [] }),
    }),
    { name: "echo-lingua-store" },
  ),
);
