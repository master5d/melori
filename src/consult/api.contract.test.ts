import { describe, expect, it, vi } from "vitest";
import { commands } from "@/bindings";
import { consultApi } from "./api";
import type { ClientCreate } from "./consentForm";
import clientDetail from "./__fixtures__/client_detail.json";
import consentTemplate from "./__fixtures__/consent_template.json";
import createClient from "./__fixtures__/create_client.json";
import error403 from "./__fixtures__/error_403.json";
import exportPlain from "./__fixtures__/export_plain.json";
import health from "./__fixtures__/health.json";
import listClients from "./__fixtures__/list_clients.json";
import purgeReport from "./__fixtures__/purge_report.json";
import revoke from "./__fixtures__/revoke.json";
import templates from "./__fixtures__/templates.json";
import sessionDetail from "./__fixtures__/session_detail.json";
import noteVersion from "./__fixtures__/note_version.json";
import emailDraft from "./__fixtures__/email_draft.json";
import search from "./__fixtures__/search.json";
import chat from "./__fixtures__/chat.json";
import recap from "./__fixtures__/recap.json";

vi.mock("@/bindings", () => ({
  commands: {
    engineRequest: vi.fn(),
  },
}));

const engineRequest = vi.mocked(commands.engineRequest);
const sessionId = clientDetail.sessions[0].id;

function respond(data: unknown) {
  engineRequest.mockResolvedValue({ status: "ok", data: JSON.stringify(data) });
}

