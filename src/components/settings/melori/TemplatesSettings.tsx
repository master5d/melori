import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Lock } from "lucide-react";
import {
  consultApi,
  TemplateDeleteConflictError,
  type NoteTemplate,
} from "@/consult/api";
import {
  slugKey,
  validateTemplateDraft,
  type TemplateDraft,
  type TemplateSection,
} from "@/consult/templates";

const newSection = (n: number): TemplateSection => ({
  title: "",
  key: slugKey("", n),
  guidance: "",
});

export default function TemplatesSettings() {
  const { t, i18n } = useTranslation();
  const [templates, setTemplates] = useState<NoteTemplate[]>([]);
  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<{
    id: string;
    clients: number;
  } | null>(null);
  const load = () =>
    consultApi
      .templates(i18n.language)
      .then(setTemplates)
      .catch(() => setTemplates([]));
  useEffect(() => void load(), []);

  const begin = (template?: NoteTemplate) => {
    setErrors([]);
    setError(null);
    setEditingId(template && !template.builtin ? template.id : null);
    setDraft({
      name: template
        ? `${template.name}${template.builtin ? " copy" : ""}`
        : "",
      sections: template?.sections.map((s) => ({ ...s })) ?? [newSection(1)],
    });
  };
  const changeSection = (index: number, patch: Partial<TemplateSection>) =>
    setDraft(
      (current) =>
        current && {
          ...current,
          sections: current.sections.map((s, i) =>
            i === index ? { ...s, ...patch } : s,
          ),
        },
    );
  const save = async () => {
    if (!draft) return;
    const next = validateTemplateDraft(draft);
    setErrors(next);
    if (next.length) return;
    try {
      const saved = editingId
        ? await consultApi.updateTemplate(editingId, draft)
        : await consultApi.createTemplate(draft);
      setTemplates((current) =>
        editingId
          ? current.map((item) => (item.id === saved.id ? saved : item))
          : [...current, saved],
      );
      setDraft(null);
      setEditingId(null);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  const remove = async (id: string, force = false) => {
    try {
      await consultApi.deleteTemplate(id, force);
      setTemplates((current) => current.filter((item) => item.id !== id));
      setConflict(null);
    } catch (cause) {
      if (cause instanceof TemplateDeleteConflictError)
        setConflict({ id, clients: cause.clients });
      else
        setError(
          cause instanceof Error ? cause.message : t("consult.errors.request"),
        );
    }
  };
  const errorText = (kind: string) =>
    t(`settings.melori.templates.errors.${kind.split(":")[0]}`);
  return (
    <section className="flex flex-col gap-5 p-6">
      <h1 className="font-serif text-3xl text-primary">
        {t("settings.melori.templates.title")}
      </h1>
      {error && (
        <p role="alert" className="text-err">
          {error}
        </p>
      )}
      {conflict && (
        <div role="alertdialog" className="border border-err p-4 text-primary">
          <p>
            {t("settings.melori.templates.deleteConflict", {
              count: conflict.clients,
            })}
          </p>
          <div className="mt-3 flex gap-3">
            <button
              className="border border-err px-3 py-2 text-err"
              onClick={() => void remove(conflict.id, true)}
            >
              {t("settings.melori.templates.deleteConfirm")}
            </button>
            <button
              className="border-b border-rule-strong"
              onClick={() => setConflict(null)}
            >
              {t("consult.actions.cancel")}
            </button>
          </div>
        </div>
      )}
      {draft ? (
        <div className="flex flex-col gap-4 border-b border-rule-strong pb-5">
          <label className="flex flex-col gap-1 text-sm text-secondary">
            {t("settings.melori.templates.name")}
            <input
              className="h-11 border-b border-rule-strong bg-transparent text-primary"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
          </label>
          {errors.includes("name") && (
            <p className="text-err">{errorText("name")}</p>
          )}
          {draft.sections.map((section, index) => (
            <fieldset
              key={index}
              className="flex flex-col gap-2 border-b border-rule pb-4"
            >
              <legend className="font-serif text-lg text-primary">
                {t("settings.melori.templates.section", { n: index + 1 })}
              </legend>
              <label className="text-sm text-secondary">
                {t("settings.melori.templates.titleField")}
                <input
                  className="mt-1 h-10 w-full border-b border-rule-strong bg-transparent text-primary"
                  value={section.title}
                  onChange={(e) =>
                    changeSection(index, {
                      title: e.target.value,
                      key:
                        section.key === slugKey(section.title, index + 1)
                          ? slugKey(e.target.value, index + 1)
                          : section.key,
                    })
                  }
                />
              </label>
              <label className="text-sm text-secondary">
                {t("settings.melori.templates.key")}
                <input
                  className="mt-1 h-10 w-full border-b border-rule-strong bg-transparent font-mono text-primary"
                  value={section.key}
                  onChange={(e) =>
                    changeSection(index, { key: e.target.value })
                  }
                />
              </label>
              <label className="text-sm text-secondary">
                {t("settings.melori.templates.guidance")}
                <textarea
                  className="mt-1 min-h-16 w-full border-b border-rule-strong bg-transparent text-primary"
                  value={section.guidance}
                  onChange={(e) =>
                    changeSection(index, { guidance: e.target.value })
                  }
                />
              </label>
              {errors.some((item) => item.endsWith(`:${index}`)) && (
                <p className="text-err">
                  {errors
                    .filter((item) => item.endsWith(`:${index}`))
                    .map(errorText)
                    .join(" ")}
                </p>
              )}
              <div className="flex gap-3 text-sm">
                <button
                  disabled={index === 0}
                  className="border-b border-rule-strong disabled:opacity-40"
                  onClick={() =>
                    setDraft({
                      ...draft,
                      sections: draft.sections.map((item, i) =>
                        i === index - 1
                          ? section
                          : i === index
                            ? draft.sections[i - 1]
                            : item,
                      ),
                    })
                  }
                >
                  {t("settings.melori.templates.up")}
                </button>
                <button
                  disabled={index === draft.sections.length - 1}
                  className="border-b border-rule-strong disabled:opacity-40"
                  onClick={() =>
                    setDraft({
                      ...draft,
                      sections: draft.sections.map((item, i) =>
                        i === index + 1
                          ? section
                          : i === index
                            ? draft.sections[i + 1]
                            : item,
                      ),
                    })
                  }
                >
                  {t("settings.melori.templates.down")}
                </button>
                <button
                  className="border-b border-err text-err"
                  onClick={() =>
                    setDraft({
                      ...draft,
                      sections: draft.sections.filter((_, i) => i !== index),
                    })
                  }
                >
                  {t("settings.melori.templates.remove")}
                </button>
              </div>
            </fieldset>
          ))}
          {errors
            .filter((item) => ["empty", "tooMany"].includes(item))
            .map((item) => (
              <p key={item} className="text-err">
                {errorText(item)}
              </p>
            ))}
          {errors.some((item) => item.startsWith("duplicate:")) && (
            <p className="text-err">{errorText("duplicate")}</p>
          )}
          <div className="flex gap-3">
            <button
              className="h-11 bg-accent px-5 text-on-accent"
              onClick={() =>
                setDraft({
                  ...draft,
                  sections: [
                    ...draft.sections,
                    newSection(draft.sections.length + 1),
                  ],
                })
              }
            >
              {t("settings.melori.templates.addSection")}
            </button>
            <button
              className="h-11 border border-rule-strong px-5 text-primary"
              onClick={() => void save()}
            >
              {t("settings.melori.templates.save")}
            </button>
            <button
              className="h-11 border-b border-rule-strong px-2 text-primary"
              onClick={() => setDraft(null)}
            >
              {t("consult.actions.cancel")}
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="flex flex-col gap-3">
            {templates
              .filter((item) => item.id !== "free")
              .map((template) => (
                <article
                  className="flex items-center justify-between border-b border-rule-strong py-3"
                  key={template.id}
                >
                  <span className="font-serif text-lg text-primary">
                    {template.name}
                  </span>
                  <div className="flex gap-4 text-sm">
                    {template.builtin ? (
                      <>
                        <Lock
                          aria-label={t("settings.melori.templates.locked")}
                          className="h-4 w-4 text-secondary"
                        />
                        <button
                          className="border-b border-rule-strong text-primary"
                          onClick={() => begin(template)}
                        >
                          {t("settings.melori.templates.copy")}
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          className="border-b border-rule-strong text-primary"
                          onClick={() => begin(template)}
                        >
                          {t("settings.melori.templates.edit")}
                        </button>
                        <button
                          className="border-b border-err text-err"
                          onClick={() => void remove(template.id)}
                        >
                          {t("settings.melori.templates.remove")}
                        </button>
                      </>
                    )}
                  </div>
                </article>
              ))}
          </div>
          <button
            className="h-11 self-start border border-rule-strong px-5 text-primary"
            onClick={() => begin()}
          >
            {t("settings.melori.templates.new")}
          </button>
        </>
      )}
    </section>
  );
}
