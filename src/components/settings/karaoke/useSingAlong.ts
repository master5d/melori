//! Unified sing-along controller for the Karaoke module. Owns the Web Audio
//! plumbing (mic capture + rAF pitch-detect loop) and reference-playback
//! coordination; presentation lives in PitchRibbon.tsx.
import { useCallback, useEffect, useRef, useState } from "react";
import { detectPitch } from "./pitchDetect";
import { scoreInPitch, type PitchSample, type PitchScore } from "./pitchScore";

interface StartOpts {
  targetAt: (t: number) => number | null;
  startReference: () => void;
  stopReference: () => void;
}

/**
 * Unified sing-along controller: starts reference playback + mic capture,
 * runs a rAF pitch-detect loop, accumulates samples, and computes the score on
 * stop. All Web Audio nodes/streams are torn down on stop/unmount.
 */
export function useSingAlong() {
  const [isSinging, setIsSinging] = useState(false);
  const [score, setScore] = useState<PitchScore | null>(null);
  const [error, setError] = useState<string | null>(null);

  const ctxRef = useRef<AudioContext | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const rafRef = useRef<number | null>(null);
  const bufRef = useRef<Float32Array | null>(null);
  const samplesRef = useRef<PitchSample[]>([]);
  const nowRef = useRef(0);
  const targetHzRef = useRef<number | null>(null);
  const startedAtRef = useRef(0);
  const stopReferenceRef = useRef<() => void>(() => {});
  const genRef = useRef(0);

  const teardown = useCallback(() => {
    genRef.current++; // invalidate any in-flight start()
    if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
    rafRef.current = null;
    streamRef.current?.getTracks().forEach((tr) => tr.stop());
    streamRef.current = null;
    analyserRef.current?.disconnect();
    analyserRef.current = null;
    ctxRef.current?.close().catch(() => {});
    ctxRef.current = null;
  }, []);

  // Ensure teardown also runs on unmount, so the AudioContext/MediaStream
  // never leaks if the user closes the panel mid-sing.
  useEffect(() => teardown, [teardown]);

  const stop = useCallback(() => {
    stopReferenceRef.current();
    if (samplesRef.current.length > 0)
      setScore(scoreInPitch(samplesRef.current));
    teardown();
    setIsSinging(false);
  }, [teardown]);

  const start = useCallback(
    async (opts: StartOpts) => {
      teardown(); // supersede in-flight; bumps genRef
      const gen = genRef.current;
      setError(null);
      setScore(null);
      samplesRef.current = [];
      stopReferenceRef.current = opts.stopReference;
      let stream: MediaStream;
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      } catch {
        if (gen !== genRef.current) return; // superseded/unmounted during prompt
        setError("mic");
        // Reference-only fallback: still play the reference, no ribbon.
        opts.startReference();
        return;
      }
      if (gen !== genRef.current) {
        // superseded/unmounted while awaiting the mic prompt
        stream.getTracks().forEach((tr) => tr.stop());
        return;
      }
      const ctx = new AudioContext();
      const analyser = ctx.createAnalyser();
      analyser.fftSize = 2048;
      ctx.createMediaStreamSource(stream).connect(analyser);
      ctxRef.current = ctx;
      streamRef.current = stream;
      analyserRef.current = analyser;
      bufRef.current = new Float32Array(analyser.fftSize);
      startedAtRef.current = ctx.currentTime;

      opts.startReference();
      setIsSinging(true);

      const loop = () => {
        const a = analyserRef.current;
        const c = ctxRef.current;
        const buf = bufRef.current;
        if (!a || !c || !buf) return;
        a.getFloatTimeDomainData(buf);
        const t = c.currentTime - startedAtRef.current;
        const hz = detectPitch(buf, c.sampleRate);
        const targetHz = opts.targetAt(t);
        targetHzRef.current = targetHz;
        nowRef.current = t;
        samplesRef.current.push({ t, hz, targetHz });
        rafRef.current = requestAnimationFrame(loop);
      };
      rafRef.current = requestAnimationFrame(loop);
    },
    [teardown],
  );

  const getFrame = useCallback(
    () => ({
      samples: samplesRef.current,
      now: nowRef.current,
      targetHz: targetHzRef.current,
    }),
    [],
  );

  return { isSinging, score, error, start, stop, getFrame };
}
