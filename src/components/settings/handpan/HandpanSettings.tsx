import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, Volume2, Info, Compass, HelpCircle } from "lucide-react";
import { SettingsGroup } from "@/components/ui";

interface HandpanNote {
  name: string;
  freq: number;
  key: string;
  // Polar coordinates for circular placement in UI (angle in radians, radius in px)
  angle: number;
  radius: number;
}

interface HandpanScale {
  id: string;
  name: string;
  notes: HandpanNote[]; // First note is the central "Ding" (fundamental)
}

const HANDPAN_SCALES: HandpanScale[] = [
  {
    id: "d_kurd",
    name: "D Kurd Minor",
    notes: [
      { name: "D3", freq: 146.83, key: "5", angle: 0, radius: 0 }, // Ding (Center)
      { name: "A3", freq: 220.0, key: "1", angle: Math.PI * 0.75, radius: 110 },
      {
        name: "Bb3",
        freq: 233.08,
        key: "2",
        angle: Math.PI * 0.5,
        radius: 110,
      },
      {
        name: "C4",
        freq: 261.63,
        key: "3",
        angle: Math.PI * 0.25,
        radius: 110,
      },
      { name: "D4", freq: 293.66, key: "4", angle: 0, radius: 110 },
      {
        name: "E4",
        freq: 329.63,
        key: "6",
        angle: Math.PI * 1.75,
        radius: 110,
      },
      { name: "F4", freq: 349.23, key: "7", angle: Math.PI * 1.5, radius: 110 },
      { name: "G4", freq: 392.0, key: "8", angle: Math.PI * 1.25, radius: 110 },
      { name: "A4", freq: 440.0, key: "9", angle: Math.PI, radius: 110 },
    ],
  },
  {
    id: "d_celtic",
    name: "D Celtic Minor (Amara)",
    notes: [
      { name: "D3", freq: 146.83, key: "5", angle: 0, radius: 0 },
      { name: "A3", freq: 220.0, key: "1", angle: Math.PI * 0.75, radius: 110 },
      { name: "C4", freq: 261.63, key: "2", angle: Math.PI * 0.5, radius: 110 },
      {
        name: "D4",
        freq: 293.66,
        key: "3",
        angle: Math.PI * 0.25,
        radius: 110,
      },
      { name: "E4", freq: 329.63, key: "4", angle: 0, radius: 110 },
      {
        name: "F4",
        freq: 349.23,
        key: "6",
        angle: Math.PI * 1.75,
        radius: 110,
      },
      { name: "G4", freq: 392.0, key: "7", angle: Math.PI * 1.5, radius: 110 },
      { name: "A4", freq: 440.0, key: "8", angle: Math.PI * 1.25, radius: 110 },
      { name: "C5", freq: 523.25, key: "9", angle: Math.PI, radius: 110 },
    ],
  },
  {
    id: "cs_integral",
    name: "C# Integral",
    notes: [
      { name: "C#3", freq: 138.59, key: "5", angle: 0, radius: 0 },
      {
        name: "G#3",
        freq: 207.65,
        key: "1",
        angle: Math.PI * 0.75,
        radius: 110,
      },
      { name: "A3", freq: 220.0, key: "2", angle: Math.PI * 0.5, radius: 110 },
      {
        name: "B3",
        freq: 246.94,
        key: "3",
        angle: Math.PI * 0.25,
        radius: 110,
      },
      { name: "C#4", freq: 277.18, key: "4", angle: 0, radius: 110 },
      {
        name: "D#4",
        freq: 311.13,
        key: "6",
        angle: Math.PI * 1.75,
        radius: 110,
      },
      { name: "E4", freq: 329.63, key: "7", angle: Math.PI * 1.5, radius: 110 },
      {
        name: "F#4",
        freq: 369.99,
        key: "8",
        angle: Math.PI * 1.25,
        radius: 110,
      },
      { name: "G#4", freq: 415.3, key: "9", angle: Math.PI, radius: 110 },
    ],
  },
  {
    id: "d_hijaz",
    name: "D Hijaz (Double Harmonic)",
    notes: [
      { name: "D3", freq: 146.83, key: "5", angle: 0, radius: 0 },
      { name: "A3", freq: 220.0, key: "1", angle: Math.PI * 0.75, radius: 110 },
      {
        name: "Bb3",
        freq: 233.08,
        key: "2",
        angle: Math.PI * 0.5,
        radius: 110,
      },
      {
        name: "C#4",
        freq: 277.18,
        key: "3",
        angle: Math.PI * 0.25,
        radius: 110,
      },
      { name: "D4", freq: 293.66, key: "4", angle: 0, radius: 110 },
      {
        name: "E4",
        freq: 329.63,
        key: "6",
        angle: Math.PI * 1.75,
        radius: 110,
      },
      { name: "F4", freq: 349.23, key: "7", angle: Math.PI * 1.5, radius: 110 },
      { name: "G4", freq: 392.0, key: "8", angle: Math.PI * 1.25, radius: 110 },
      { name: "A4", freq: 440.0, key: "9", angle: Math.PI, radius: 110 },
    ],
  },
];