describe("consultApi engine contract", () => {
  it("uses the real client and session response shapes", async () => {
    respond(listClients);
    expect(await consultApi.listClients()).toEqual(listClients);
    expect(engineRequest).toHaveBeenLastCalledWith("GET", "/api/clients", null);

    respond(clientDetail);
    const detail = await consultApi.getClient("anna");
    expect(detail.client.id).toBe("anna");
    // id сессии = дата генерации фикстуры + номер; день прогона генератора не зашит
    expect(detail.sessions[0].id).toMatch(/^\d{4}-\d{2}-\d{2}-01$/);
    expect(engineRequest).toHaveBeenLastCalledWith(
      "GET",
      "/api/clients/anna",
      null,
    );
  });

  it("uses the engine export route and preserves plain markdown", async () => {
    respond(exportPlain);
    const result = await consultApi.exportSession("anna", sessionId, false);
    expect(result).toEqual(exportPlain);
    expect(result.markdown).not.toContain("Разбор");
    expect(engineRequest).toHaveBeenLastCalledWith(
      "GET",
      `/api/clients/anna/sessions/${sessionId}/export?include_council=false`,
      null,
    );
  });

  it("returns health, purge, revoke, and consent template fixtures without loss", async () => {
    respond(health);
    expect(await consultApi.health()).toEqual(health);
    expect(engineRequest).toHaveBeenLastCalledWith("GET", "/health", null);

    respond(purgeReport);
    expect(await consultApi.purgeReport()).toEqual(purgeReport);
    expect(engineRequest).toHaveBeenLastCalledWith(
      "GET",
      "/api/purge-report",
      null,
    );

    respond(revoke);
    const revoked = await consultApi.revokeClient("anna");
    expect(revoked.consent.revoked).not.toBeNull();
    expect(engineRequest).toHaveBeenLastCalledWith(
      "POST",
      "/api/clients/anna/revoke",
      null,
    );

    respond(consentTemplate);
    expect(await consultApi.consentTemplate("anna")).toEqual(consentTemplate);
    expect(engineRequest).toHaveBeenLastCalledWith(
      "GET",
      "/api/clients/anna/consent/template",
      null,
    );
  });

  it("covers client creation, signed consent, closing, and deletion routes", async () => {
    const payload: ClientCreate = {
      alias: "Анна",
      permissions: ["council"],
      consent_date: "2026-09-27",
      retain_days: null,
      tags: [],
      intake: "",
    };
    respond(createClient);
    expect(await consultApi.createClient(payload)).toEqual(createClient);
    expect(engineRequest).toHaveBeenLastCalledWith(
      "POST",
      "/api/clients",
      JSON.stringify(payload),
    );

    respond({});
    await consultApi.signedConsent("anna", "consent.md", "Y29uc2VudA==");
    expect(engineRequest).toHaveBeenLastCalledWith(
      "POST",
      "/api/clients/anna/consent/signed",
      JSON.stringify({
        filename: "consent.md",
        content_base64: "Y29uc2VudA==",
      }),
    );

    respond({});
    await consultApi.deleteClient("anna");
    expect(engineRequest).toHaveBeenLastCalledWith(
      "DELETE",
      "/api/clients/anna",
      null,
    );
  });

  it("rejects with the engine detail", async () => {
    engineRequest.mockResolvedValue({
      status: "error",
      error: JSON.stringify(error403),
    });
    await expect(consultApi.getClient("anna")).rejects.toThrow(error403.detail);
  });
  it("preserves template and session note fixture shapes", async () => {
    respond(templates);
    expect(await consultApi.templates()).toEqual(templates);
    expect((await consultApi.templates())[0].sections[0]).toHaveProperty(
      "guidance",
    );

    respond(sessionDetail);
    const detail = await consultApi.sessionDetail("anna", sessionId);
    expect(detail.note).toEqual(sessionDetail.note);
    expect(detail.transcript[0].text).toBe("A session fact.");

    expect(noteVersion.sections).toEqual(sessionDetail.note.sections);
    expect(noteVersion.fields).toEqual(sessionDetail.note.fields);
    expect(emailDraft).toEqual(sessionDetail.email);
    expect(detail.versions[0].parent).toBeNull();
    expect(detail.versions[0].provenance?.input).toBe("transcript");
    expect(detail.email?.provenance?.input).toBe("note");
  });
  it("types a template delete conflict with its client count", async () => {
    engineRequest.mockResolvedValue({
      status: "error",
      error: JSON.stringify({ detail: "conflict", clients: 3 }),
    });
    await expect(consultApi.deleteTemplate("custom")).rejects.toMatchObject({
      name: "TemplateDeleteConflictError",
      clients: 3,
    });
  });
  it("deletes one session through the engine route", async () => {
    engineRequest.mockResolvedValue({ status: "ok", data: "" });
    await consultApi.deleteSession("anna", sessionId);
    expect(engineRequest).toHaveBeenLastCalledWith(
      "DELETE",
      `/api/clients/anna/sessions/${sessionId}`,
      null,
    );
  });
  it("preserves memory search, chat, and recap contracts", async () => {
    respond(search);
    expect(await consultApi.search("anna", "сон слова", "words")).toEqual(
      search,
    );
    expect(engineRequest).toHaveBeenLastCalledWith(
      "GET",
      "/api/clients/anna/search?q=%D1%81%D0%BE%D0%BD%20%D1%81%D0%BB%D0%BE%D0%B2%D0%B0&mode=words&limit=20",
      null,
    );

    respond([chat]);
    expect(await consultApi.chats("anna")).toEqual([chat]);
    respond(chat);
    expect(await consultApi.ask("anna", "What was discussed?", 1)).toEqual(
      chat,
    );
    expect(engineRequest).toHaveBeenLastCalledWith(
      "POST",
      "/api/clients/anna/chats",
      JSON.stringify({ question: "What was discussed?", chat: 1 }),
    );

    respond(recap);
    expect(await consultApi.recap("anna")).toEqual(recap);
    respond(recap);
    expect(await consultApi.generateRecap("anna")).toEqual(recap);
  });
});

describe("errorDetailText", () => {
  it("reads FastAPI validation lists, not [object Object]", async () => {
    const { errorDetailText } = await import("./api");
    expect(
      errorDetailText([{ loc: ["body", "q"], msg: "Field required" }]),
    ).toBe("q: Field required");
    expect(errorDetailText("no such client")).toBe("no such client");
    expect(errorDetailText(undefined)).toBeUndefined();
  });
});
