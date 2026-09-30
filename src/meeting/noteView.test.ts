import { describe, expect, it } from "vitest";
import type { ClientNote } from "@/bindings";
import { noteCopyText, noteRows } from "./NoteView";

describe("note view helpers", () => {
  const note: ClientNote = {
    n: 1,
    template_id: "dap",
    sections: [
      ["data", "Данные"],
      ["plan", "План"],
    ],
    fields: { data: "Факты", plan: "Шаги" },
    stored: true,
  };

  it("maps [key, title] sections to title and field text", () => {
    expect(noteRows(note)).toEqual([
      ["Данные", "Факты"],
      ["План", "Шаги"],
    ]);
  });

  it("copies the same section order and text", () => {
    expect(noteCopyText(note)).toBe("Данные:\nФакты\n\nПлан:\nШаги");
  });
});