interface Ripple {
  id: number;
  x: number;
  y: number;
  radius: number;
  maxRadius: number;
  color: string;
  opacity: number;
}

class HandpanAudioEngine {
  private ctx: AudioContext | null = null;
  private masterGain: GainNode | null = null;
  private delayNode: DelayNode | null = null;
  private feedbackGain: GainNode | null = null;
  private filterNode: BiquadFilterNode | null = null;

  // Settings variables
  private ringTimeMultiplier: number = 1.0;
  private strikeHardness: number = 0.8;
  private reverbMix: number = 0.35;

  constructor() {}

  setParams(ringTime: number, hardness: number, reverb: number) {
    this.ringTimeMultiplier = ringTime;
    this.strikeHardness = hardness;
    this.reverbMix = reverb;

    if (this.feedbackGain && this.ctx) {
      // scale feedback based on reverb mix
      this.feedbackGain.gain.setValueAtTime(reverb * 0.7, this.ctx.currentTime);
    }
  }

  init() {
    if (this.ctx) return;
    this.ctx = new (
      window.AudioContext || (window as any).webkitAudioContext
    )();

    this.masterGain = this.ctx.createGain();
    this.masterGain.gain.setValueAtTime(0.7, this.ctx.currentTime);
    this.masterGain.connect(this.ctx.destination);

    // Create mystical echo/feedback delay network for spacious reverb
    this.delayNode = this.ctx.createDelay(1.0);
    this.delayNode.delayTime.setValueAtTime(0.38, this.ctx.currentTime); // 380ms delay

    this.feedbackGain = this.ctx.createGain();
    this.feedbackGain.gain.setValueAtTime(
      this.reverbMix * 0.7,
      this.ctx.currentTime,
    );

    this.filterNode = this.ctx.createBiquadFilter();
    this.filterNode.type = "lowpass";
    this.filterNode.frequency.setValueAtTime(1200, this.ctx.currentTime); // smooth high frequencies

    // Wire up feedback loop: master -> delay -> filter -> feedback -> delay
    this.masterGain.connect(this.delayNode);
    this.delayNode.connect(this.filterNode);
    this.filterNode.connect(this.feedbackGain);
    this.feedbackGain.connect(this.delayNode);

    // Wire output of feedback loop to destination
    const reverbWetGain = this.ctx.createGain();
    reverbWetGain.gain.setValueAtTime(0.4, this.ctx.currentTime);
    this.feedbackGain.connect(reverbWetGain);
    reverbWetGain.connect(this.ctx.destination);
  }

