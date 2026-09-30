import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSpectrum } from "./useSpectrum";
import { SpectrumCanvas } from "./SpectrumCanvas";
import { SpectrogramCanvas } from "./SpectrogramCanvas";

/**
 * The Audio-console analyzer: a live spectrum plus a scrolling spectrogram, both
 * driven by ONE `useSpectrum()` subscription (calling the hook inside each canvas
 * would open two listeners for the same stream).
 *
 * Peak-hold and freeze are ephemeral view state — no settings, no persistence.
 *
 * Visibility: an `IntersectionObserver` on the panel wrapper tells both canvases
 * to skip their rAF work while the panel is scrolled off-screen. This is a purely
 * frontend cost cut — it does NOT touch `useSpectrum` (still listen-only) or the
 * backend `monitor_active` gate, which the Epic-A meter (`useInputMonitor`) alone
 * owns the lifecycle of.
 */
interface Props {
  /** The device is gone (`unavailable`/`stalled`). Not set for `silent`. */
  signalLost?: boolean;
}

export const SpectrumPanel: React.FC<Props> = ({ signalLost = false }) => {
  const { t } = useTranslation();
  const spectrum = useSpectrum();
  const [peakHoldOn, setPeakHoldOn] = useState(true);
  const [frozen, setFrozen] = useState(false);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => setVisible(entry.isIntersecting),
      { threshold: 0 },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!signalLost) return;
    // framesRef's only writer is the input-spectrum listener, which always delivers
    // fresh audio. By the time signalLost turns true (stalled ≥2s or capture-error),
    // no frame can be in flight. On recovery, frames are already resuming, so the ref
    // holds at most one health tick of fresh data — correct to paint on remount.
    spectrum.framesRef.current = null;
  }, [signalLost, spectrum.framesRef]);

  const buttonClass = (on: boolean) =>
    `px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider transition-colors ${
      on
        ? "bg-accent text-ground"
        : "bg-surface-raised text-secondary hover:text-primary"
    }`;

  return (
    <div ref={panelRef} className="px-5 py-4 space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-black text-secondary uppercase tracking-[0.25em]">
          {t("settings.audio.spectrum.title")}
        </span>
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            className={buttonClass(peakHoldOn)}
            aria-pressed={peakHoldOn}
            onClick={() => setPeakHoldOn((v) => !v)}
          >
            {t("settings.audio.spectrum.peakHold")}
          </button>
          <button
            type="button"
            className={buttonClass(frozen)}
            aria-pressed={frozen}
            onClick={() => setFrozen((v) => !v)}
          >
            {t("settings.audio.spectrum.freeze")}
          </button>
        </div>
      </div>

      {signalLost ? (
        // UNMOUNT the canvases rather than telling them to draw nothing. That
        // kills their rAF loops, and — more importantly — discards the peak-hold
        // and the spectrogram history along with them. After a device loss that
        // history is not stale-but-harmless, it is WRONG: it depicts a signal
        // that no longer exists. The placeholders keep the layout identical
        // (h-40 + h-24) so nothing jumps when the device comes back.
        <>
          <div className="w-full h-40 rounded-md border border-edge bg-surface flex items-center justify-center">
            <span className="text-[10px] font-bold uppercase tracking-wider text-secondary/60">
              {t("settings.audio.deviceHealth.noSignal")}
            </span>
          </div>
          <div className="w-full h-24 rounded-md border border-edge bg-ground" />
        </>
      ) : (
        <>
          <SpectrumCanvas
            {...spectrum}
            peakHoldOn={peakHoldOn}
            frozen={frozen}
            visible={visible}
          />
          <SpectrogramCanvas
            framesRef={spectrum.framesRef}
            sampleRate={spectrum.sampleRate}
            active={spectrum.active}
            frozen={frozen}
            visible={visible}
          />
        </>
      )}

      {!signalLost && !spectrum.active && (
        <p className="text-[10px] text-secondary/50">
          {t("settings.audio.spectrum.idle")}
        </p>
      )}
    </div>
  );
};
