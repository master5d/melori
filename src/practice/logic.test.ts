import { describe, it, expect } from "vitest";
import { planFor, streakFrom, dayNumber, localDate } from "./logic";
import type { Lesson } from "@/courses/types";

const LESSONS: Lesson[] = [
  {
    id: 0,
    title: "Урок 0",
    desc: "",
    exercises: [
      { id: "a1", name: "A1", desc: "", instruction: "" },
      { id: "a2", name: "A2", desc: "", instruction: "" },
    ],
  },
  {
    id: 1,
    title: "Урок 1",
    desc: "",
    exercises: [
      { id: "b1", name: "B1", desc: "", instruction: "" },
      { id: "b2", name: "B2", desc: "", instruction: "" },
    ],
  },
];

describe("planFor", () => {
  it("берёт следующие N незавершённых в порядке уроков", () => {
    const plan = planFor(LESSONS, ["a1"], 2);
    expect(plan.map((p) => p.exercise.id)).toEqual(["a2", "b1"]);
    expect(plan[1].lesson.id).toBe(1);
  });

  it("пропуск завершённых не ломает переход через границу урока", () => {
    const plan = planFor(LESSONS, ["a1", "a2", "b1"], 3);
    expect(plan.map((p) => p.exercise.id)).toEqual(["b2"]);
  });

  it("завершённый курс даёт пустой план, а не бесконечность", () => {
    expect(planFor(LESSONS, ["a1", "a2", "b1", "b2"])).toEqual([]);
  });
});

describe("streakFrom", () => {
  it("подряд до сегодня включительно", () => {
    expect(
      streakFrom(["2026-08-22", "2026-08-23", "2026-08-24"], "2026-08-24"),
    ).toBe(3);
  });

  it("сегодня ещё пусто — серия жива от вчера (под угрозой, не обнулена)", () => {
    expect(streakFrom(["2026-08-22", "2026-08-23"], "2026-08-24")).toBe(2);
  });

  it("полный пропущенный день обнуляет", () => {
    expect(streakFrom(["2026-08-21", "2026-08-22"], "2026-08-24")).toBe(0);
  });

  it("переход через границу месяца не рвёт серию", () => {
    expect(streakFrom(["2026-07-31", "2026-08-01"], "2026-08-01")).toBe(2);
  });

  it("пустой дневник = 0", () => {
    expect(streakFrom([], "2026-08-24")).toBe(0);
  });
});

describe("dayNumber", () => {
  it("сегодня ещё не занимался — следующий день практики", () => {
    expect(dayNumber(["2026-08-20", "2026-08-22"], "2026-08-24")).toBe(3);
  });
  it("сегодня уже занимался — текущий день", () => {
    expect(dayNumber(["2026-08-23", "2026-08-24"], "2026-08-24")).toBe(2);
  });
});

describe("localDate", () => {
  it("однозначные месяц/день дополняются нулём", () => {
    expect(localDate(new Date(2026, 0, 5))).toBe("2026-01-05");
  });
});
