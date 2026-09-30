import { describe, expect, it } from "vitest";
import i18next from "i18next";
import ru from "./locales/ru/translation.json";
import en from "./locales/en/translation.json";

// The real locale files through a real i18next: Russian needs one/few/many forms
// ("1 сессия", "2 сессии", "5 сессий"), which only Intl.PluralRules picks correctly.
async function tFor(lng: "ru" | "en") {
  const i = i18next.createInstance();
  await i.init({
    lng,
    fallbackLng: "en",
    resources: { ru: { translation: ru }, en: { translation: en } },
  });
  return i.t.bind(i);
}

describe("plural forms", () => {
  it("Russian sessions, days, lines and clients follow the number", async () => {
    const t = await tFor("ru");
    const forms = (key: string) =>
      [1, 2, 5, 11, 21, 22].map((count) => t(key, { count }));
    expect(forms("consult.card.sessionsCount")).toEqual([
      "1 сессия",
      "2 сессии",
      "5 сессий",
      "11 сессий",
      "21 сессия",
      "22 сессии",
    ]);
    expect(forms("consult.memory.daysAgo")).toEqual([
      "1 день назад",
      "2 дня назад",
      "5 дней назад",
      "11 дней назад",
      "21 день назад",
      "22 дня назад",
    ]);
    expect(forms("consult.session.provenance.transcriptInput")[0]).toBe(
      "транскрипт, 1 реплика",
    );
    expect(forms("consult.session.provenance.transcriptInput")[2]).toBe(
      "транскрипт, 5 реплик",
    );
    expect(
      t("settings.melori.templates.deleteConflict", { count: 1 }),
    ).toContain("за 1 клиентом");
    expect(
      t("settings.melori.templates.deleteConflict", { count: 3 }),
    ).toContain("за 3 клиентами");
    expect(t("consult.card.deleteConfirm", { count: 2 })).toBe(
      "Удалить 2 сессии безвозвратно?",
    );
  });

  it("English has one and other", async () => {
    const t = await tFor("en");
    expect(t("consult.card.sessionsCount", { count: 1 })).toBe("1 session");
    expect(t("consult.card.sessionsCount", { count: 3 })).toBe("3 sessions");
  });
});
