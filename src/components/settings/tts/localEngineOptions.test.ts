import { describe, expect, it } from "vitest";
import {
  LOCAL_ENGINES,
  SUPERTONIC_VOICES,
  showSupertonicVoice,
} from "./localEngineOptions";

describe("localEngineOptions", () => {
  it("has exactly two engines with supertonic first", () => {
    expect(LOCAL_ENGINES).toEqual(["supertonic", "piper"]);
  });

  it("has ten voices M1..F5", () => {
    expect(SUPERTONIC_VOICES).toHaveLength(10);
    expect(SUPERTONIC_VOICES).toContain("M1");
    expect(SUPERTONIC_VOICES).toContain("F5");
  });

  it("shows voice dropdown only for supertonic", () => {
    expect(showSupertonicVoice("supertonic")).toBe(true);
    expect(showSupertonicVoice("piper")).toBe(false);
    expect(showSupertonicVoice(undefined)).toBe(true); // default is supertonic
  });
});
