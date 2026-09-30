import React, { useEffect, useRef } from "react";
import { peakHold } from "./meterMath";
import { dbfsToY, gridlinePositions, shouldRepaint } from "./spectrumMath";
import type { SpectrumState } from "./useSpectrum";

/** dB gridlines drawn across the plot. */
const DB_MARKS = [0, -20, -40, -60, -80];
/** Frequency gridlines. Marks beyond the device's Nyquist are skipped by gridlinePositions. */
const FREQ_MARKS = [100, 1000, 10000];
/** Peak-hold decay per drawn frame (~60/s) — a slow, readable fall. */
const HOLD_DECAY_DB = 0.6;

interface SpectrumCanvasProps extends SpectrumState {
  peakHoldOn: boolean;
  frozen: boolean;
  /** False while the panel is off-screen (IntersectionObserver in SpectrumPanel) — skip repaint work. */
  visible: boolean;
}

/** Read the Warm Studio semantic tokens (a canvas can't use Tailwind classes). */
function readTokens(el: HTMLElement) {
  const cs = getComputedStyle(el);
  const v = (name: string) => cs.getPropertyValue(name).trim();
  return {
    surface: v("--color-surface"),
    edge: v("--color-edge"),
    secondary: v("--color-secondary"),
    accent: v("--color-accent"),
    accentHot: v("--color-accent-hot"),
  };
}

/**
 * The spectrum plot: a dB/frequency grid, the live band curve, and a peak-hold
 * line. Frames are read from a ref inside a requestAnimationFrame loop, so this
 * component does not re-render per frame.
 */
