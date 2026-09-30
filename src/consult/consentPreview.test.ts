import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/consent_preview.json";
import { consentPreview } from "./consentPreview";
import type { ConsentFormState } from "./consentForm";

describe("consent preview", () => {
  for (const [index, item] of fixture.entries()) {
    it(`matches the engine fixture ${index + 1} byte-for-byte`, () => {
      expect(consentPreview(item.form as ConsentFormState, item.date)).toBe(
        item.text,
      );
    });
  }

  it("marks permissions in engine order", () => {
    const form: ConsentFormState = {
      alias: "Тест",
      consentDate: "2026-09-28",
      retainDays: 30,
      permissions: {
        transcript: true,
        video: false,
        council: true,
        retain: true,
      },
    };
    const text = consentPreview(form, form.consentDate);
    expect(text.indexOf("[x] Транскрипт")).toBeLessThan(
      text.indexOf("[ ] Видео"),
    );
    expect(text).toContain("[x] Разбор сессии");
    expect(text).toContain("[x] Хранение");
  });

  it("describes non-retention and until-revocation retention", () => {
    const base: ConsentFormState = {
      alias: "Тест",
      consentDate: "2026-09-28",
      retainDays: null,
      permissions: {
        transcript: true,
        video: false,
        council: false,
        retain: false,
      },
    };
    expect(consentPreview(base, base.consentDate)).toContain("не сохраняется");
    expect(
      consentPreview(
        { ...base, permissions: { ...base.permissions, retain: true } },
        base.consentDate,
      ),
    ).toContain("до отзыва согласия");
  });
  it("does not re-substitute placeholders that appear inside the alias", () => {
    const form: ConsentFormState = {
      alias: "{date}",
      consentDate: "2026-09-28",
      retainDays: null,
      permissions: {
        transcript: true,
        video: false,
        council: false,
        retain: false,
      },
    };
    expect(consentPreview(form, form.consentDate)).toContain(
      "Клиент (псевдоним): {date}",
    );
  });
});