  playNote(frequency: number) {
    this.init();
    if (!this.ctx || !this.masterGain) return;

    if (this.ctx.state === "suspended") {
      this.ctx.resume();
    }

    const now = this.ctx.currentTime;
    const force = this.strikeHardness;

    // 1. Physical Tap/Strike Transient
    // Finger pad strike simulation (lowpass filtered noise burst)
    const noiseBuffer = this.ctx.createBuffer(
      1,
      this.ctx.sampleRate * 0.015,
      this.ctx.sampleRate,
    );
    const channelData = noiseBuffer.getChannelData(0);
    for (let i = 0; i < noiseBuffer.length; i++) {
      channelData[i] = Math.random() * 2 - 1;
    }

    const noiseSource = this.ctx.createBufferSource();
    noiseSource.buffer = noiseBuffer;

    const noiseFilter = this.ctx.createBiquadFilter();
    noiseFilter.type = "bandpass";
    // Strike pitch is slightly higher than fundamental
    noiseFilter.frequency.setValueAtTime(frequency * 1.4, now);
    noiseFilter.Q.setValueAtTime(3.0, now);

    const noiseGain = this.ctx.createGain();
    noiseGain.gain.setValueAtTime(0.12 * force, now);
    noiseGain.gain.exponentialRampToValueAtTime(0.0001, now + 0.012);

    noiseSource.connect(noiseFilter);
    noiseFilter.connect(noiseGain);
    noiseGain.connect(this.masterGain);
    noiseSource.start(now);

    // 2. Resonant Harmonics (Tuned Handpan Ratios)
    // Fundamental ($f_0$), Octave ($2f_0$), and Compound Fifth ($3f_0$)
    const partials = [
      { ratio: 1.0, gain: 0.85, decay: 2.4 * this.ringTimeMultiplier },
      { ratio: 2.0, gain: 0.45, decay: 1.5 * this.ringTimeMultiplier },
      { ratio: 3.0, gain: 0.22, decay: 0.9 * this.ringTimeMultiplier },
    ];

    partials.forEach((p) => {
      if (!this.ctx || !this.masterGain) return;
      const osc = this.ctx.createOscillator();
      osc.type = "sine";
      osc.frequency.setValueAtTime(frequency * p.ratio, now);

      // Add extremely subtle vibrato/pulsing on fundamental to mimic metal warp
      if (p.ratio === 1.0) {
        const vibrato = this.ctx.createOscillator();
        vibrato.frequency.setValueAtTime(3.5, now); // 3.5Hz pulse
        const vibratoGain = this.ctx.createGain();
        vibratoGain.gain.setValueAtTime(1.5, now); // 1.5Hz variance
        vibrato.connect(vibratoGain);
        vibratoGain.connect(osc.frequency);
        vibrato.start(now);
        vibrato.stop(now + p.decay + 0.1);
      }

      const gain = this.ctx.createGain();
      gain.gain.setValueAtTime(0.0, now);
      // Soft finger strike attack (approx 6-8ms)
      gain.gain.linearRampToValueAtTime(p.gain * force * 0.45, now + 0.006);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + p.decay);

      osc.connect(gain);
      gain.connect(this.masterGain);

      osc.start(now);
      osc.stop(now + p.decay + 0.1);
    });
  }
}

