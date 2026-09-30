// Ежедневная практика (интейк #136, B-23): чистая логика плана дня и серии.
// Паттерн школы: «10–15 минут в день» = короткая сессия из нескольких упражнений
// подряд; серия (streak) мотивирует не рвать цепочку. Всё вычислимо из
// дневника занятий — никакого собственного часового состояния.
import type { Lesson, Exercise } from "@/courses/types";

/** Упражнений в одной дневной сессии (≈10–15 минут школьного формата). */
export const SESSION_SIZE = 3;

export interface PlannedExercise {
  lesson: Lesson;
  exercise: Exercise;
}

/** Локальная календарная дата YYYY-MM-DD (не UTC: занятие «сегодня» — по часам
 * пользователя; полночь UTC резала бы вечернюю сессию на два дня). */
export function localDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/**
 * План на сегодня: следующие N НЕзавершённых упражнений курса, в порядке уроков.
 * Пропуски дней не наказываются — план просто продолжает очередь; завершённый
 * курс даёт пустой план (UI показывает «курс пройден»).
 */
export function planFor(
  lessons: Lesson[],
  completedExercises: string[],
  n: number = SESSION_SIZE,
): PlannedExercise[] {
  const done = new Set(completedExercises);
  const out: PlannedExercise[] = [];
  for (const lesson of lessons) {
    for (const exercise of lesson.exercises) {
      if (done.has(exercise.id)) continue;
      out.push({ lesson, exercise });
      if (out.length >= n) return out;
    }
  }
  return out;
}

/**
 * Серия: сколько дней ПОДРЯД (заканчивая сегодня или вчера) есть занятия.
 * Сегодняшний день без занятия серию НЕ обнуляет — она «под угрозой», но жива
 * до конца дня; разрыв ≥1 полного дня обнуляет. Дни сравниваются строками
 * YYYY-MM-DD через шаг локального календаря.
 */
export function streakFrom(daysPracticed: string[], today: string): number {
  const days = new Set(daysPracticed);
  let cursor = today;
  if (!days.has(cursor)) {
    cursor = prevDate(cursor);
    if (!days.has(cursor)) return 0;
  }
  let streak = 0;
  while (days.has(cursor)) {
    streak += 1;
    cursor = prevDate(cursor);
  }
  return streak;
}

/** Номер «дня практики» = сколько дней с занятиями всего + 1, если сегодня ещё
 * пусто. Устойчив к пропускам: это счётчик занятий, не календарь от старта. */
export function dayNumber(daysPracticed: string[], today: string): number {
  const days = new Set(daysPracticed);
  return days.size + (days.has(today) ? 0 : 1);
}

function prevDate(ymd: string): string {
  const [y, m, d] = ymd.split("-").map(Number);
  const dt = new Date(y, m - 1, d);
  dt.setDate(dt.getDate() - 1);
  return localDate(dt);
}
