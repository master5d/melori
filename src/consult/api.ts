import { commands } from "@/bindings";
import type { ClientCreate } from "./consentForm";

export interface Consent {
  permissions: string[];
  date: string;
  retain_days: number | null;
  template_version: string | null;
  signed_sha256: string | null;
  revoked: string | null;
  active: boolean;
}

export interface EngineClient {
  id: string;
  alias: string;
  tags: string[];
  created: string;
  consent: Consent;
  template_id?: string | null;
}

export interface EngineSession {
  id: string;
  client: string;
  date: string;
  created: string;
  status: string;
  meeting_type: string;
  note: string;
  runs: unknown[];
  template_id?: string | null;
  duration_ms?: number;
  language?: string;
}

export interface NoteTemplate {
  id: string;
  name: string;
  builtin: boolean;
  sections: { key: string; title: string; guidance: string }[];
}
export interface SessionNote {
  n: number;
  created: string;
  author: string;
  template_id: string;
  sections: [string, string][];
  fields: Record<string, string>;
  parent: number | null;
  provenance: Provenance | null;
  warnings?: { section: string; phrase: string }[];
}
export interface Provenance {
  model: string;
  endpoint_local: boolean;
  input: "transcript" | "note";
  chars_sent: number;
  language: string;
  attempts: number;
}
export interface EmailDraft {
  subject: string;
  body: string;
  author: "model" | "practitioner";
  from_note: number | null;
  created: string;
  provenance: Provenance | null;
  opened_in_mail_at: string | null;
}
export interface NoteVersion {
  n: number;
  created: string;
  author: string;
  template_id: string;
  parent: number | null;
  provenance: Provenance | null;
  warnings?: { section: string; phrase: string }[];
}
export interface SessionAsk {
  at: string;
  elapsed_ms: number;
  kind: string;
  question: string;
  answer: string;
  provenance: Provenance & { until_ms: number; truncated: boolean };
  stored: boolean;
}
export interface SessionDetail {
  session: EngineSession;
  transcript: {
    i: number;
    source: string;
    start_ms: number;
    end_ms: number;
    text: string;
  }[];
  note: SessionNote | null;
  versions: NoteVersion[];
  asks: SessionAsk[];
  runs: unknown[];
  email: EmailDraft | null;
}

export interface ClientDetail {
  client: EngineClient;
  sessions: EngineSession[];
}

export interface SearchResult {
  chunk_id: number;
  session_id: string;
  kind: "transcript" | "note" | "email";
  label: string;
  start_ms: number | null;
  snippet: string;
  score: number;
}
export interface SearchResponse {
  results: SearchResult[];
  index: {
    sessions: number;
    chunks: number;
    vectors_missing: number;
    meaning_available: boolean;
    meaning_error: string | null;
  };
}
export interface ChatProvenance {
  model: string;
  endpoint_local: boolean;
  input: "fragments";
  chars_sent: number;
  fragments: number;
  sessions: number;
  language: string;
  attempts: number;
}
export interface ChatTurn {
  q: string;
  a: string;
  at: string;
  refs: { session_id: string; kind: string; chunk_id: number }[];
  provenance: ChatProvenance;
}
export interface ChatSummary {
  n: number;
  created: string;
  title: string;
  turns: ChatTurn[];
}
export interface ChatResponse {
  chat: number | null;
  turn: ChatTurn;
  stored: boolean;
}
export interface RecapData {
  created: string;
  sessions: string[];
  points: string[];
  provenance: {
    model: string;
    endpoint_local: boolean;
    input: "notes";
    language: string;
    attempts: number;
  };
}
export interface RecapResponse {
  quick: {
    date: string;
    days_ago: number;
    plan: { title: string; text: string } | null;
    email_subject: string | null;
  } | null;
  model: RecapData | null;
  model_stale: boolean;
}

export interface Health {
  ok: boolean;
  version: string;
  disk_encryption: string;
  llm_local: boolean;
}

export interface CouncilSpecialist {
  id: string;
  name: string;
  paradigm: string;
}

export interface PurgeReport {
  deleted: string[];
}

export interface ExportResult {
  filename: string;
  markdown: string;
}

export interface CloseResult {
  retained: boolean;
  session: EngineSession;
}

export class TemplateDeleteConflictError extends Error {
  readonly clients: number;

  constructor(clients: number) {
    super("template is assigned to clients");
    this.name = "TemplateDeleteConflictError";
    this.clients = clients;
  }
}

/** FastAPI sends `detail` as a string, or as a list of validation errors (`[{loc, msg}]`). */
export function errorDetailText(
  detail: string | { msg?: string; loc?: unknown[] }[] | undefined,
): string | undefined {
  if (typeof detail === "string") return detail;
  if (Array.isArray(detail) && detail.length > 0)
    return detail
      .map((item) =>
        [item.loc?.slice(1).join("."), item.msg].filter(Boolean).join(": "),
      )
      .join("; ");
  return undefined;
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const result = await commands.engineRequest(
    method,
    path,
    body === undefined ? null : JSON.stringify(body),
  );
  if (result.status === "error") {
    let detail = String(result.error);
    try {
      const parsed = JSON.parse(detail) as {
        detail?: string | { msg?: string; loc?: unknown[] }[];
        clients?: number;
      };
      if (typeof parsed.clients === "number") {
        throw new TemplateDeleteConflictError(parsed.clients);
      }
      detail = errorDetailText(parsed.detail) ?? detail;
    } catch (cause) {
      if (cause instanceof TemplateDeleteConflictError) throw cause;
      /* engine errors may be plain text */
    }
    throw new Error(detail);
  }
  if (!result.data.trim()) return undefined as T;
  return JSON.parse(result.data) as T;
}

