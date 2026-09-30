export type Permission = "transcript" | "video" | "council" | "retain";
export const PERMISSIONS: Permission[] = [
  "transcript",
  "video",
  "council",
  "retain",
];

export interface ConsentFormState {
  alias: string;
  permissions: Record<Permission, boolean>;
  retainDays: number | null;
  consentDate: string;
}

export interface ClientCreate {
  alias: string;
  permissions: Permission[];
  consent_date: string;
  retain_days: number | null;
  tags: string[];
  intake: string;
}

export function buildCreatePayload(form: ConsentFormState): ClientCreate {
  return {
    alias: form.alias.trim(),
    permissions: PERMISSIONS.filter(
      (permission) => form.permissions[permission],
    ),
    consent_date: form.consentDate,
    retain_days: form.permissions.retain ? form.retainDays : null,
    tags: [],
    intake: "",
  };
}

export function validateForm(form: ConsentFormState): string[] {
  const errors: string[] = [];
  if (!form.alias.trim()) errors.push("alias");
  if (!PERMISSIONS.some((permission) => form.permissions[permission]))
    errors.push("permissions");
  if (form.permissions.retain && form.retainDays === undefined)
    errors.push("retainDays");
  return errors;
}
