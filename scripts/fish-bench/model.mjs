const MODEL_URL = "https://api.fish.audio/model";

export function buildCreateModelRequest({ apiKey, title, wavBytes, wavName }) {
  const form = new FormData();
  form.set("type", "tts");
  form.set("train_mode", "fast");
  form.set("title", title);
  form.set("visibility", "private");
  form.set("voices", new File([wavBytes], wavName, { type: "audio/wav" }));

  return {
    url: MODEL_URL,
    headers: { Authorization: `Bearer ${apiKey}` },
    form,
  };
}

export function assertPrivate(form) {
  if (form.get("visibility") !== "private") {
    throw new Error(
      "refusing to upload voice reference: visibility must be private (Fish default is PUBLIC and public grants are irrevocable per their Terms)",
    );
  }
}