export const HandpanSettings: React.FC = () => {
  const { t } = useTranslation();
  const [selectedScale, setSelectedScale] = useState<HandpanScale>(
    HANDPAN_SCALES[0],
  );
  const [ringTime, setRingTime] = useState(1.0);
  const [hardness, setHardness] = useState(0.8);
  const [reverb, setReverb] = useState(0.35);

  // Note trigger tracking for styling
  const [activeNoteKey, setActiveNoteKey] = useState<string | null>(null);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const ripplesRef = useRef<Ripple[]>([]);
  const synthRef = useRef<HandpanAudioEngine | null>(null);
  const animationFrameRef = useRef<number | null>(null);

  // Initialize Audio Engine
  useEffect(() => {
    synthRef.current = new HandpanAudioEngine();
    synthRef.current.setParams(ringTime, hardness, reverb);

    // Start canvas animation loop
    const canvas = canvasRef.current;
    if (canvas) {
      const ctx = canvas.getContext("2d");
      if (ctx) {
        const render = () => {
          ctx.clearRect(0, 0, canvas.width, canvas.height);

          // Draw and decay ripples
          ripplesRef.current = ripplesRef.current.filter((r) => {
            r.radius += (r.maxRadius - r.radius) * 0.08;
            r.opacity -= 0.02;

            if (r.opacity <= 0) return false;

            ctx.beginPath();
            ctx.arc(r.x, r.y, r.radius, 0, Math.PI * 2);
            ctx.strokeStyle = r.color
              .replace(")", `, ${r.opacity})`)
              .replace("rgb", "rgba");
            ctx.lineWidth = 2.5;
            ctx.stroke();
            return true;
          });

          animationFrameRef.current = requestAnimationFrame(render);
        };
        render();
      }
    }

    return () => {
      if (animationFrameRef.current)
        cancelAnimationFrame(animationFrameRef.current);
    };
  }, []);

  // Update Synth parameters on slider change
  useEffect(() => {
    synthRef.current?.setParams(ringTime, hardness, reverb);
  }, [ringTime, hardness, reverb]);

  // Handle Keyboard Triggers
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const note = selectedScale.notes.find((n) => n.key === e.key);
      if (note) {
        triggerNote(note);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedScale]);

  const triggerNote = (note: HandpanNote) => {
    // Play sound
    synthRef.current?.playNote(note.freq);

    // Trigger visual style animation
    setActiveNoteKey(note.key);
    setTimeout(() => setActiveNoteKey(null), 150);

    // Add ripple effect
    const canvas = canvasRef.current;
    if (canvas) {
      const rect = canvas.getBoundingClientRect();
      const centerX = canvas.width / 2;
      const centerY = canvas.height / 2;

      // Calculate absolute position based on angle and radius
      // Scale coordinates from design layout (110px radius) to canvas scale (approx 360px total width)
      const scaleMultiplier = 1.3;
      const x = centerX + Math.cos(note.angle) * note.radius * scaleMultiplier;
      const y = centerY - Math.sin(note.angle) * note.radius * scaleMultiplier;

      const colors = [
        "rgb(99, 102, 241)", // Indigo
        "rgb(168, 85, 247)", // Purple
        "rgb(236, 72, 153)", // Pink
        "rgb(59, 130, 246)", // Blue
        "rgb(34, 197, 94)", // Green
      ];
      // Assign color based on frequency/note position
      const color =
        note.radius === 0
          ? "rgb(236, 72, 153)"
          : colors[parseInt(note.key) % colors.length];

      ripplesRef.current.push({
        id: Date.now() + Math.random(),
        x,
        y,
        radius: 5,
        maxRadius: note.radius === 0 ? 120 : 60,
        color,
        opacity: 0.8,
      });
    }
  };

  const handleScaleChange = (id: string) => {
    const scale = HANDPAN_SCALES.find((s) => s.id === id);
    if (scale) setSelectedScale(scale);
  };

  // UI coordinate maps for buttons (matches coordinates on canvas)
  const getNotePositionStyles = (note: HandpanNote) => {
    if (note.radius === 0) {
      // Ding (Center note)
      return {
        left: "50%",
        top: "50%",
        transform: "translate(-50%, -50%)",
      };
    }

    // Surrounding notes (convert polar to CSS percentage layout)
    const distancePercent = 38; // Radius distance in percentages from center
    const x = 50 + Math.cos(note.angle) * distancePercent;
    const y = 50 - Math.sin(note.angle) * distancePercent;

    return {
      left: `${x}%`,
      top: `${y}%`,
      transform: "translate(-50%, -50%)",
    };
  };

  return (
    <div className="w-full max-w-4xl mx-auto flex flex-col gap-6 animate-in fade-in duration-500">
      {/* Main Container */}
      <div className="relative overflow-hidden rounded-3xl border border-surface-raised bg-surface/30 backdrop-blur-2xl p-6 sm:p-8 flex flex-col md:flex-row gap-8 items-center justify-between shadow-2xl">
        {/* Glow Spheres in the Background */}
        <div className="absolute top-1/2 left-1/4 -translate-x-1/2 -translate-y-1/2 w-80 h-80 rounded-full bg-accent/5 blur-[100px] pointer-events-none" />
        <div className="absolute bottom-10 right-10 w-60 h-60 rounded-full bg-accent/5 blur-[80px] pointer-events-none" />

        {/* Left Control Panel */}
        <div className="w-full md:w-80 flex flex-col gap-6 z-10">
          <div className="flex items-center gap-2.5">
            <div className="p-2.5 rounded-2xl bg-accent/10 border border-accent/20 text-accent">
              <Compass className="w-5 h-5 animate-spin-slow" />
            </div>
            <div>
              <h2 className="text-xl font-black text-white tracking-tight leading-none">
                {t("sidebar.handpan")}
              </h2>
              <span className="text-[10px] text-secondary font-bold uppercase tracking-wider">
                {t("settings.handpan.subtitle")}
              </span>
            </div>
          </div>

          {/* Scale Selector */}
          <div className="flex flex-col gap-2">
            <label className="text-[10px] font-bold text-secondary uppercase tracking-widest ml-1">
              {t("settings.handpan.selectScale")}
            </label>
            <select
              value={selectedScale.id}
              onChange={(e) => handleScaleChange(e.target.value)}
              className="text-xs font-semibold bg-ground/80 border border-surface-raised rounded-xl px-3 py-2.5 text-primary focus:outline-none focus:ring-2 focus:ring-accent"
            >
              {HANDPAN_SCALES.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </div>

          {/* Audio Adjustments */}
          <div className="flex flex-col gap-4 bg-ground/20 border border-surface-raised/40 rounded-2xl p-4">
            {/* Ring Time */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-[10px] font-bold text-secondary">
                <span className="uppercase tracking-wider">
                  {t("settings.handpan.ringTime")}
                </span>
                <span>{`${ringTime.toFixed(1)}x`}</span>
              </div>
              <input
                type="range"
                min="0.4"
                max="2.0"
                step="0.1"
                value={ringTime}
                onChange={(e) => setRingTime(parseFloat(e.target.value))}
                className="w-full accent-accent h-1 bg-surface-raised rounded-lg appearance-none cursor-pointer"
              />
            </div>

            {/* Hardness */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-[10px] font-bold text-secondary">
                <span className="uppercase tracking-wider">
                  {t("settings.handpan.hardness")}
                </span>
                <span>{Math.round(hardness * 100)}%</span>
              </div>
              <input
                type="range"
                min="0.3"
                max="1.2"
                step="0.05"
                value={hardness}
                onChange={(e) => setHardness(parseFloat(e.target.value))}
                className="w-full accent-accent h-1 bg-surface-raised rounded-lg appearance-none cursor-pointer"
              />
            </div>

            {/* Reverb Space */}
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-[10px] font-bold text-secondary">
                <span className="uppercase tracking-wider">
                  {t("settings.handpan.reverb")}
                </span>
                <span>{Math.round(reverb * 100)}%</span>
              </div>
              <input
                type="range"
                min="0.0"
                max="0.8"
                step="0.05"
                value={reverb}
                onChange={(e) => setReverb(parseFloat(e.target.value))}
                className="w-full accent-accent h-1 bg-surface-raised rounded-lg appearance-none cursor-pointer"
              />
            </div>
          </div>

          {/* Quick Help */}
          <div
            className={`flex gap-2.5 items-start bg-accent/5 rounded-2xl border border-accent/10 p-3
              text-secondary text-xs`}
          >
            <HelpCircle className="w-4 h-4 text-accent shrink-0 mt-0.5" />
            <p className="leading-normal">{t("settings.handpan.help")}</p>
          </div>
        </div>

        {/* Right Handpan Instrument UI */}
        <div className="relative w-80 sm:w-96 h-80 sm:h-96 shrink-0 flex items-center justify-center select-none">
          {/* Canvas for ripples (rendered behind buttons) */}
          <canvas
            ref={canvasRef}
            width={420}
            height={420}
            className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[110%] h-[110%] pointer-events-none z-0"
          />

          {/* Main Handpan Shell Circle */}
          <div className="absolute w-[280px] sm:w-[320px] h-[280px] sm:h-[320px] rounded-full border border-edge bg-gradient-to-br from-surface via-ground to-surface shadow-inner flex items-center justify-center overflow-hidden z-10">
            <div className="w-[90%] h-[90%] rounded-full border border-surface-raised/60 shadow-md opacity-30 pointer-events-none" />
          </div>

          {/* Note Fields mapping */}
          {selectedScale.notes.map((note) => {
            const isDing = note.radius === 0;
            const isActive = activeNoteKey === note.key;

            return (
              <button
                key={note.key}
                style={getNotePositionStyles(note)}
                onClick={() => triggerNote(note)}
                className={`absolute z-20 rounded-full flex flex-col items-center justify-center border cursor-pointer select-none transition-all duration-150 ${
                  isDing
                    ? "w-24 h-24 sm:w-28 sm:h-28 bg-gradient-to-br from-surface-raised via-surface to-surface-raised/80 border-edge shadow-md hover:border-accent"
                    : "w-16 h-16 sm:w-20 sm:h-20 bg-gradient-to-br from-ground via-surface to-ground border-surface-raised shadow hover:border-accent"
                } ${
                  isActive
                    ? "scale-90 border-accent bg-accent/10 shadow-[0_0_15px_var(--color-accent-glow-soft)]"
                    : ""
                }`}
              >
                {/* Visual central dimple (Gu) */}
                <div
                  className={`rounded-full bg-ground shadow-inner border border-surface-raised/30 mb-0.5 ${isDing ? "w-7 h-7 sm:w-8 sm:h-8" : "w-4 h-4 sm:w-5 sm:h-5"}`}
                />

                {/* Note details */}
                <span className="text-[10px] font-extrabold text-primary tracking-tight">
                  {note.name}
                </span>

                {/* Keyboard hotkey label */}
                <span className="text-[8px] font-bold text-secondary uppercase tracking-widest mt-0.5">
                  {t("settings.handpan.keyLabel", { k: note.key })}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
};
