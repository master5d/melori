import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { createCourseStore } from "./courseStore";

// Тесты гоняются в node-окружении (vitest.config.ts: environment "node"), где
// localStorage нет вовсе. Persist-мидлвар zustand это переживает — он лишь пишет
// в stderr `Unable to update item 'echo-…-store', the given storage is currently
// unavailable` и работает дальше. Тесты оставались зелёными, но проверяли стор
// БЕЗ персистентности: сериализация и восстановление прогресса курса не были
// покрыты ничем, и первым, кто сообщил бы о поломке, стал бы пользователь.
//
// Шим ставится точечно, на этот файл, а не глобальным setupFiles: общий
// localStorage поднял бы гидратацию у ВСЕХ персистентных сторов сразу и мог бы
// молча поменять поведение чужих тестов, которые сейчас зелёные.
function installMemoryStorage() {
  const map = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return map.size;
    },
    clear: () => map.clear(),
    getItem: (key) => (map.has(key) ? (map.get(key) as string) : null),
    key: (index) => Array.from(map.keys())[index] ?? null,
    removeItem: (key) => {
      map.delete(key);
    },
    setItem: (key, value) => {
      map.set(key, String(value));
    },
  };
  // Ставится ИМЕННО на `window`, и это не мелочь: zustand берёт хранилище как
  // `createJSONStorage(() => window.localStorage)` (node_modules/zustand/esm/
  // middleware.mjs:332), а не как голый `localStorage`. Первая версия этого шима
  // клала объект в globalThis.localStorage — `typeof localStorage` показывал
  // "object", тесты были зелёные, и persist всё это время писал в никуда,
  // печатая ту же жалобу «storage is currently unavailable». Поймала только
  // проверка на непустой ключ ниже; сам факт наличия шима не доказывал НИЧЕГО.
  Object.defineProperty(globalThis, "window", {
    value: { ...(globalThis.window ?? {}), localStorage: storage },
    configurable: true,
    writable: true,
  });
  Object.defineProperty(globalThis, "localStorage", {
    value: storage,
    configurable: true,
    writable: true,
  });
}

beforeEach(() => {
  installMemoryStorage();
});

afterEach(() => {
  Reflect.deleteProperty(globalThis, "localStorage");
  Reflect.deleteProperty(globalThis, "window");
});

describe("createCourseStore", () => {
  it("tracks completion and reports per course", () => {
    const useStore = createCourseStore("testCourse");
    const s = useStore.getState();
    s.completeExercise(1, "ex1");
    s.completeLesson(1);
    s.addReport({
      id: "r1",
      date: "d",
      lessonId: 1,
      exerciseId: "ex1",
      reportText: "",
      transcription: "",
      feedback: "fb",
    });
    const after = useStore.getState();
    expect(after.completedExercises).toContain("ex1");
    expect(after.completedLessons).toContain(1);
    expect(after.reports[0].feedback).toBe("fb");
    after.resetProgress();
    expect(useStore.getState().reports).toHaveLength(0);
  });

  it("dedupes repeated completion", () => {
    const useStore = createCourseStore("testCourse2");
    const s = useStore.getState();
    s.completeExercise(1, "ex1");
    s.completeExercise(1, "ex1");
    s.completeLesson(2);
    s.completeLesson(2);
    const after = useStore.getState();
    expect(after.completedExercises).toEqual(["ex1"]);
    expect(after.completedLessons).toEqual([2]);
  });

  it("персистит прогресс и поднимает его в НОВОМ сторе того же курса", () => {
    const first = createCourseStore("roundTrip");
    first.getState().completeLesson(3);
    first.getState().completeExercise(3, "ex-a");
    first.getState().addReport({
      id: "r1",
      date: "d",
      lessonId: 3,
      exerciseId: "ex-a",
      reportText: "",
      transcription: "",
      feedback: "fb",
    });

    // Ключ проверяется явно: он часть контракта с уже установленными копиями
    // приложения — переименование сбросило бы прогресс у всех молча.
    const raw = localStorage.getItem("echo-roundTrip-store");
    expect(raw).toBeTruthy();

    const reborn = createCourseStore("roundTrip");
    const state = reborn.getState();
    expect(state.completedLessons).toEqual([3]);
    expect(state.completedExercises).toEqual(["ex-a"]);
    expect(state.reports).toHaveLength(1);
    expect(state.reports[0].feedback).toBe("fb");
  });

  it("сброс прогресса переживает пересоздание стора", () => {
    // Отдельным тестом, потому что «забыть записать очистку» — отдельная ошибка
    // от «забыть записать добавление», и первая тише: экран выглядит чистым до
    // перезапуска, а потом прогресс воскресает.
    const first = createCourseStore("resetRoundTrip");
    first.getState().completeLesson(1);
    first.getState().resetProgress();

    const reborn = createCourseStore("resetRoundTrip");
    expect(reborn.getState().completedLessons).toEqual([]);
    expect(reborn.getState().reports).toEqual([]);
  });

  it("курсы не видят прогресс друг друга", () => {
    const a = createCourseStore("courseA");
    const b = createCourseStore("courseB");
    a.getState().completeLesson(7);

    expect(createCourseStore("courseB").getState().completedLessons).toEqual(
      [],
    );
    expect(b.getState().completedLessons).toEqual([]);
    expect(createCourseStore("courseA").getState().completedLessons).toEqual([
      7,
    ]);
  });
});