export const SpectrumCanvas: React.FC<SpectrumCanvasProps> = ({
  framesRef,
  freqs,
  active,
  peakHoldOn,
  frozen,
  visible,
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const holdRef = useRef<number[]>([]);
  /** A snapshot taken when freeze turns on — see the FREEZE MUST SNAPSHOT note below. */
  const frozenRef = useRef<number[] | null>(null);
  // Keep the latest props visible to the rAF loop without restarting it.
  const propsRef = useRef({ freqs, active, peakHoldOn, frozen, visible });
  propsRef.current = { freqs, active, peakHoldOn, frozen, visible };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let tokens = readTokens(canvas);
    let raf = 0;
    // Repaint-skip bookkeeping (mirrors SpectrogramCanvas's lastDrawnFrame gate).
    let lastDrawn: number[] | null = null;
    let dirty = true; // force a repaint after a resize / a prop change
    let prevFreqs: number[] | null = null;
    let prevHoldOn: boolean | null = null;
    let prevFrozen: boolean | null = null;
    let wasVisible = true;

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(rect.width * dpr));
      canvas.height = Math.max(1, Math.round(rect.height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      tokens = readTokens(canvas); // tokens are cheap to refresh here, not per frame
      dirty = true; // the canvas was cleared by the resize — it must be redrawn
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    const draw = () => {
      raf = requestAnimationFrame(draw);
      const {
        freqs: f,
        active: isActive,
        peakHoldOn: holdOn,
        frozen: isFrozen,
        visible: isVisible,
      } = propsRef.current;

      // Off-screen: keep the loop alive but do no work at all until visible again.
      if (!isVisible) {
        wasVisible = false;
        return;
      }
      if (!wasVisible) {
        wasVisible = true;
        dirty = true; // force a repaint on becoming visible again
      }

      // A change in freqs/peakHoldOn/frozen changes what's drawn even though it
      // reaches the loop via propsRef, not a new analyzer frame.
      if (f !== prevFreqs || holdOn !== prevHoldOn || isFrozen !== prevFrozen) {
        dirty = true;
        prevFreqs = f;
        prevHoldOn = holdOn;
        prevFrozen = isFrozen;
      }

      // FREEZE MUST SNAPSHOT. framesRef keeps receiving live frames regardless,
      // so drawing straight from it while "frozen" would still show live audio —
      // a freeze that doesn't freeze. On the false→true edge we copy the current
      // frame and draw that copy until unfrozen.
      if (isFrozen) {
        if (!frozenRef.current && framesRef.current) {
          frozenRef.current = framesRef.current.slice();
        }
      } else if (frozenRef.current) {
        frozenRef.current = null;
      }

      const src = frozenRef.current ?? framesRef.current;

      // --- peak-hold decay (element-wise reuse of the Epic-A meter's decay) --
      // Runs every tick — even on ticks whose paint is about to be skipped
      // below — because shouldRepaint() needs to know whether the hold moved
      // on THIS tick. Gating the decay itself on the paint gate would freeze
      // the hold's last visible step instead of ever showing it settle.
      let holdMoving = false;
      if (holdOn && isActive && src && src.length > 0) {
        if (holdRef.current.length !== src.length) {
          holdRef.current = src.slice(); // fresh hold starts locked to this frame
        } else if (!isFrozen) {
          for (let i = 0; i < src.length; i++) {
            const prevHold = holdRef.current[i];
            holdRef.current[i] = peakHold(prevHold, src[i], HOLD_DECAY_DB);
            // The band was still above its frame value BEFORE this tick's decay
            // step, so this tick is the (possibly last) one that visibly moves it.
            if (prevHold > src[i]) holdMoving = true;
          }
        }
      } else {
        holdRef.current = [];
      }

      // Repaint only when something can actually have changed: a resize/prop
      // change dirtied the canvas, a new frame arrived, or the hold moved.
      if (!shouldRepaint(dirty, src, lastDrawn, holdMoving)) return;
      dirty = false;
      lastDrawn = src;

      const dpr = window.devicePixelRatio || 1;
      const w = canvas.width / dpr;
      const h = canvas.height / dpr;

      ctx.fillStyle = tokens.surface;
      ctx.fillRect(0, 0, w, h);

      // --- grid ---------------------------------------------------------- //
      ctx.strokeStyle = tokens.edge;
      ctx.fillStyle = tokens.secondary;
      ctx.lineWidth = 1;
      ctx.font = "9px system-ui, sans-serif";

      for (const db of DB_MARKS) {
        const y = Math.round(dbfsToY(db, h)) + 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
        ctx.fillText(`${db}`, 2, Math.min(h - 2, y + 9));
      }

      for (const { freq, x } of gridlinePositions(f, FREQ_MARKS)) {
        const px = Math.round(x * w) + 0.5;
        ctx.beginPath();
        ctx.moveTo(px, 0);
        ctx.lineTo(px, h);
        ctx.stroke();
        const label = freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
        ctx.fillText(label, px + 2, h - 2);
      }

      if (!isActive || !src || src.length === 0) {
        return; // grid only — nothing to plot yet
      }

      // --- the band curve -------------------------------------------------- //
      const xOf = (i: number) => (i / (src.length - 1)) * w;

      ctx.beginPath();
      ctx.moveTo(0, h);
      for (let i = 0; i < src.length; i++) {
        ctx.lineTo(xOf(i), dbfsToY(src[i], h));
      }
      ctx.lineTo(w, h);
      ctx.closePath();
      ctx.globalAlpha = 0.25;
      ctx.fillStyle = tokens.accent;
      ctx.fill();
      ctx.globalAlpha = 1;

      ctx.beginPath();
      for (let i = 0; i < src.length; i++) {
        const x = xOf(i);
        const y = dbfsToY(src[i], h);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.strokeStyle = tokens.accent;
      ctx.lineWidth = 1.5;
      ctx.stroke();

      // --- peak-hold line -------------------------------------------------- //
      if (holdOn && holdRef.current.length === src.length) {
        ctx.beginPath();
        for (let i = 0; i < holdRef.current.length; i++) {
          const x = xOf(i);
          const y = dbfsToY(holdRef.current[i], h);
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.strokeStyle = tokens.accentHot;
        ctx.lineWidth = 1;
        ctx.stroke();
      }
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
      className="w-full h-40 rounded-md border border-edge bg-surface"
      aria-hidden="true"
    />
  );
};
