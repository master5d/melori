import React, { useState, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  BookOpen,
  Calendar,
  Mic,
  Play,
  Square,
  Sparkles,
  CheckCircle2,
  ChevronRight,
  User,
  ArrowLeft,
} from "lucide-react";
import { getCourse } from "@/courses/registry";
import { createCourseStore } from "@/stores/courseStore";
import type { CourseReport, Lesson, Exercise } from "@/courses/types";
import { toast } from "sonner";

/**
 * Generic, data-driven course UI. Replaces the 11 near-identical per-course
 * Settings components. Content comes from the course registry; progress from a
 * per-course store; coach feedback from the generic `course_feedback` command.
 */
export const CourseSettings: React.FC<{ courseId: string }> = ({
  courseId,
}) => {
  const { t } = useTranslation();
  const course = useMemo(() => getCourse(courseId), [courseId]);
  const useStore = useMemo(() => createCourseStore(courseId), [courseId]);

  const {
    completedLessons,
    completedExercises,
    reports,
    completeExercise,
    completeLesson,
    addReport,
  } = useStore();

  const [activeTab, setActiveTab] = useState<"lessons" | "reports">("lessons");
  const [selectedLesson, setSelectedLesson] = useState<Lesson | null>(null);
  const [selectedExercise, setSelectedExercise] = useState<Exercise | null>(
    null,
  );

  const [isRecording, setIsRecording] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [reportText, setReportText] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [coachResponse, setCoachResponse] = useState<string | null>(null);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioChunksRef = useRef<Blob[]>([]);

  if (!course) return null;
  const LESSONS = course.lessons;
  const Icon = course.icon;

  const startRecording = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mediaRecorder = new MediaRecorder(stream);
      mediaRecorderRef.current = mediaRecorder;
      audioChunksRef.current = [];
      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) audioChunksRef.current.push(event.data);
      };
      mediaRecorder.onstop = () => {
        const audioBlob = new Blob(audioChunksRef.current, {
          type: "audio/wav",
        });
        setAudioUrl(URL.createObjectURL(audioBlob));
      };
      mediaRecorder.start();
      setIsRecording(true);
      setAudioUrl(null);
      toast.success(t("settings.course.toastRecStart"));
    } catch (e) {
      console.error(e);
      toast.error(t("settings.course.toastMicFail"));
    }
  };

  const stopRecording = () => {
    if (mediaRecorderRef.current && isRecording) {
      mediaRecorderRef.current.stop();
      mediaRecorderRef.current.stream
        .getTracks()
        .forEach((track) => track.stop());
      setIsRecording(false);
      toast.success(t("settings.course.toastRecStop"));
    }
  };

  const submitReport = async () => {
    if (!selectedLesson || !selectedExercise) return;
    if (!reportText.trim()) {
      toast.error(t("settings.course.toastFillReport"));
      return;
    }
    setIsSubmitting(true);
    setCoachResponse(null);
    try {
      const response = await invoke<string>("course_feedback", {
        courseId,
        lessonId: selectedLesson.id,
        exerciseId: selectedExercise.id,
        userNotes: reportText,
        transcription: audioUrl ? "Practice Audio recorded" : "No audio",
      });

      const newReport: CourseReport = {
        id: Date.now().toString(),
        date: new Date().toLocaleDateString("ru-RU", {
          day: "2-digit",
          month: "short",
          hour: "2-digit",
          minute: "2-digit",
        }),
        lessonId: selectedLesson.id,
        exerciseId: selectedExercise.id,
        reportText,
        transcription: audioUrl ? "audio" : "none",
        feedback: response,
      };

      addReport(newReport);
      completeExercise(selectedLesson.id, selectedExercise.id);

      const allExerciseIds = selectedLesson.exercises.map((e) => e.id);
      const completedOnes = [...completedExercises, selectedExercise.id];
      if (allExerciseIds.every((id) => completedOnes.includes(id))) {
        completeLesson(selectedLesson.id);
        toast.success(
          t("settings.course.toastLessonDone", { title: selectedLesson.title }),
        );
      }

      setCoachResponse(response);
      toast.success(t("settings.course.toastSubmitted"));
    } catch (e) {
      console.error(e);
      toast.error(t("settings.course.toastCoachFail"));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleBackToLesson = () => {
    setSelectedExercise(null);
    setAudioUrl(null);
    setReportText("");
    setCoachResponse(null);
  };
  const handleBackToLessonsList = () => setSelectedLesson(null);

  return (
    <div className="w-full max-w-4xl mx-auto flex flex-col gap-6 animate-in fade-in duration-500">
      {/* Header */}
      <div className="relative overflow-hidden rounded-3xl border border-surface-raised bg-surface/30 backdrop-blur-2xl p-6 sm:p-8 flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 shadow-2xl">
        <div className="absolute top-1/2 left-1/4 -translate-x-1/2 -translate-y-1/2 w-80 h-80 rounded-full bg-accent/5 blur-[100px] pointer-events-none" />
        <div className="absolute bottom-0 right-0 w-60 h-60 rounded-full bg-accent/5 blur-[80px] pointer-events-none" />
        <div className="flex items-center gap-3.5 z-10">
          <div className="p-3 rounded-2xl bg-gradient-to-tr from-accent/20 to-accent/40 border border-accent/20 text-accent">
            <Icon className="w-6 h-6 animate-pulse" />
          </div>
          <div>
            <h2 className="text-xl sm:text-2xl font-black text-white tracking-tight leading-none">
              {t(`sidebar.${course.sidebarKey}`)}
            </h2>
            <span className="text-[10px] text-secondary font-bold uppercase tracking-wider block mt-1.5">
              {t("settings.course.subtitle")}
            </span>
          </div>
        </div>
        <div className="flex bg-ground/60 p-1.5 rounded-2xl border border-surface-raised/80 z-10 w-full sm:w-auto">
          <button
            onClick={() => {
              setActiveTab("lessons");
              handleBackToLesson();
              handleBackToLessonsList();
            }}
            className={`flex-1 sm:flex-none px-4 py-2 text-xs font-bold rounded-xl transition-all ${
              activeTab === "lessons"
                ? "bg-accent text-white shadow-md"
                : "text-secondary hover:text-white"
            }`}
          >
            {t("settings.course.tabCourse")}
          </button>
          <button
            onClick={() => setActiveTab("reports")}
            className={`flex-1 sm:flex-none px-4 py-2 text-xs font-bold rounded-xl transition-all ${
              activeTab === "reports"
                ? "bg-accent text-white shadow-md"
                : "text-secondary hover:text-white"
            }`}
          >
            {t("settings.course.tabDiary")}
          </button>
        </div>
      </div>

      <div className="w-full">
        {activeTab === "lessons" && (
          <div className="w-full">
            {!selectedLesson ? (
              <div className="flex flex-col gap-4">
                {LESSONS.map((lesson) => {
                  const allExCompleted = lesson.exercises.every((e) =>
                    completedExercises.includes(e.id),
                  );
                  const isLessonFinished =
                    completedLessons.includes(lesson.id) || allExCompleted;
                  return (
                    <div
                      key={lesson.id}
                      onClick={() => setSelectedLesson(lesson)}
                      className="group relative overflow-hidden rounded-2xl border border-surface-raised bg-surface/10 hover:bg-surface/30 cursor-pointer p-5 transition-all flex items-center justify-between"
                    >
                      <div className="flex items-center gap-4">
                        <div
                          className={`p-3 rounded-xl border ${
                            isLessonFinished
                              ? "bg-ok/10 border-ok/20 text-ok"
                              : "bg-surface-raised/50 border-surface-raised text-secondary"
                          }`}
                        >
                          {isLessonFinished ? (
                            <CheckCircle2 className="w-5 h-5" />
                          ) : (
                            <BookOpen className="w-5 h-5" />
                          )}
                        </div>
                        <div>
                          <div className="flex items-center gap-2">
                            <span className="text-[10px] text-accent font-extrabold uppercase tracking-wider">
                              {t("settings.course.lesson", { n: lesson.id })}
                            </span>
                            {isLessonFinished && (
                              <span className="text-[9px] bg-ok/15 text-ok px-1.5 py-0.5 rounded-md font-bold">
                                {t("settings.course.completed")}
                              </span>
                            )}
                          </div>
                          <h3 className="text-sm font-bold text-white mt-1 group-hover:text-accent transition-colors">
                            {lesson.title}
                          </h3>
                          <p className="text-xs text-secondary mt-1 max-w-xl">
                            {lesson.desc}
                          </p>
                        </div>
                      </div>
                      <ChevronRight className="w-5 h-5 text-secondary group-hover:text-accent transition-colors shrink-0" />
                    </div>
                  );
                })}
              </div>
            ) : !selectedExercise ? (
              <div className="flex flex-col gap-4">
                <button
                  onClick={handleBackToLessonsList}
                  className="flex items-center gap-2 text-xs font-bold text-secondary hover:text-white mb-2 self-start"
                >
                  <ArrowLeft className="w-4 h-4" />{" "}
                  {t("settings.course.backToLessons")}
                </button>
                <div className="p-5 rounded-2xl border border-surface-raised bg-ground/20 mb-2">
                  <span className="text-[10px] text-accent font-extrabold uppercase tracking-wider">
                    {t("settings.course.lesson", { n: selectedLesson.id })}
                  </span>
                  <h3 className="text-lg font-bold text-white mt-1">
                    {selectedLesson.title}
                  </h3>
                  <p className="text-xs text-secondary mt-2 leading-relaxed">
                    {selectedLesson.desc}
                  </p>
                </div>
                <h4 className="text-xs font-bold text-secondary uppercase tracking-widest ml-1 mt-2">
                  {t("settings.course.exercisesHeading")}
                </h4>
                <div className="flex flex-col gap-3">
                  {selectedLesson.exercises.map((exercise) => {
                    const isCompleted = completedExercises.includes(
                      exercise.id,
                    );
                    return (
                      <div
                        key={exercise.id}
                        onClick={() => setSelectedExercise(exercise)}
                        className="group rounded-2xl border border-surface-raised bg-surface/15 hover:bg-surface/30 cursor-pointer p-4 flex items-center justify-between transition-all"
                      >
                        <div className="flex items-start gap-3">
                          <div
                            className={`p-2 rounded-lg mt-0.5 ${
                              isCompleted
                                ? "bg-ok/10 text-ok"
                                : "bg-surface-raised text-secondary"
                            }`}
                          >
                            {isCompleted ? (
                              <CheckCircle2 className="w-4 h-4" />
                            ) : (
                              <Play className="w-4 h-4" />
                            )}
                          </div>
                          <div>
                            <h5 className="text-xs font-bold text-white group-hover:text-accent transition-colors">
                              {exercise.name}
                            </h5>
                            <p className="text-xs text-secondary mt-1 max-w-lg leading-relaxed">
                              {exercise.desc}
                            </p>
                          </div>
                        </div>
                        <ChevronRight className="w-4 h-4 text-secondary group-hover:text-accent transition-colors shrink-0" />
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-6">
                <button
                  onClick={handleBackToLesson}
                  className="flex items-center gap-2 text-xs font-bold text-secondary hover:text-white mb-1 self-start"
                >
                  <ArrowLeft className="w-4 h-4" />{" "}
                  {t("settings.course.backToExercises")}
                </button>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                  <div className="md:col-span-1 flex flex-col gap-4 p-5 rounded-2xl border border-surface-raised bg-ground/20">
                    <span className="text-[10px] text-accent font-extrabold uppercase tracking-widest">
                      {t("settings.course.instructionLabel")}
                    </span>
                    <h4 className="text-sm font-black text-white">
                      {selectedExercise.name}
                    </h4>
                    <div className="text-xs text-secondary leading-relaxed space-y-3">
                      <p>{selectedExercise.desc}</p>
                      <div className="p-3 bg-accent/5 rounded-xl border border-accent/10 text-primary">
                        <strong className="text-white block mb-1">
                          {t("settings.course.howTo")}
                        </strong>
                        {selectedExercise.instruction}
                      </div>
                    </div>
                  </div>
                  <div className="md:col-span-2 flex flex-col gap-5 p-5 rounded-2xl border border-surface-raised bg-surface/10">
                    <h3 className="text-sm font-bold text-white uppercase tracking-wider ml-1">
                      {t("settings.course.practiceHeading")}
                    </h3>
                    <div className="flex items-center justify-between p-4 bg-ground/40 rounded-xl border border-surface-raised/80">
                      <div className="flex items-center gap-3">
                        <div
                          className={`p-2.5 rounded-xl border ${
                            isRecording
                              ? "bg-err/15 border-err/30 text-err animate-pulse"
                              : "bg-surface-raised border-edge text-primary"
                          }`}
                        >
                          <Mic className="w-4 h-4" />
                        </div>
                        <div>
                          <span className="text-xs font-bold text-white block">
                            {t("settings.course.recordTitle")}
                          </span>
                          <span className="text-[10px] text-secondary block">
                            {isRecording
                              ? t("settings.course.recording")
                              : t("settings.course.recordHint")}
                          </span>
                        </div>
                      </div>
                      <div className="flex gap-2">
                        {isRecording ? (
                          <button
                            onClick={stopRecording}
                            className="bg-err hover:bg-err/80 text-white p-2 rounded-xl border border-err/20"
                          >
                            <Square className="w-4 h-4" />
                          </button>
                        ) : (
                          <button
                            onClick={startRecording}
                            className="bg-accent hover:bg-accent text-white p-2 rounded-xl"
                          >
                            <Mic className="w-4 h-4" />
                          </button>
                        )}
                      </div>
                    </div>
                    {audioUrl && (
                      <div className="p-3 bg-ground/20 rounded-xl border border-surface-raised/60 flex items-center justify-between">
                        <span className="text-xs text-secondary font-bold ml-1">
                          {t("settings.course.listen")}
                        </span>
                        <audio
                          src={audioUrl}
                          controls
                          className="h-8 max-w-[200px]"
                        />
                      </div>
                    )}
                    <div className="flex flex-col gap-2">
                      <label className="text-[10px] font-bold text-secondary uppercase tracking-widest ml-1">
                        {t("settings.course.reportLabel")}
                      </label>
                      <textarea
                        value={reportText}
                        onChange={(e) => setReportText(e.target.value)}
                        placeholder={t("settings.course.reportPlaceholder")}
                        rows={4}
                        className="text-xs bg-ground/60 border border-surface-raised rounded-xl p-3 text-primary focus:outline-none focus:ring-2 focus:ring-accent resize-none leading-relaxed"
                      />
                    </div>
                    <button
                      onClick={submitReport}
                      disabled={isSubmitting || !reportText.trim()}
                      className="w-full font-bold bg-accent hover:bg-accent disabled:opacity-50 text-white py-3 rounded-xl shadow-lg shadow-accent/10 flex items-center justify-center gap-2 cursor-pointer transition-colors"
                    >
                      {isSubmitting ? (
                        <>{t("settings.course.submitting")}</>
                      ) : (
                        <>
                          <Sparkles className="w-4 h-4" />{" "}
                          {t("settings.course.submit")}
                        </>
                      )}
                    </button>
                    {coachResponse && (
                      <div className="flex flex-col gap-3 p-4 bg-accent/5 rounded-xl border border-accent/15 animate-in fade-in slide-in-from-bottom-2">
                        <div className="flex items-center gap-2 text-accent">
                          <Sparkles className="w-4 h-4 animate-pulse" />
                          <span className="text-xs font-black uppercase tracking-wider">
                            {t("settings.course.recommendations")}
                          </span>
                        </div>
                        <p className="text-xs text-primary leading-relaxed whitespace-pre-line">
                          {coachResponse}
                        </p>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {activeTab === "reports" && (
          <div className="flex flex-col gap-4 animate-in fade-in duration-300">
            {reports.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-16 text-center border border-surface-raised/80 rounded-2xl bg-ground/15 p-6">
                <Calendar className="w-8 h-8 text-secondary mb-3" />
                <h4 className="text-sm font-bold text-white">
                  {t("settings.course.diaryEmptyTitle")}
                </h4>
                <p className="text-xs text-secondary mt-1 max-w-sm leading-relaxed">
                  {t("settings.course.diaryEmptyBody")}
                </p>
              </div>
            ) : (
              <div className="flex flex-col gap-4">
                {reports.map((report) => {
                  const lesson = LESSONS.find((l) => l.id === report.lessonId);
                  const exercise = lesson?.exercises.find(
                    (e) => e.id === report.exerciseId,
                  );
                  return (
                    <div
                      key={report.id}
                      className="rounded-2xl border border-surface-raised bg-surface/10 p-5 flex flex-col gap-4"
                    >
                      <div className="flex justify-between items-center border-b border-surface-raised/50 pb-3">
                        <div>
                          <span className="text-[10px] text-accent font-extrabold uppercase tracking-wider block">
                            {t("settings.course.lessonReport", {
                              n: report.lessonId,
                              name: exercise?.name || t("settings.course.task"),
                            })}
                          </span>
                          <span className="text-[9px] text-secondary font-bold block mt-0.5">
                            {report.date}
                          </span>
                        </div>
                        <span className="text-[9px] bg-accent/15 text-accent font-bold px-2 py-0.5 rounded">
                          {t("settings.course.reviewed")}
                        </span>
                      </div>
                      <div className="flex items-start gap-2.5 bg-ground/20 p-3.5 rounded-xl border border-surface-raised/30">
                        <div className="p-1.5 rounded-lg bg-surface-raised text-secondary mt-0.5">
                          <User className="w-3.5 h-3.5" />
                        </div>
                        <div>
                          <strong className="text-[10px] text-secondary font-bold block uppercase tracking-wider">
                            {t("settings.course.myReport")}
                          </strong>
                          <p className="text-xs text-primary leading-relaxed mt-1">
                            {report.reportText}
                          </p>
                        </div>
                      </div>
                      <div className="flex items-start gap-2.5 bg-accent/5 p-3.5 rounded-xl border border-accent/10">
                        <div className="p-1.5 rounded-lg bg-accent/15 text-accent mt-0.5">
                          <Sparkles className="w-3.5 h-3.5" />
                        </div>
                        <div>
                          <strong className="text-[10px] text-accent font-black block uppercase tracking-wider">
                            {t("settings.course.analysis")}
                          </strong>
                          <p className="text-xs text-primary leading-relaxed whitespace-pre-line mt-1">
                            {report.feedback}
                          </p>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
