export interface Exercise {
  id: string;
  name: string;
  desc: string;
  instruction: string;
}

export interface Lesson {
  id: number;
  title: string;
  desc: string;
  exercises: Exercise[];
}

export interface CourseReport {
  id: string;
  date: string;
  lessonId: number;
  exerciseId: string;
  reportText: string;
  transcription: string;
  feedback: string;
}
