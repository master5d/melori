import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { CourseReport } from "@/courses/types";

export interface CourseState {
  completedLessons: number[];
  completedExercises: string[];
  reports: CourseReport[];
  completeExercise: (lessonId: number, exerciseId: string) => void;
  completeLesson: (lessonId: number) => void;
  addReport: (report: CourseReport) => void;
  resetProgress: () => void;
}

/**
 * Factory for a per-course progress store. Each course gets its own persisted
 * zustand store keyed by `echo-<courseId>-store`. Shape matches the 11 original
 * per-course stores that this replaces.
 */
export function createCourseStore(courseId: string) {
  return create<CourseState>()(
    persist(
      (set) => ({
        completedLessons: [],
        completedExercises: [],
        reports: [],

        completeExercise: (_lessonId, exerciseId) =>
          set((state) =>
            state.completedExercises.includes(exerciseId)
              ? state
              : {
                  completedExercises: [...state.completedExercises, exerciseId],
                },
          ),

        completeLesson: (lessonId) =>
          set((state) =>
            state.completedLessons.includes(lessonId)
              ? state
              : { completedLessons: [...state.completedLessons, lessonId] },
          ),

        addReport: (report) =>
          set((state) => ({ reports: [report, ...state.reports] })),

        resetProgress: () =>
          set({ completedLessons: [], completedExercises: [], reports: [] }),
      }),
      { name: `echo-${courseId}-store` },
    ),
  );
}
