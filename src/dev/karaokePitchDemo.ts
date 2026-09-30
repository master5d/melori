// DEV-ONLY Karaoke pitch-ribbon browser smoke — mounts PitchRibbon over the
// app against a synthetic pitch sweep (no getUserMedia, which is unavailable
// headless). Gated by VITE_TAURI_MOCK + VITE_KARAOKE_DEMO in installMockFirst.ts
// so this module + PitchRibbon-demo path tree-shake out of prod.
import { createElement } from "react";
import { createRoot } from "react-dom/client";
import PitchRibbon from "../components/settings/karaoke/PitchRibbon";
import type { PitchSample } from "../components/settings/karaoke/pitchScore";

/**
 * Mount the ribbon in a fixed overlay with a synthetic C3-centered sweep, plus
 * a static representative score plate below it (real mic-driven scoring is
 * exercised by pitchScore.test.ts + the live KaraokeSettings flow — this demo
 * cannot produce a genuine take headlessly, so the plate below reuses the
 * exact KaraokeSettings markup/tokens with a fixed illustrative value, not a
 * fabricated "live" result).
 */
export function runKaraokePitchDemo(): void {
  const target = 130.81; // C3
  const mount = document.createElement("div");
  mount.style.cssText =
    "position:fixed;bottom:16px;left:16px;width:640px;z-index:99999";
  document.body.appendChild(mount);

  const samples: PitchSample[] = [];
  const startMs = performance.now();
  const getFrame = () => {
    const now = (performance.now() - startMs) / 1000;
    // sweep ±120 cents around C3 so the line crosses in/out of the ±50c band
    const hz = target * Math.pow(2, (Math.sin(now * 0.8) * 120) / 1200);
    samples.push({ t: now, hz, targetHz: target });
    if (samples.length > 600) samples.shift();
    return { samples, now, targetHz: target };
  };

  const reducedMotion =
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

  const scorePlate = createElement(
    "div",
    { className: "mt-2 rounded-lg bg-surface p-3" },
    createElement(
      "div",
      { className: "text-sm font-medium text-primary" },
      "Pitch accuracy (demo — representative, not a live take)",
    ),
    createElement(
      "div",
      { className: "text-2xl font-semibold text-accent" },
      "72% in pitch",
    ),
  );

  createRoot(mount).render(
    createElement(
      "div",
      null,
      createElement(PitchRibbon, { getFrame, reducedMotion }),
      scorePlate,
    ),
  );
}