export const consultApi = {
  listClients: () => request<EngineClient[]>("GET", "/api/clients"),
  getClient: (id: string) =>
    request<ClientDetail>("GET", `/api/clients/${encodeURIComponent(id)}`),
  createClient: (payload: ClientCreate) =>
    request<EngineClient>("POST", "/api/clients", payload),
  revokeClient: (id: string) =>
    request<EngineClient>(
      "POST",
      `/api/clients/${encodeURIComponent(id)}/revoke`,
    ),
  deleteClient: (id: string) =>
    request<void>("DELETE", `/api/clients/${encodeURIComponent(id)}`),
  deleteSession: (clientId: string, sessionId: string) =>
    request<void>(
      "DELETE",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}`,
    ),
  search: (
    clientId: string,
    query: string,
    mode: "words" | "meaning" | "both",
  ) =>
    request<SearchResponse>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/search?q=${encodeURIComponent(query)}&mode=${mode}&limit=20`,
    ),
  chats: (clientId: string) =>
    request<ChatSummary[]>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/chats`,
    ),
  chat: (clientId: string, n: number) =>
    request<ChatSummary>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/chats/${n}`,
    ),
  ask: (clientId: string, question: string, chatN?: number) =>
    request<ChatResponse>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/chats`,
      { question, ...(chatN === undefined ? {} : { chat: chatN }) },
    ),
  deleteChat: (clientId: string, n: number) =>
    request<void>(
      "DELETE",
      `/api/clients/${encodeURIComponent(clientId)}/chats/${n}`,
    ),
  recap: (clientId: string) =>
    request<RecapResponse>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/recap`,
    ),
  generateRecap: (clientId: string) =>
    request<{ recap: RecapData; stored: boolean; model_stale: boolean }>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/recap`,
    ),
  health: () => request<Health>("GET", "/health"),
  purgeReport: () => request<PurgeReport>("GET", "/api/purge-report"),
  councilSpecialists: () =>
    request<CouncilSpecialist[]>("GET", "/api/psych-council/specialists"),
  consentTemplate: (id: string) =>
    request<{ version: string; text: string }>(
      "GET",
      `/api/clients/${encodeURIComponent(id)}/consent/template`,
    ),
  signedConsent: (id: string, filename: string, contentBase64: string) =>
    request<void>(
      "POST",
      `/api/clients/${encodeURIComponent(id)}/consent/signed`,
      {
        filename,
        content_base64: contentBase64,
      },
    ),
  exportSession: (
    clientId: string,
    sessionId: string,
    includeCouncil: boolean,
  ) =>
    request<ExportResult>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/export?include_council=${includeCouncil}`,
    ),
  templates: (language?: string) =>
    request<NoteTemplate[]>(
      "GET",
      language
        ? `/api/templates?language=${encodeURIComponent(language)}`
        : "/api/templates",
    ),
  createTemplate: (payload: {
    name: string;
    sections: NoteTemplate["sections"];
  }) => request<NoteTemplate>("POST", "/api/templates", payload),
  updateTemplate: (
    id: string,
    payload: { name: string; sections: NoteTemplate["sections"] },
  ) =>
    request<NoteTemplate>(
      "PUT",
      `/api/templates/${encodeURIComponent(id)}`,
      payload,
    ),
  deleteTemplate: (id: string, force = false) =>
    request<void>(
      "DELETE",
      `/api/templates/${encodeURIComponent(id)}${force ? "?force=true" : ""}`,
    ),
  setClientTemplate: (id: string, template_id: string | null) =>
    request<EngineClient>("PUT", `/api/clients/${encodeURIComponent(id)}`, {
      template_id,
    }),
  sessionDetail: (clientId: string, sessionId: string) =>
    request<SessionDetail>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}`,
    ),
  generateNote: (clientId: string, sessionId: string, templateId: string) =>
    request<unknown>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/notes:generate`,
      { template_id: templateId },
    ),
  saveNote: (
    clientId: string,
    sessionId: string,
    payload: {
      template_id: string;
      fields: Record<string, string>;
      parent: number | null;
    },
  ) =>
    request<unknown>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/notes`,
      payload,
    ),
  noteVersion: (clientId: string, sessionId: string, n: number) =>
    request<SessionNote>(
      "GET",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/notes/${n}`,
    ),
  /** `note` only for a client without `retain`: nothing is stored, so the window sends the note it shows. */
  generateEmail: (
    clientId: string,
    sessionId: string,
    note?: {
      n: number | null;
      template_id: string;
      sections: [string, string][];
      fields: Record<string, string>;
    },
  ) =>
    request<{ email: EmailDraft; stored: boolean }>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/email:generate`,
      note ? { note } : undefined,
    ),
  saveEmail: (
    clientId: string,
    sessionId: string,
    payload: { subject: string; body: string },
  ) =>
    request<EmailDraft>(
      "PUT",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/email`,
      payload,
    ),
  markEmailOpened: (clientId: string, sessionId: string) =>
    request<void>(
      "POST",
      `/api/clients/${encodeURIComponent(clientId)}/sessions/${encodeURIComponent(sessionId)}/email/opened`,
    ),
  // Closing a session is NOT a UI call: the shell's save_client_meeting builds the note and closes it.
};

export type ClientSummary = EngineClient;
export type HealthReport = Health & Partial<PurgeReport>;
