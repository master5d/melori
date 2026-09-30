import { describe, it, expect } from "vitest";
import { COURSES, getCourse } from "./registry";

const IDS = [
  "dictionTraining",
  "menVoice",
  "womenVoice",
  "voiceTraining",
  "onlineTraining",
  "oratoryTraining",
  "speechImprov",
  "toastTraining",
  "readingTraining",
  "rhetoricTraining",
  "cameraTraining",
];

describe("course registry", () => {
  it("has all 11 courses with non-empty lessons + exercises", () => {
    expect(COURSES.map((c) => c.id).sort()).toEqual([...IDS].sort());
    for (const id of IDS) {
      const c = getCourse(id)!;
      expect(c, `course ${id} missing`).toBeTruthy();
      expect(c.lessons.length, `course ${id} has no lessons`).toBeGreaterThan(
        0,
      );
      expect(
        c.lessons[0].exercises.length,
        `course ${id} lesson 0 has no exercises`,
      ).toBeGreaterThan(0);
      expect(c.sidebarKey).toBe(id);
    }
  });

  it("returns undefined for unknown course", () => {
    expect(getCourse("nope")).toBeUndefined();
  });
});
