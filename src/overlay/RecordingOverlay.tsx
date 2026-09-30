import { listen } from "@tauri-apps/api/event";
import { X } from "lucide-react";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";
import {
  barsFromBands,
  formatTimer,
  isCancellable,
  pillVariant,
  type OverlayState,
} from "./pillLogic";

const SUBTITLE_FONT_PX: Record<string, number> = {
  small: 14,
  medium: 18,
  large: 24,
};

const WAVE_BAR_COUNT = 14;

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("preparing");
  const [bands, setBands] = useState<number[]>([]);
  const [subtitles, setSubtitles] = useState("");
  const [elapsedMs, setElapsedMs] = useState(0);
  const [loadingPercent, setLoadingPercent] = useState(0);
  const recordingStartRef = useRef<number | null>(null);
  const direction = getLanguageDirection(i18n.language);

  // Kept in sync every render (not just in an effect) so the Esc listener
  // below — registered once in the setup effect — always reads the
  // current pipeline state instead of the value from mount time.
  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    const setup = async () => {
      const unlistenShow = await listen("show-overlay", async (event) => {
        await syncLanguageFromSettings();
        // Apply the user's subtitle font size to the CSS var driving the
        // subtitle area. Read fresh on each show so changes take effect
        // without restarting the overlay window.
        try {
          const result = await commands.getAppSettings();
          if (result.status === "ok") {
            const px =
              SUBTITLE_FONT_PX[result.data.subtitle_font_size ?? "medium"] ??
              18;
            document.documentElement.style.setProperty(
              "--subtitle-font-size",
              `${px}px`,
            );
          }
        } catch {
          /* keep CSS fallback */
        }
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setIsVisible(true);
        if (overlayState === "recording" || overlayState === "preparing") {
          setSubtitles("");
        }
        if (overlayState === "preparing") setLoadingPercent(0);
      });

      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
      });

      // Keep the raw per-band spectrum so the listening capsule can render
      // a real waveform (barsFromBands), instead of collapsing to a peak.
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        setBands(event.payload ?? []);
      });

      const unlistenModelProgress = await listen<{ percentage: number }>(
        "model-download-progress",
        (event) => {
          const percentage = event.payload?.percentage;
          if (Number.isFinite(percentage)) {
            setLoadingPercent(Math.max(0, Math.min(100, percentage)));
          }
        },
      );

      const unlistenSubtitles = await listen<string>(
        "subtitle-update",
        (event) => setSubtitles(event.payload),
      );

      // Esc-to-cancel: a passive keydown listener on the document — never
      // calls .focus() or otherwise activates the overlay window, so the
      // never-steal-focus invariant holds. Only acts when the current
      // variant is cancellable (mirrors the pf-cancel/pf-ring button).
      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key !== "Escape") return;
        if (isCancellable(pillVariant(stateRef.current))) {
          commands.cancelOperation();
        }
      };
      document.addEventListener("keydown", handleKeyDown);

      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
        unlistenModelProgress();
        unlistenSubtitles();
        document.removeEventListener("keydown", handleKeyDown);
      };
    };

    // setup() is async and resolves to the listener/keydown teardown fn;
    // wire it into React's cleanup so remounts (e.g. StrictMode dev) don't
    // leak or double-register the Esc handler.
    const cleanupPromise = setup();
    return () => {
      void cleanupPromise.then((cleanup) => cleanup?.());
    };
  }, []);

  const variant = pillVariant(state);
  const isPreparing = state === "preparing";
  const canCancel = isCancellable(variant);

  // Mono session timer: starts counting from the moment the pill enters
  // the "listening" variant, resets once it leaves it.
  useEffect(() => {
    if (variant !== "listening") {
      recordingStartRef.current = null;
      setElapsedMs(0);
      return;
    }
    recordingStartRef.current = Date.now();
    setElapsedMs(0);
    const interval = window.setInterval(() => {
      if (recordingStartRef.current !== null) {
        setElapsedMs(Date.now() - recordingStartRef.current);
      }
    }, 100);
    return () => window.clearInterval(interval);
  }, [variant]);

  const caption = isPreparing
    ? t("overlay.preparing")
    : state === "transcribing"
      ? t("overlay.transcribing")
      : state === "processing"
        ? t("overlay.processing")
        : "";

  const bars = barsFromBands(bands, WAVE_BAR_COUNT);

  const handleCancel = () => {
    if (canCancel) commands.cancelOperation();
  };

  return (
    <div className="overlay-container">
      <div dir={direction} className={`pf-root ${isVisible ? "fade-in" : ""}`}>
        {variant === "listening" ? (
          <div className="pf-surface pf-listening" role="status">
            <span className="pf-ember" aria-hidden="true" />
            <span className="pf-label">
              {t("overlay.recording")} ·{" "}
              <span className="pf-timer">{formatTimer(elapsedMs)}</span>
            </span>
            <div className="pf-wave" aria-hidden="true">
              {bars.map((h, i) => (
                <span key={i} className="pf-bar" style={{ height: `${h}px` }} />
              ))}
            </div>
            <button
              type="button"
              className="pf-cancel"
              aria-label={t("overlay.cancel")}
              onClick={handleCancel}
            >
              <X size={13} strokeWidth={2} aria-hidden="true" />
            </button>
          </div>
        ) : variant === "processing" ? (
          <div className="pf-surface pf-stage" role="status">
            <button
              type="button"
              className="pf-ring"
              aria-label={canCancel ? t("overlay.cancel") : caption}
              onClick={handleCancel}
            ></button>
            <span className="pf-proc">{t("overlay.movingToWindow")}</span>
          </div>
        ) : variant === "loading" ? (
          <div className="pf-surface pf-stage" role="status">
            <button
              type="button"
              className="pf-cancel"
              aria-label={canCancel ? t("overlay.cancel") : caption}
              onClick={handleCancel}
            >
              <X size={13} strokeWidth={2} aria-hidden="true" />
            </button>
            <span className="pf-proc">{t("overlay.loadingModel")}</span>
            <span className="pf-model">{loadingPercent}%</span>
          </div>
        ) : (
          <div className="pf-surface pf-stage" role="status">
            <span className="pf-proc">{caption}</span>
          </div>
        )}
      </div>

      {isVisible && subtitles && (
        <div className="subtitle-area fade-in">{subtitles}</div>
      )}
    </div>
  );
};

export default RecordingOverlay;
