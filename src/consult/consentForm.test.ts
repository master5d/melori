import { describe, expect, it } from "vitest";
import {
  buildCreatePayload,
  validateForm,
  type ConsentFormState,
} from "./consentForm";

const base: ConsentFormState = {
  alias: "Анна",
  permissions: { transcript: true, video: false, council: true, retain: true },
  retainDays: 90,
  consentDate: "2026-09-27",
};

describe("consent form", () => {
  it("sends only granted permissions", () => {
    expect(buildCreatePayload(base).permissions).toEqual([
      "transcript",
      "council",
      "retain",
    ]);
  });
  it("retain_days is null when retain is not granted", () => {
    const f = { ...base, permissions: { ...base.permissions, retain: false } };
    expect(buildCreatePayload(f).retain_days).toBeNull();
  });
  it("rejects empty alias and no permissions", () => {
    const f = {
      ...base,
      alias: " ",
      permissions: {
        transcript: false,
        video: false,
        council: false,
        retain: false,
      },
    };
    expect(validateForm(f).length).toBe(2);
  });
  it("valid form has no errors", () => {
    expect(validateForm(base)).toEqual([]);
  });
});
