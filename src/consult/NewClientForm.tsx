import { useState } from "react";
import { useTranslation } from "react-i18next";
import { consultApi } from "./api";
import {
  buildCreatePayload,
  PERMISSIONS,
  validateForm,
  type ConsentFormState,
  type Permission,
} from "./consentForm";
import { consentPreview } from "./consentPreview";

interface Props {
  onCreated: () => void;
  onCancel: () => void;
}

export function NewClientForm({ onCreated, onCancel }: Props) {
  const { t } = useTranslation();
  const [form, setForm] = useState<ConsentFormState>({
    alias: "",
    permissions: {
      transcript: false,
      video: false,
      council: false,
      retain: false,
    },
    retainDays: null,
    consentDate: new Date().toISOString().slice(0, 10),
  });
  const [error, setError] = useState<string | null>(null);
  const setPermission = (permission: Permission, value: boolean) =>
    setForm((current) => ({
      ...current,
      permissions: { ...current.permissions, [permission]: value },
    }));
  const submit = async () => {
    const errors = validateForm(form);
    if (errors.length) {
      setError(t("consult.form.invalid"));
      return;
    }
    try {
      await consultApi.createClient(buildCreatePayload(form));
      onCreated();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : t("consult.errors.request"),
      );
    }
  };
  return (
    <div className="grid w-full grid-cols-1 gap-10 xl:grid-cols-12">
      <form
        className="flex flex-col gap-[22px] xl:col-span-6"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <h1 className="border-b-2 border-rule-strong pb-3.5 font-serif text-[40px] font-semibold text-primary">
          {t("consult.newClient.title")}
        </h1>
        <label className="flex flex-col gap-1.5 text-sm text-secondary">
          {t("consult.form.alias")}
          <span className="text-xs">{t("consult.journal.aliasHint")}</span>
          <input
            required
            className="h-11 border-0 border-b border-edge-strong bg-transparent px-0 font-serif text-[22px] text-primary outline-none focus:border-accent"
            value={form.alias}
            onChange={(event) =>
              setForm({ ...form, alias: event.target.value })
            }
          />
        </label>
        <fieldset className="flex flex-col gap-3.5 border-0 p-0">
          <legend className="mb-2.5 font-mono text-xs uppercase tracking-[0.08em] text-secondary">
            {t("consult.form.permissions")}
          </legend>
          {PERMISSIONS.map((permission) => (
            <label
              key={permission}
              className="flex items-start gap-3 text-base text-primary"
            >
              <input
                type="checkbox"
                className="mt-0.5 h-5 w-5 accent-accent"
                checked={form.permissions[permission]}
                onChange={(event) =>
                  setPermission(permission, event.target.checked)
                }
              />
              <span>
                {t(`consult.permissions.${permission}`)}
                <span className="block text-sm text-secondary">
                  {t(`consult.journal.permissionHint.${permission}`)}
                </span>
              </span>
            </label>
          ))}
        </fieldset>
        {form.permissions.retain && (
          <label className="flex flex-col gap-1.5 text-sm text-secondary">
            {t("consult.form.retention")}
            <select
              className="h-11 border border-rule bg-surface px-2.5 text-base text-primary"
              value={form.retainDays ?? ""}
              onChange={(event) =>
                setForm({
                  ...form,
                  retainDays:
                    event.target.value === ""
                      ? null
                      : Number(event.target.value),
                })
              }
            >
              <option value="">{t("consult.form.chooseRetention")}</option>
              <option value="30">30</option>
              <option value="90">90</option>
              <option value="365">365</option>
              <option value="0">{t("consult.form.untilRevoke")}</option>
            </select>
          </label>
        )}
        <label className="flex flex-col gap-1.5 text-sm text-secondary">
          {t("consult.form.date")}
          <input
            type="date"
            className="h-11 border-0 border-b border-edge-strong bg-transparent text-primary"
            value={form.consentDate}
            onChange={(event) =>
              setForm({ ...form, consentDate: event.target.value })
            }
          />
        </label>
        {error && (
          <p role="alert" className="text-sm text-err">
            {error}
          </p>
        )}
        <div className="mt-auto flex gap-3">
          <button
            type="submit"
            className="h-11 bg-accent px-[22px] text-sm font-medium text-on-accent"
          >
            {t("consult.actions.create")}
          </button>
          <button
            type="button"
            className="h-11 border border-rule-strong bg-transparent px-[22px] text-sm text-primary"
            onClick={onCancel}
          >
            {t("consult.actions.cancel")}
          </button>
        </div>
      </form>
      <section
        aria-label={t("consult.journal.preview")}
        className="flex flex-col gap-3 border border-rule bg-surface p-8 font-serif text-[15px] leading-relaxed text-primary xl:col-span-6"
      >
        <div className="font-mono text-xs text-secondary">
          {t("consult.journal.preview")}
        </div>
        <pre className="m-0 whitespace-pre-wrap font-serif text-[15px] leading-relaxed">
          {consentPreview(form, form.consentDate)}
        </pre>
      </section>
    </div>
  );
}
