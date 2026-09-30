// Анкета лингвопрофиля (B-25, интейк #139) — структура Игоря Галкина,
// адаптированная с латино-профиля на сакральные языки лаборатории.
// Тексты — по-русски в данных (прецедент: LESSONS курсов так и живут).

export interface ProfileQuestion {
  id: string;
  question: string;
  options: string[];
  multi?: boolean;
}

export const LINGUA_LANGUAGES: { id: string; label: string }[] = [
  { id: "chu", label: "Церковнославянский" },
  { id: "san", label: "Санскрит" },
  { id: "arc", label: "Арамейский" },
  { id: "pli", label: "Пали" },
  { id: "pan", label: "Гурмукхи (панджаби)" },
];

export const PROFILE_QUESTIONS: ProfileQuestion[] = [
  {
    id: "pull",
    question: "Что тебя цепляет сильнее?",
    options: [
      "мантры и звучание",
      "тексты и писания",
      "философия и смысл",
      "ритуал и практика",
      "письменность и каллиграфия",
    ],
    multi: true,
  },
  {
    id: "why",
    question: "Зачем тебе этот язык сейчас?",
    options: [
      "духовная практика",
      "читать первоисточники",
      "петь и произносить верно",
      "исследовательский интерес",
      "внутренний зов",
    ],
    multi: true,
  },
  {
    id: "perception",
    question: "Как тебе легче воспринимать?",
    options: [
      "на слух",
      "короткие фрагменты текста",
      "визуальные образы",
      "диалог",
      "через движение и ритм",
    ],
    multi: true,
  },
  {
    id: "tires",
    question: "Что быстро утомляет?",
    options: [
      "длинные тексты",
      "правила",
      "однообразие",
      "много слов сразу",
      "отсутствие практики",
    ],
    multi: true,
  },
  {
    id: "attention",
    question: "Сколько внимания держится комфортно?",
    options: ["3–5 мин", "10–15 мин", "20+ мин"],
  },
  {
    id: "learnBy",
    question: "Тебе проще учиться через:",
    options: [
      "ритм и распев",
      "ассоциации",
      "повторение",
      "разговор",
      "действие",
    ],
    multi: true,
  },
  {
    id: "mistakes",
    question: "Как ты реагируешь на ошибки?",
    options: [
      "спокойно",
      "теряю мотивацию",
      "нужен мягкий фидбек",
      "люблю сразу исправлять",
    ],
  },
  {
    id: "priority",
    question: "Что важнее сейчас?",
    options: [
      "произносить и петь",
      "понимать на слух",
      "читать письменность",
      "чувствовать традицию",
    ],
  },
  {
    id: "format",
    question: "Какой формат удержит интерес?",
    options: [
      "мини-задания",
      "разбор мантр и строк",
      "карточки",
      "сцены и истории",
      "живой диалог",
    ],
    multi: true,
  },
  {
    id: "rhythm",
    question: "Выбери свой ритм:",
    options: [
      "suave · мягко",
      "fuego · интенсивно",
      "libre · свободно",
      "ritual · регулярно",
    ],
  },
  {
    id: "experience",
    question: "Опыт с этим языком?",
    options: [
      "ноль",
      "слышал в практике",
      "могу читать письменность",
      "понимаю отдельные фразы",
    ],
  },
  {
    id: "keeps",
    question: "Что помогает не бросать?",
    options: [
      "поддержка",
      "интересные темы",
      "быстрые победы",
      "структура",
      "свобода выбора",
    ],
    multi: true,
  },
];

/** Профиль → текст для системного промпта тьютора. */
export function profileToPrompt(
  languageLabel: string,
  answers: Record<string, string[]>,
): string {
  const lines = PROFILE_QUESTIONS.filter((q) => answers[q.id]?.length).map(
    (q) => `- ${q.question} → ${answers[q.id].join(", ")}`,
  );
  return [`Изучает: ${languageLabel}.`, ...lines].join("\n");
}
