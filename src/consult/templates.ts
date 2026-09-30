export interface TemplateSection {
  key: string;
  title: string;
  guidance: string;
}
export interface TemplateDraft {
  name: string;
  sections: TemplateSection[];
}
const keyPattern = /^[a-z][a-z0-9_]{0,31}$/;
const translit: Record<string, string> = {
  а: "a",
  б: "b",
  в: "v",
  г: "g",
  д: "d",
  е: "e",
  ё: "e",
  ж: "zh",
  з: "z",
  и: "i",
  й: "j",
  к: "k",
  л: "l",
  м: "m",
  н: "n",
  о: "o",
  п: "p",
  р: "r",
  с: "s",
  т: "t",
  у: "u",
  ф: "f",
  х: "h",
  ц: "c",
  ч: "ch",
  ш: "sh",
  щ: "shch",
  ъ: "",
  ы: "y",
  ь: "",
  э: "e",
  ю: "yu",
  я: "ya",
};
export function slugKey(title: string, n = 1): string {
  const text = [...title.toLowerCase()].map((c) => translit[c] ?? c).join("");
  const slug = text
    .replace(/[^a-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 32);
  return /^[a-z]/.test(slug) ? slug : `section_${n}`;
}
export function validateTemplateDraft(draft: TemplateDraft): string[] {
  const errors: string[] = [];
  if (draft.name.trim().length < 1 || draft.name.trim().length > 60)
    errors.push("name");
  if (draft.sections.length === 0) errors.push("empty");
  if (draft.sections.length > 12) errors.push("tooMany");
  const seen = new Set<string>();
  draft.sections.forEach((s, i) => {
    if (!keyPattern.test(s.key)) errors.push(`key:${i}`);
    if (s.title.trim().length < 1 || s.title.length > 60)
      errors.push(`title:${i}`);
    if (s.guidance.length > 400) errors.push(`guidance:${i}`);
    if (seen.has(s.key)) errors.push(`duplicate:${i}`);
    seen.add(s.key);
  });
  return errors;
}
