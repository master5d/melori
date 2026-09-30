import { useEffect, useRef } from "react";
import type { PitchSample } from "./pitchScore";
import { octaveFoldedInBand } from "./pitchDetect";
import {
  hzToY,
  timeToX,
  targetOctaveHzs,
  RIBBON_WINDOW_SEC,
} from "./ribbonLayout";

const TOL_CENTS = 50;

interface PitchRibbonProps {
  getFrame: () => {
    samples: PitchSample[];
    now: number;
    targetHz: number | null;
  };
  reducedMotion: boolean;
}

/** Read a Warm Studio token color (e.g. "--color-accent") to a CSS string. */
function tokenColor(name: string): string {
  return (
    getComputedStyle(document.documentElement).getPropertyValue(name).trim() ||
    "#888"
  );
}

/**
 * Canvas horizontal pitch ribbon: octave target bands + your live pitch line,
 * hit (in-band) vs miss colored. Presentational — reads frame data via
 * getFrame() each rAF tick; holds no session state.
 */
export default function PitchRibbon({
  getFrame,
  reducedMotion,
}: PitchRibbonProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const colBg = tokenColor("--color-surface");
    const colBand =
      tokenColor("--color-accent-glow-soft") || tokenColor("--color-accent");
    const colHit = tokenColor("--color-ok");
    const colMiss = tokenColor("--color-err");
    const colEdge = tokenColor("--color-accent"); // --color-edge is ~0.11 alpha, too faint for the now-marker

    const draw = () => {
      const { samples, now, targetHz } = getFrame();
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      ctx.fillStyle = colBg;
      ctx.fillRect(0, 0, w, h);

      // Target octave bands (±50 cents → pixel half-height around each octave).
      if (targetHz != null) {
        ctx.fillStyle = colBand;
        for (const oct of targetOctaveHzs(targetHz)) {
          const yc = hzToY(oct, h);
          const yTop = hzToY(oct * Math.pow(2, TOL_CENTS / 1200), h);
          const yBot = hzToY(oct * Math.pow(2, -TOL_CENTS / 1200), h);
          ctx.globalAlpha = 0.5;
          ctx.fillRect(0, Math.min(yTop, yc), w, Math.abs(yBot - yTop));
          ctx.globalAlpha = 1;
        }
      }

      // Your live pitch line over the visible window.
      const start = now - RIBBON_WINDOW_SEC;
      ctx.lineWidth = 2;
      let prevX: number | null = null;
      let prevY: number | null = null;
      for (const s of samples) {
        if (s.t < start || s.hz == null) {
          prevX = null;
          prevY = null;
          continue;
        }
        const x = timeToX(s.t, now, RIBBON_WINDOW_SEC, w);
        const y = hzToY(s.hz, h);
        const hit =
          s.targetHz != null && octaveFoldedInBand(s.hz, s.targetHz, TOL_CENTS);
        ctx.strokeStyle = hit ? colHit : colMiss;
        if (prevX != null && prevY != null) {
          ctx.beginPath();
          ctx.moveTo(prevX, prevY);
          ctx.lineTo(x, y);
          ctx.stroke();
        }
        prevX = x;
        prevY = y;
      }

      // "Now" marker at the right edge.
      ctx.strokeStyle = colEdge;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(w - 1, 0);
      ctx.lineTo(w - 1, h);
      ctx.stroke();
    };

    if (reducedMotion) {
      // Redraw on a slow interval instead of a continuous rAF scroll.
      draw();
      const id = window.setInterval(draw, 250);
      return () => window.clearInterval(id);
    }
    const loop = () => {
      draw();
      rafRef.current = requestAnimationFrame(loop);
    };
    rafRef.current = requestAnimationFrame(loop);
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
    };
  }, [getFrame, reducedMotion]);

  return (
    <canvas
      ref={canvasRef}
      width={640}
      height={200}
      className="w-full rounded-lg"
    />
  );
}
