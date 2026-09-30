import React, { useEffect, useRef } from "react";
import { dbfsToIntensity } from "./spectrumMath";
import type { SpectrumState } from "./useSpectrum";

interface SpectrogramCanvasProps extends Pick<
  SpectrumState,
  "framesRef" | "sampleRate" | "active"
> {
  frozen: boolean;
  /** False while the panel is off-screen (IntersectionObserver in SpectrumPanel) — skip appending columns. */
  visible: boolean;
}

/** Read the Warm Studio semantic tokens (a canvas can't use Tailwind classes). */
function readTokens(el: HTMLElement) {
  const cs = getComputedStyle(el);
  const v = (name: string) => cs.getPropertyValue(name).trim();
  return {
    ground: v("--color-ground"),
    accent: v("--color-accent"),
    accentHot: v("--color-accent-hot"),
  };
}

/**
 * The scrolling spectrogram: one column per analyzer frame, self-blitted left by
 * a pixel each time. Y is log-frequency (low at the bottom — the bands are
 * already log-spaced, so the band index maps linearly to y). Colour intensity
 * comes from dBFS.
 *
 * Columns are drawn on new-frame arrival, not on a wall-clock timer:
 * `useSpectrum` assigns a brand-new array object to `framesRef.current` on
 * every incoming event, so reference identity is a perfect "new frame"
 * signal. The rAF loop compares `framesRef.current` against the last frame it
 * drew and skips the draw when nothing new has arrived, which guarantees
 * exactly one column per analyzer frame with no drift-induced double-draws
 * or skips.
 */
export const SpectrogramCanvas: React.FC<SpectrogramCanvasProps> = ({
  framesRef,
  sampleRate,
  active,
  frozen,
  visible,
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const propsRef = useRef({ sampleRate, active, frozen, visible });
  propsRef.current = { sampleRate, active, frozen, visible };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let tokens = readTokens(canvas);
    let raf = 0;
    let lastRate = 0;
    let lastDrawnFrame: number[] | null = null;

    const clear = () => {
      const dpr = window.devicePixelRatio || 1;
      ctx.fillStyle = tokens.ground;
      ctx.fillRect(0, 0, canvas.width / dpr, canvas.height / dpr);
    };

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(rect.width * dpr));
      canvas.height = Math.max(1, Math.round(rect.height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      tokens = readTokens(canvas);
      clear(); // the old pixels are the wrong size now
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    const draw = () => {
      raf = requestAnimationFrame(draw);
      const {
        sampleRate: rate,
        active: isActive,
        frozen: isFrozen,
        visible: isVisible,
      } = propsRef.current;

      // A rate change means a new frequency axis — the old columns would LIE.
      if (rate !== lastRate) {
        lastRate = rate;
        lastDrawnFrame = null;
        clear();
      }

      // Off-screen: keep the loop alive but append no columns until visible again.
      if (isFrozen || !isActive || !isVisible) return;
      const bands = framesRef.current;
      if (!bands || bands.length === 0 || bands === lastDrawnFrame) return;
      lastDrawnFrame = bands;

      const dpr = window.devicePixelRatio || 1;
      const w = canvas.width / dpr;
      const h = canvas.height / dpr;

      // Scroll left by one pixel by blitting the canvas onto itself.
      ctx.drawImage(canvas, -1 / dpr, 0, w, h);

      // Draw the new column at the right edge, low frequencies at the bottom.
      const bandH = h / bands.length;
      for (let i = 0; i < bands.length; i++) {
        const y = h - (i + 1) * bandH;
        const intensity = dbfsToIntensity(bands[i]);
        ctx.globalAlpha = 1;
        ctx.fillStyle = tokens.ground;
        ctx.fillRect(w - 1, y, 1, bandH + 1);
        if (intensity > 0) {
          ctx.globalAlpha = intensity;
          ctx.fillStyle = intensity > 0.85 ? tokens.accentHot : tokens.accent;
          ctx.fillRect(w - 1, y, 1, bandH + 1);
        }
      }
      ctx.globalAlpha = 1;
    };

    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
    // framesRef is a stable ref; props are mirrored through propsRef.
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="w-full h-24 rounded-md border border-edge bg-ground"
      aria-hidden="true"
    />
  );
};
