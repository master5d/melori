import { describe, it, expect } from "vitest";
import { SCENARIOS, buildTtsRequest } from "./scenarios.mjs";

describe("SCENARIOS", () => {
  it("содержит S1–S6 с непустыми текстами", () => {
    expect(SCENARIOS.map((s) => s.id)).toEqual([
      "S1",
      "S2",
      "S3",
      "S4",
      "S5",
      "S6",
    ]);
    for (const s of SCENARIOS) expect(s.text.length).toBeGreaterThan(20);
  });
  it("S2 достаточно длинный для longform-оценки (>1000 символов)", () => {
    expect(SCENARIOS.find((s) => s.id === "S2").text.length).toBeGreaterThan(
      1000,
    );
  });
});

describe("buildTtsRequest", () => {
  const args = {
    apiKey: "SECRET-XYZ",
    model: "s2.1-pro",
    refId: "r1",
    text: "привет",
    format: "wav",
    latency: "normal",
  };
  it("собирает URL, заголовки и JSON-тело", () => {
    const { url, headers, body } = buildTtsRequest(args);
    expect(url).toBe("https://api.fish.audio/v1/tts");
    expect(headers.Authorization).toBe("Bearer SECRET-XYZ");
    expect(headers.model).toBe("s2.1-pro");
    expect(headers["Content-Type"]).toBe("application/json");
    const parsed = JSON.parse(body);
    expect(parsed).toMatchObject({
      text: "привет",
      reference_id: "r1",
      format: "wav",
      latency: "normal",
    });
  });
  it("ключ не попадает в тело", () => {
    expect(buildTtsRequest(args).body).not.toContain("SECRET-XYZ");
  });
});
