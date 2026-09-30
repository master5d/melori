import consentTemplate from "../../engine/melori_engine/templates/consent-v1.md?raw";
import { PERMISSIONS, type ConsentFormState } from "./consentForm";

const LABELS: Record<(typeof PERMISSIONS)[number], string> = {
  transcript: "Транскрипт разговора (текст) / Conversation transcript",
  video:
    "Видео-анализ поведения и выражения лица (числовые ряды) / Video behaviour & facial-expression analysis",
  council:
    "Разбор сессии языковой моделью / Session analysis by a language model",
  retain:
    "Хранение материалов после сессии / Keeping session material afterwards",
};

export function consentPreview(form: ConsentFormState, date: string): string {
  const permissionLines = PERMISSIONS.map(
    (permission) =>
      `- [${form.permissions[permission] ? "x" : " "}] ${LABELS[permission]}`,
  ).join("\n");
  const retention = !form.permissions.retain
    ? "не сохраняется — после сессии материалы удаляются"
    : form.retainDays === null
      ? "до отзыва согласия"
      : `${form.retainDays} дней после сессии`;
  // один проход, как str.format в движке: подставленное значение повторно не разбирается
  const values: Record<string, string> = {
    alias: form.alias,
    date,
    permission_lines: permissionLines,
    retention_line: retention,
  };
  return consentTemplate.replace(
    /\{(alias|date|permission_lines|retention_line)\}/g,
    (_, key: string) => values[key],
  );
}
