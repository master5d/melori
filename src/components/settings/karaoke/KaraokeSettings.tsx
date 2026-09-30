import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Play,
  Pause,
  Mic,
  Square,
  Upload,
  Music,
  Award,
  Check,
  X,
  Volume2,
  Sparkles,
} from "lucide-react";
import { commands, type ScoreReport } from "@/bindings";
import { useSingAlong } from "./useSingAlong";
import PitchRibbon from "./PitchRibbon";
import { analyzeReferenceContour, contourHzAt } from "./referencePitch";

interface Track {
  id: string;
  name: string;
  lyrics: string;
  duration: number; // in seconds
  words: { word: string; start: number; end: number }[];
}

export const KaraokeSettings: React.FC = () => {
  const { t } = useTranslation();
  // Track state — no built-in catalog; the only source is an imported track.
  const [track, setTrack] = useState<Track | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [playbackSpeed, setPlaybackSpeed] = useState(1.0);

  // Imported audio + lyrics states
  const [customAudioUrl, setCustomAudioUrl] = useState<string | null>(null);
  const [customLyrics, setCustomLyrics] = useState("");

  // Recording states
  const [isRecording, setIsRecording] = useState(false);
  const [recordDuration, setRecordDuration] = useState(0);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [scoreReport, setScoreReport] = useState<ScoreReport | null>(null);

  // Audio node references
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const recordTimerRef = useRef<number | null>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioChunksRef = useRef<Blob[]>([]);

  // Sing-along states/refs
  const refContourRef = useRef<{ t: number; hz: number }[] | null>(null);
  const singAlong = useSingAlong();
  const prefersReducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  useEffect(() => {
    return () => {
      if (recordTimerRef.current) clearInterval(recordTimerRef.current);
    };
  }, []);

  const handlePlayPause = () => {
    if (!track || !customAudioUrl || !audioRef.current) return;
    if (isPlaying) {
      audioRef.current.pause();
      setIsPlaying(false);
    } else {
      setScoreReport(null);
      if (currentTime >= (audioRef.current.duration || track.duration)) {
        setCurrentTime(0);
      }
      audioRef.current.playbackRate = playbackSpeed;
      audioRef.current
        .play()
        .catch((err) => console.error("Audio playback failed", err));
      setIsPlaying(true);
    }
  };

  const handleStop = () => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current.currentTime = 0;
    }
    setIsPlaying(false);
    setCurrentTime(0);
  };

  // Custom File Import Function
  const handleImportAudio = async () => {
    handleStop();
    try {
      await commands.showMainWindowCommand(); // ensure tauri focus
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");

      const file = await openDialog({
        multiple: false,
        filters: [
          { name: "Audio", extensions: ["mp3", "wav", "m4a", "ogg", "webm"] },
        ],
      });

      if (typeof file === "string") {
        // Grant asset-protocol read access to this specific picked file (scope is
        // otherwise empty); must run before convertFileSrc or playback is blocked.
        await commands.allowAssetFile(file);
        // Convert file path to Tauri asset URL or read it
        const { convertFileSrc } = await import("@tauri-apps/api/core");
        const assetUrl = convertFileSrc(file);
        setCustomAudioUrl(assetUrl);

        // Best-effort reference-contour extraction for sing-along pitch
        // targets on the imported audio; failure just disables the live
        // pitch target (no crash).
        let ac: AudioContext | null = null;
        try {
          const resp = await fetch(assetUrl);
          const arr = await resp.arrayBuffer();
          ac = new AudioContext();
          const audioBuf = await ac.decodeAudioData(arr);
          refContourRef.current = analyzeReferenceContour(
            audioBuf.getChannelData(0),
            audioBuf.sampleRate,
          );
        } catch {
          refContourRef.current = null;
        } finally {
          await ac?.close().catch(() => {});
        }

        // Create an initial track config; duration/words fill in once audio
        // metadata loads and lyrics are entered.
        const importedTrack: Track = {
          id: "imported",
          name: file.split(/[\\/]/).pop() || "Imported Track",
          lyrics: customLyrics,
          duration: 10.0, // default, updated when audio metadata loads
          words: [],
        };
        setTrack(importedTrack);
      }
    } catch (e) {
      console.error("Failed to import audio file:", e);
    }
  };

  // Handle custom audio element events
  const handleAudioMetadataLoaded = () => {
    if (audioRef.current && track) {
      const dur = audioRef.current.duration;
      setTrack((prev) =>
        prev
          ? {
              ...prev,
              duration: dur,
              words: generateEvenTimestamps(customLyrics || prev.lyrics, dur),
            }
          : prev,
      );
    }
  };

  const handleAudioTimeUpdate = () => {
    if (audioRef.current && isPlaying) {
      setCurrentTime(audioRef.current.currentTime);
    }
  };

  const handleAudioEnded = () => {
    setIsPlaying(false);
    setCurrentTime(0);
  };

  // Generate evenly spaced timestamps for user-defined lyrics
  const generateEvenTimestamps = (text: string, duration: number) => {
    const rawWords = text.trim().split(/\s+/).filter(Boolean);
    if (rawWords.length === 0) return [];

    const wordDur = (duration - 0.8) / rawWords.length;
    return rawWords.map((word, index) => {
      const start = 0.4 + index * wordDur;
      return {
        word,
        start,
        end: start + wordDur - 0.05,
      };
    });
  };

  const handleLyricsChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setCustomLyrics(val);
    setTrack((prev) =>
      prev
        ? { ...prev, words: generateEvenTimestamps(val, prev.duration) }
        : prev,
    );
  };

  // Recording functionality
  const startRecording = async () => {
    handleStop();
    setScoreReport(null);
    audioChunksRef.current = [];

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mediaRecorder = new MediaRecorder(stream, {
        mimeType: "audio/webm",
      });
      mediaRecorderRef.current = mediaRecorder;

      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          audioChunksRef.current.push(event.data);
        }
      };

      mediaRecorder.onstop = async () => {
        const audioBlob = new Blob(audioChunksRef.current, {
          type: "audio/webm",
        });
        await processRecording(audioBlob);

        // Stop all audio tracks to release microphone
        stream.getTracks().forEach((track) => track.stop());
      };

      setIsRecording(true);
      setRecordDuration(0);
      mediaRecorder.start();

      recordTimerRef.current = window.setInterval(() => {
        setRecordDuration((prev) => prev + 1);
      }, 1000) as unknown as number;
    } catch (err) {
      console.error("Failed to access microphone for recording:", err);
    }
  };

  const stopRecording = () => {
    if (mediaRecorderRef.current && isRecording) {
      mediaRecorderRef.current.stop();
      setIsRecording(false);
      if (recordTimerRef.current) {
        clearInterval(recordTimerRef.current);
        recordTimerRef.current = null;
      }
    }
  };

  // Convert blob to file and score using backend commands
  const processRecording = async (blob: Blob) => {
    setIsTranscribing(true);
    try {
      const appDirResult = await commands.getAppDirPath();
      if (appDirResult.status !== "ok")
        throw new Error("Could not fetch app directory");

      const appDir = appDirResult.data;
      const tempPath = `${appDir}/temp_karaoke_take.webm`;

      // Read blob as Uint8Array
      const arrayBuffer = await blob.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);

      // Write using tauri-plugin-fs
      const { writeFile } = await import("@tauri-apps/plugin-fs");
      await writeFile(tempPath, bytes);

      // Transcribe file using Whisper model on backend
      const transcriptResult = await commands.transcribeFileToString(
        tempPath,
        null, // auto detect language
        null, // default model
        false, // no diarization
        null, // speaker hint
        "plain", // text output
        null, // no translation
      );

      if (transcriptResult.status === "ok") {
        const transcribedText = transcriptResult.data.trim();
        if (transcribedText) {
          // Compare transcription with the track's lyrics
          const referenceText = track?.lyrics ?? "";
          const score = await commands.tutorScore(
            referenceText,
            transcribedText,
          );
          setScoreReport(score);
        } else {
          setScoreReport({
            overall: 0,
            reference_word_count: track?.words.length ?? 0,
            matched_word_count: 0,
            words: (track?.words ?? []).map((w) => ({
              reference: w.word,
              spoken: null,
              matched: false,
            })),
            note: "No singing detected. Please hold mic closer and project clearly.",
          });
        }
      } else {
        throw new Error(transcriptResult.error || "Transcription failed");
      }
    } catch (e) {
      console.error("Error transcribing recording:", e);
      setScoreReport({
        overall: 0,
        reference_word_count: track?.words.length ?? 0,
        matched_word_count: 0,
        words: (track?.words ?? []).map((w) => ({
          reference: w.word,
          spoken: null,
          matched: false,
        })),
        note: "Failed to process audio. Please ensure a Whisper model is loaded in settings.",
      });
    } finally {
      setIsTranscribing(false);
    }
  };

  // Helper to check if a word is currently active based on currentTime
  const getActiveWordIndex = () => {
    if (!track) return -1;
    return track.words.findIndex(
      (w) => currentTime >= w.start && currentTime <= w.end,
    );
  };

  const activeWordIdx = getActiveWordIndex();

  // Sing-along controls (unified live pitch ribbon + accuracy score)
  const startSingAlong = () => {
    if (!track || !customAudioUrl) return;
    handleStop();
    const contour = refContourRef.current;
    const targetAt = (t: number): number | null =>
      contour ? contourHzAt(contour, t) : null;
    void singAlong.start({
      targetAt,
      startReference: () => {
        if (audioRef.current) {
          audioRef.current.currentTime = 0;
          void audioRef.current.play();
        }
        setIsPlaying(true);
      },
      stopReference: () => handleStop(),
    });
  };

  const stopSingAlong = () => {
    singAlong.stop();
  };

  return (
    <div className="w-full max-w-4xl mx-auto flex flex-col gap-6 animate-in fade-in duration-500">
      {/* Invisible HTML5 Audio Element for the imported track */}
      {customAudioUrl && (
        <audio
          ref={audioRef}
          src={customAudioUrl}
          onLoadedMetadata={handleAudioMetadataLoaded}
          onTimeUpdate={handleAudioTimeUpdate}
          onEnded={handleAudioEnded}
        />
      )}

      {/* Main Glassmorphic Panel */}
      <div className="relative overflow-hidden rounded-3xl border border-surface-raised bg-surface/30 backdrop-blur-2xl p-6 sm:p-8 flex flex-col items-center gap-6 shadow-2xl">
        {/* Glow Spheres in the Background */}
        <div
          className={`absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-80 h-80 rounded-full bg-accent/10 blur-[100px] pointer-events-none transition-transform duration-1000 ${isPlaying ? "scale-150 animate-pulse" : "scale-100"}`}
        />
        <div className="absolute top-10 right-10 w-40 h-40 rounded-full bg-accent/5 blur-[80px] pointer-events-none" />

        {/* Top Header & Import control */}
        <div className="w-full flex flex-col sm:flex-row items-center justify-between gap-4 z-10">
          <div className="flex items-center gap-2">
            <div className="p-2.5 rounded-2xl bg-accent/10 border border-accent/20 text-accent">
              <Sparkles className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-xl font-black text-white tracking-tight leading-none">
                {t("settings.karaoke.title")}
              </h2>
              <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
                {t("settings.karaoke.subtitle")}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={handleImportAudio}
              className="p-2 rounded-xl bg-ground/60 hover:bg-surface border border-surface-raised text-secondary hover:text-accent transition-colors flex items-center gap-1.5 text-xs font-semibold"
              title={t("settings.karaoke.import")}
            >
              <Upload className="w-3.5 h-3.5" />
              <span>{t("settings.karaoke.import")}</span>
            </button>
          </div>
        </div>

        {!track ? (
          /* Empty state — no track imported yet */
          <div className="w-full flex flex-col items-center justify-center min-h-[160px] text-center my-6 z-10 gap-4">
            <div className="p-4 rounded-full bg-ground/40 border border-surface-raised/60 text-secondary">
              <Music className="w-8 h-8" />
            </div>
            <button
              onClick={handleImportAudio}
              className="inline-flex items-center gap-2 rounded-xl bg-accent px-4 py-2 text-sm font-semibold text-ground"
            >
              <Upload className="w-4 h-4" />
              {t("settings.karaoke.import")}
            </button>
          </div>
        ) : (
          <>
            {/* Text Display Section */}
            <div className="w-full flex flex-col items-center justify-center min-h-[160px] text-center my-6 z-10 px-4">
              {/* Transliteration Timing Display */}
              <div className="flex flex-wrap items-center justify-center gap-x-3 gap-y-2 max-w-2xl">
                {track.words.length > 0 ? (
                  track.words.map((w, idx) => {
                    const isActive = idx === activeWordIdx;
                    const isPassed = idx < activeWordIdx;

                    return (
                      <span
                        key={idx}
                        className={`text-xl sm:text-2xl font-bold tracking-tight px-1.5 py-0.5 rounded-lg transition-all duration-300 ${
                          isActive
                            ? "text-accent scale-110 drop-shadow-[0_0_12px_var(--color-accent-glow)] bg-accent/10 border border-accent/20"
                            : isPassed
                              ? "text-accent/80 font-medium"
                              : "text-secondary opacity-60"
                        }`}
                      >
                        {w.word}
                      </span>
                    );
                  })
                ) : (
                  <span className="text-sm italic text-secondary">
                    {"Enter lyrics below to set up karaoke sync!"}
                  </span>
                )}
              </div>
            </div>

            {/* Lyrics input */}
            <div className="w-full z-10 border-t border-surface-raised/60 pt-4 flex flex-col gap-2">
              <label className="text-[10px] font-bold text-secondary uppercase tracking-widest">
                {t("settings.karaoke.lyricsLabel")}
              </label>
              <textarea
                value={customLyrics}
                onChange={handleLyricsChange}
                placeholder="e.g. Amazing grace how sweet the sound"
                className="w-full h-16 text-xs bg-ground/40 border border-surface-raised rounded-xl p-3 text-primary focus:outline-none focus:border-accent"
              />
            </div>

            {/* Audio Visualizer representation */}
            <div className="w-full h-12 flex items-center justify-center gap-1 z-10">
              {Array.from({ length: 24 }).map((_, i) => {
                const isActive = isPlaying;
                // Procedural pulsing bar heights
                const scale = isActive
                  ? Math.sin(currentTime * 5 + i * 0.4) * 0.8 + 1.2
                  : 0.1;

                return (
                  <div
                    key={i}
                    style={{
                      height: `${Math.max(4, Math.min(36, scale * 14))}px`,
                    }}
                    className={`w-1 rounded-full transition-all duration-150 ${
                      isActive
                        ? "bg-gradient-to-t from-accent to-accent-hot"
                        : "bg-surface-raised"
                    }`}
                  />
                );
              })}
            </div>

            {/* Progress bar slider */}
            <div className="w-full flex items-center gap-3 z-10 text-[10px] font-bold text-secondary">
              <span>
                {t("settings.karaoke.seconds", { val: currentTime.toFixed(1) })}
              </span>
              <div className="flex-1 h-1.5 bg-ground rounded-full relative overflow-hidden group cursor-pointer">
                <div
                  style={{
                    width: `${(currentTime / track.duration) * 100}%`,
                  }}
                  className="absolute left-0 top-0 h-full bg-gradient-to-r from-accent to-accent-hot transition-all duration-75"
                />
              </div>
              <span>
                {t("settings.karaoke.seconds", {
                  val: track.duration.toFixed(1),
                })}
              </span>
            </div>

            {/* Playback Controls & Recording Interface */}
            <div className="w-full flex flex-col sm:flex-row items-center justify-between gap-4 mt-2 z-10">
              {/* Speed control */}
              <div className="flex items-center gap-2">
                <Volume2 className="w-4 h-4 text-secondary" />
                <select
                  value={playbackSpeed}
                  onChange={(e) => setPlaybackSpeed(parseFloat(e.target.value))}
                  className="text-[10px] font-bold bg-ground/60 border border-surface-raised rounded-lg px-2 py-1 text-secondary"
                >
                  <option value="0.75">
                    {t("settings.karaoke.speedSlow")}
                  </option>
                  <option value="1.0">
                    {t("settings.karaoke.speedNormal")}
                  </option>
                  <option value="1.25">
                    {t("settings.karaoke.speedFast")}
                  </option>
                </select>
              </div>

              {/* Main playback buttons */}
              <div className="flex items-center gap-3">
                <button
                  onClick={handleStop}
                  className="p-3 rounded-2xl bg-ground/60 hover:bg-surface border border-surface-raised text-primary hover:text-err transition-all active:scale-95"
                >
                  <Square className="w-4 h-4 fill-current" />
                </button>

                <button
                  onClick={handlePlayPause}
                  className="p-5 rounded-full bg-gradient-to-r from-accent to-accent-hot hover:from-accent hover:to-accent-hot text-white shadow-xl shadow-accent/20 border-none transition-all hover:scale-105 active:scale-95"
                >
                  {isPlaying ? (
                    <Pause className="w-6 h-6 fill-current" />
                  ) : (
                    <Play className="w-6 h-6 fill-current ml-0.5" />
                  )}
                </button>

                {/* Record Button */}
                <button
                  onClick={isRecording ? stopRecording : startRecording}
                  className={`p-3 rounded-2xl transition-all active:scale-95 ${
                    isRecording
                      ? "bg-accent-hot text-white animate-pulse"
                      : "bg-ground/60 hover:bg-surface border border-surface-raised text-primary hover:text-accent"
                  }`}
                >
                  <Mic className="w-4 h-4" />
                </button>
              </div>

              <div className="text-[10px] font-bold text-secondary uppercase tracking-widest select-none">
                {isRecording
                  ? `Recording... ${recordDuration}s`
                  : "Ready to sing"}
              </div>
            </div>
          </>
        )}
      </div>

      {/* Sing-along: live pitch ribbon + accuracy score */}
      <div className="mt-4 flex flex-col gap-2">
        <button
          type="button"
          onClick={singAlong.isSinging ? stopSingAlong : startSingAlong}
          disabled={!track}
          className="inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-medium text-ground disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {singAlong.isSinging ? (
            <>
              <Square className="h-4 w-4" /> {t("settings.karaoke.singStop")}
            </>
          ) : (
            <>
              <Music className="h-4 w-4" /> {t("settings.karaoke.singAlong")}
            </>
          )}
        </button>
        {singAlong.error === "mic" && (
          <div className="text-sm text-err">
            {t("settings.karaoke.micError")}
          </div>
        )}
        {track && (
          <div className="text-xs text-secondary">
            {t("settings.karaoke.customApprox")}
          </div>
        )}
        {singAlong.isSinging && (
          <PitchRibbon
            getFrame={singAlong.getFrame}
            reducedMotion={prefersReducedMotion}
          />
        )}
        {singAlong.score && !singAlong.isSinging && (
          <div className="rounded-lg bg-surface p-3">
            <div className="text-sm font-medium text-primary">
              {t("settings.karaoke.pitchScoreTitle")}
            </div>
            <div className="text-2xl font-semibold text-accent">
              {t("settings.karaoke.pitchScoreValue", {
                pct: singAlong.score.percentInPitch,
              })}
            </div>
          </div>
        )}
      </div>

      {/* Transcription Scoring Panel */}
      {isTranscribing && (
        <div className="p-6 rounded-3xl border border-surface-raised bg-surface/20 flex flex-col items-center justify-center gap-3 text-center">
          <div className="w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin" />
          <span className="text-xs font-semibold text-secondary">
            Analyzing your performance with Whisper...
          </span>
        </div>
      )}

      {scoreReport && (
        <div className="p-6 sm:p-8 rounded-3xl border border-accent/20 bg-accent/5 backdrop-blur-md flex flex-col md:flex-row gap-6 items-center md:items-start justify-between shadow-lg animate-in zoom-in-95 duration-300">
          <div className="flex flex-col items-center md:items-start gap-2">
            <div className="flex items-center gap-2 text-accent">
              <Award className="w-5 h-5" />
              <span className="text-[10px] font-bold uppercase tracking-widest">
                {t("settings.karaoke.scoreTitle")}
              </span>
            </div>

            <div className="flex items-baseline gap-1 mt-1">
              <span className="text-5xl font-black text-white tracking-tighter">
                {Math.round(scoreReport.overall)}%
              </span>
              <span className="text-xs text-secondary font-bold">
                {t("settings.karaoke.match")}
              </span>
            </div>

            <p className="text-xs text-primary italic leading-relaxed text-center md:text-left mt-2 max-w-md">
              "{scoreReport.note}"
            </p>
          </div>

          <div className="flex-1 flex flex-col gap-3 w-full md:max-w-md border-t md:border-t-0 md:border-l border-surface-raised/80 pt-5 md:pt-0 md:pl-6">
            <span className="text-[10px] font-bold text-secondary uppercase tracking-widest">
              {t("settings.karaoke.alignmentBreakdown")}
            </span>
            <div className="flex flex-wrap gap-2">
              {scoreReport.words.map((word, idx) => (
                <div
                  key={idx}
                  className={`px-3 py-1.5 rounded-xl text-xs font-semibold transition-colors flex items-center gap-1.5 ${
                    word.matched
                      ? "text-ok bg-ok/10 border border-ok/20"
                      : "text-err bg-err/10 border border-err/20"
                  }`}
                >
                  {word.matched ? (
                    <Check className="w-3.5 h-3.5 text-ok" />
                  ) : (
                    <X className="w-3.5 h-3.5 text-err" />
                  )}
                  <span>{word.reference}</span>
                  {word.spoken && word.spoken !== word.reference && (
                    <span className="text-[10px] text-secondary line-through pl-1">
                      {word.spoken}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
