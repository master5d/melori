import type { LucideIcon } from "lucide-react";
import type { Lesson } from "./types";
import dictionTraining from "./dictionTraining";
import menVoice from "./menVoice";
import womenVoice from "./womenVoice";
import voiceTraining from "./voiceTraining";
import onlineTraining from "./onlineTraining";
import oratoryTraining from "./oratoryTraining";
import speechImprov from "./speechImprov";
import toastTraining from "./toastTraining";
import readingTraining from "./readingTraining";
import rhetoricTraining from "./rhetoricTraining";
import cameraTraining from "./cameraTraining";

export interface CourseData {
  /** Canonical course id — matches the sidebar section key and the backend course_feedback id. */
  id: string;
  /** i18n sidebar label key suffix: `sidebar.<sidebarKey>`. */
  sidebarKey: string;
  icon: LucideIcon;
  lessons: Lesson[];
}

export const COURSES: CourseData[] = [
  dictionTraining,
  menVoice,
  womenVoice,
  voiceTraining,
  onlineTraining,
  oratoryTraining,
  speechImprov,
  toastTraining,
  readingTraining,
  rhetoricTraining,
  cameraTraining,
];

export function getCourse(id: string): CourseData | undefined {
  return COURSES.find((c) => c.id === id);
}
