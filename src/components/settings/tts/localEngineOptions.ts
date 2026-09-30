export const LOCAL_ENGINES = ["supertonic", "piper"] as const;
export type LocalEngine = (typeof LOCAL_ENGINES)[number];

export const SUPERTONIC_VOICES = [
  "M1",
  "M2",
  "M3",
  "M4",
  "M5",
  "F1",
  "F2",
  "F3",
  "F4",
  "F5",
] as const;

/** Voice picker is Supertonic-specific; default engine is supertonic. */
export function showSupertonicVoice(engine: string | undefined): boolean {
  return (engine ?? "supertonic") === "supertonic";
}
