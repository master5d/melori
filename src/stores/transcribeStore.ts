import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/bindings";

export type TranscribeFormat =
  | "plain"
  | "inline"
  | "srt"
  | "vtt"
  | "json"
  | "karaoke";

type Progress = { phase: string; percent: number | null } | null;

interface TranscribeState {
  // Inputs (persist across tab switches)
  path: string | null;
  timestamps: boolean;
  format: TranscribeFormat;
  diarize: boolean;
  speakers: string;
  outputPath: string | null;
  // Optional offline-translation target (ISO code like "en"/"ru"); null = no translation.
  translateTarget: string | null;

  // Run state (persist so a long transcription stays visible after navigating
  // away and back — the component used to hold this locally and lost it on
  // unmount, hiding progress and resetting the form mid-run).
  busy: boolean;
  result: string;
  error: string | null;
  progress: Progress;
  cancelling: boolean;
  savedTo: string | null;

  setPath: (p: string | null) => void;
  setTimestamps: (b: boolean) => void;
  setFormat: (f: TranscribeFormat) => void;
  setDiarize: (b: boolean) => void;
  setSpeakers: (s: string) => void;
  setOutputPath: (p: string | null) => void;
  setTranslateTarget: (c: string | null) => void;

  run: () => Promise<void>;
  cancel: () => Promise<void>;
}

export const useTranscribeStore = create<TranscribeState>((set, get) => ({
  path: null,
  timestamps: false,
  format: "inline",
  diarize: false,
  speakers: "",
  outputPath: null,
  translateTarget: null,

  busy: false,
  result: "",
  error: null,
  progress: null,
  cancelling: false,
  savedTo: null,

  setPath: (p) => set({ path: p, result: "", error: null }),
  setTimestamps: (b) => set({ timestamps: b }),
  setFormat: (f) => set({ format: f }),
  setDiarize: (b) => set({ diarize: b }),
  setSpeakers: (s) => set({ speakers: s }),
  setOutputPath: (p) => set({ outputPath: p }),
  setTranslateTarget: (c) => set({ translateTarget: c }),

  run: async () => {
    const {
      path,
      timestamps,
      format,
      diarize,
      speakers,
      outputPath,
      translateTarget,
      busy,
    } = get();
    if (!path || busy) return;

    set({
      busy: true,
      error: null,
      result: "",
      savedTo: null,
      cancelling: false,
      progress: { phase: "decoding", percent: null },
    });

    const effectiveFormat: TranscribeFormat = timestamps ? format : "plain";
    const hint = diarize && speakers.trim() ? Number(speakers.trim()) : null;

    // Listeners live for the run's lifetime in the store, not the component, so
    // progress keeps flowing even while the Transcribe File tab is unmounted.
    const unlisten = await listen<{ phase: string; percent: number | null }>(
      "transcription-progress",
      (e) => set({ progress: e.payload }),
    );
    const unlistenDl = await listen<{ percentage: number }>(
      "model-download-progress",
      (e) =>
        set({
          progress: { phase: "loading_model", percent: e.payload.percentage },
        }),
    );

    try {
      const res = await commands.transcribeFileToString(
        path,
        null,
        null,
        diarize,
        Number.isFinite(hint) ? hint : null,
        effectiveFormat,
        translateTarget,
      );
      if (res.status === "ok") {
        set({ result: res.data });
        if (outputPath) {
          const { writeTextFile } = await import("@tauri-apps/plugin-fs");
          await writeTextFile(outputPath, res.data);
          set({ savedTo: outputPath });
        }
      } else if (!get().cancelling) {
        set({ error: res.error });
      }
    } finally {
      unlisten();
      unlistenDl();
      set({ busy: false, progress: null });
    }
  },

  cancel: async () => {
    set({ cancelling: true });
    await commands.cancelFileTranscription();
  },
}));
