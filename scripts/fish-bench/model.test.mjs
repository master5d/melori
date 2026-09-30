import { describe, it, expect } from "vitest";
import { buildCreateModelRequest, assertPrivate } from "./model.mjs";

function make() {
  return buildCreateModelRequest({
    apiKey: "SECRET-XYZ",
    title: "echo-owner-private",
    wavBytes: new Uint8Array([82, 73, 70, 70]),
    wavName: "ref.wav",
  });
}

describe("buildCreateModelRequest", () => {
  it("целится в /model с Bearer-авторизацией", () => {
    const { url, headers } = make();
    expect(url).toBe("https://api.fish.audio/model");
    expect(headers.Authorization).toBe("Bearer SECRET-XYZ");
  });

  it("visibility=private, type=tts, train_mode=fast", () => {
    const { form } = make();
    expect(form.get("visibility")).toBe("private");
    expect(form.get("type")).toBe("tts");
    expect(form.get("train_mode")).toBe("fast");
    expect(form.get("title")).toBe("echo-owner-private");
    expect(form.get("voices").name).toBe("ref.wav");
  });

  it("assertPrivate пропускает private и бросает на всём остальном", () => {
    const { form } = make();
    expect(() => assertPrivate(form)).not.toThrow();
    form.set("visibility", "public");
    expect(() => assertPrivate(form)).toThrow(/private/);
    form.delete("visibility");
    expect(() => assertPrivate(form)).toThrow(/private/);
  });
});
