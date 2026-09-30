// DEV-ONLY Tauri IPC mock — lets the frontend boot in a plain browser for
// visual/design smoke of the shell skins without a native Tauri window.
// Activated only when VITE_TAURI_MOCK is set (see installMockFirst.ts); the
// gated dynamic import means this module is dead-code-eliminated from prod.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import mockSettings from "./mockSettings.json";
import fxClientDetail from "../consult/__fixtures__/client_detail.json";
import fxConsentTemplate from "../consult/__fixtures__/consent_template.json";
import fxHealth from "../consult/__fixtures__/health.json";
import fxListClients from "../consult/__fixtures__/list_clients.json";

const mockParams = new URLSearchParams(window.location.search);
const mockEngine = {
  state: mockParams.get("engine") ?? "ready",
  empty: mockParams.get("clients") === "empty",
  longAlias: mockParams.get("alias") === "long",
  revoked: mockParams.get("revoked") === "1",
};
// ?lang=ar etc. — audit a locale (RTL) without touching mockSettings.json;
// one stable object, since consumers compare settings by reference.
const mockAppSettings = {
  ...mockSettings,
  app_language: mockParams.get("lang") ?? mockSettings.app_language,
};
const LONG_ALIAS =
  "Анна-Мария Константинопольская-Верхнеуфалейская, группа по вторникам";

function mockClient<T extends { alias: string }>(client: T): T {
  const c = { ...client } as T & { consent?: Record<string, unknown> };
  if (mockEngine.longAlias) c.alias = LONG_ALIAS;
  if (mockEngine.revoked && c.consent)
    c.consent = { ...c.consent, revoked: "2026-09-28", active: false };
  return c;
}

function mockEngineResponse(method: string, path: string): unknown {
  if (path === "/health") return fxHealth;
  if (path.endsWith("/consent/template")) return fxConsentTemplate;
  if (method === "GET" && path === "/api/clients")
    return mockEngine.empty ? [] : fxListClients.map(mockClient);
  if (method === "GET" && /^\/api\/clients\/[^/]+$/.test(path))
    return { ...fxClientDetail, client: mockClient(fxClientDetail.client) };
  return {};
}

/** Install synchronous OS-plugin internals + a permissive IPC mock. */
export function installTauriMock(): void {
  // `platform()` (and siblings) read these synchronously at main.tsx load.
  (window as unknown as Record<string, unknown>).__TAURI_OS_PLUGIN_INTERNALS__ =
    {
      platform: "windows",
      version: "11.0.0",
      family: "windows",
      os_type: "windows",
      arch: "x86_64",
      exe_extension: "exe",
      eol: "\r\n",
      hostname: "mock-host",
    };

  // Registers window metadata so getCurrentWebviewWindow()/convertFileSrc work.
  mockWindows("main");

  let eventId = 1;
  // Stable singletons so repeated getters return referentially-equal values —
  // a fresh [] each call would churn React state and spin an invoke↔render loop.
  const EMPTY: readonly unknown[] = Object.freeze([]);
  const PAGINATED = Object.freeze({ entries: EMPTY, has_more: false });
  const asArray = (cmd: string) =>
    /models|devices|microphones|outputs?|providers|prompts|bindings|snippets|words|voices|languages|profiles|list/i.test(
      cmd,
    );

  // Minimal real event bus so emitted events (e.g. show-overlay/mic-level
  // from the DEV overlay-demo cycler) actually reach `listen()` handlers —
  // needed for the overlay browser smoke, which only renders once it
  // receives a "show-overlay" event. Mirrors the invoke arg shapes used by
  // @tauri-apps/api/event's listen()/emit()/emitTo() (`event`, `target`,
  // `handler` — a transformCallback() id — for listen; `event`, `payload`
  // for emit/emit_to; `event`, `eventId` for unlisten).
  const eventListeners = new Map<string, Set<number>>();
  const listenerRidToHandler = new Map<
    number,
    { event: string; handler: number }
  >();

  // Task 4 (shell-live-transport): mock `toggle_dictation` so the
  // TransportBar's ● Rec button is clickable in the browser smoke without a
  // real dictation pipeline. Flips a local idle<->recording flag and
  // re-broadcasts `recording-state` — the same bare-string event the real
  // backend emits (Tasks 1-2) — so `useAudioTransport` picks it up exactly
  // as it would in prod. Independent of the VITE_TRANSPORT_DEMO cycler in
  // src/dev/transportDemo.ts (both just emit `recording-state`); clicking
  // Rec while the demo loop is running simply interrupts/overrides it, which
  // is fine for manual smoke.
  let mockDictationActive = false;

  // Epic A (Audio console) — self-driving `input-monitor-level` ticker so the
  // InputMeter animates in the browser smoke without a real cpal stream (see
  // `start_input_monitor`/`stop_input_monitor` below).
  let monitorTimer: number | undefined;

  // Epic D2 (Audio console) — a synthetic health frame every 8th monitor tick
  // (~4/s) so the DeviceHealthCard renders populated facts in the browser smoke.
  let healthTick = 0;

  // The status was hardcoded "ok", so the browser smoke could never exercise the
  // paths that only a SICK device takes (blanked meter/spectrum, Reconnect, the
  // passthrough auto-off). Script it: `?health=unavailable` pins a status, and
  // `window.__mockHealth("stalled")` flips it live from the console/Playwright.
  let mockHealthStatus = "ok";
  const healthFromQuery = new URLSearchParams(window.location.search).get(
    "health",
  );
  if (healthFromQuery) mockHealthStatus = healthFromQuery;
  (window as unknown as { __mockHealth: (s: string) => void }).__mockHealth = (
    s: string,
  ) => {
    mockHealthStatus = s;
  };

  // Epic B (Audio console) — mock-local gain so `get_input_gain` returns what
  // `set_input_gain` last wrote, letting the dB slider render + move in the
  // browser smoke without a native cpal backend.
  let mockInputGainDb = 0;

  // Epic C (Audio console) — mock monitor/passthrough state so the toggle,
  // device dropdown, and volume slider render + move in the browser smoke.
  let mockPassthrough = false;
  let mockMonitorVolume = 1.0;
  let mockMonitorDevice = "default";

  // Epic D1 (Audio console) — a synthetic spectrum so the analyzer canvases
  // animate in the browser smoke. The band layout mirrors the Rust
  // `band_centers` (128 geometric bands from 20 Hz to 20 kHz at 48 kHz).
  const MOCK_BANDS = 128;
  const mockFreqs = Array.from(
    { length: MOCK_BANDS },
    (_, i) => 20 * Math.pow(1000, (i + 0.5) / MOCK_BANDS),
  );
  /** A voice-ish hump around 300–3 kHz, a 60 Hz hum spike, and a noise floor. */
  const mockSpectrumFrame = (): number[] =>
    mockFreqs.map((f) => {
      const voice = -18 - Math.pow(Math.log10(f / 700), 2) * 26;
      const hum = f > 50 && f < 72 ? -30 : -120;
      const floor = -78 + Math.random() * 6;
      return Math.max(-100, Math.min(0, Math.max(voice, hum, floor)));
    });

  const dispatchEvent = (event: string, payload: unknown) => {
    const handlers = eventListeners.get(event);
    if (!handlers) return;
    const internals = (
      window as unknown as {
        __TAURI_INTERNALS__?: {
          runCallback?: (id: number, data: unknown) => void;
        };
      }
    ).__TAURI_INTERNALS__;
    for (const handlerId of handlers) {
      try {
        internals?.runCallback?.(handlerId, { event, id: 0, payload });
      } catch {
        // Best-effort dev-only event dispatch — a broken listener must not
        // break the rest of the mock event bus.
      }
    }
  };

  mockIPC(async (cmd, args) => {
    switch (cmd) {
      // Tauri event system — a real (if minimal) listen/emit bus so the
      // overlay's `listen("show-overlay", …)` etc. actually fire in DEV.
      case "plugin:event|listen": {
        const { event, handler } = args as { event: string; handler: number };
        if (!eventListeners.has(event)) eventListeners.set(event, new Set());
        eventListeners.get(event)!.add(handler);
        const rid = eventId++;
        listenerRidToHandler.set(rid, { event, handler });
        return rid;
      }
      case "plugin:event|unlisten": {
        const { eventId: rid } = args as { eventId: number };
        const entry = listenerRidToHandler.get(rid);
        if (entry) {
          eventListeners.get(entry.event)?.delete(entry.handler);
          listenerRidToHandler.delete(rid);
        }
        return null;
      }
      case "plugin:event|emit":
      case "plugin:event|emit_to": {
        const { event, payload } = args as { event: string; payload: unknown };
        dispatchEvent(event, payload);
        return null;
      }
      // The calls that actually drive rendering (shell skin + section gates).
      case "get_app_settings":
      case "get_default_settings":
      case "get_settings":
        return mockAppSettings;
      // Structured (non-array) getters whose consumers destructure fields.
      case "get_history_entries":
        return PAGINATED;

      // Meeting Copilot browser smoke (Phase C) — `meeting_status`/
      // `meeting_llm_destination` don't match the generic get_/list_ prefix
      // pattern below, so they need explicit shapes (MeetingStatus /
      // LlmDestination, src/bindings.ts). `active: true` takes the panel's
      // `decideMount()` "reattach" branch straight to the recording phase
      // (skipping the consent gate) so the demo doesn't need a real
      // start_meeting round-trip.
      // melori engine — answered from the contract fixtures (real engine
      // response shapes, src/consult/__fixtures__). Audit variants via URL:
      // ?engine=down|starting, ?clients=empty, ?alias=long, ?revoked=1.
      case "engine_status":
        return {
          state: mockEngine.state,
          url: mockEngine.state === "ready" ? "http://127.0.0.1:0" : null,
          disk_encryption: mockEngine.state === "ready" ? "on" : null,
        };
      case "engine_request": {
        const { method, path } = args as { method: string; path: string };
        if (mockEngine.state !== "ready") throw "engine unavailable";
        return JSON.stringify(mockEngineResponse(method, path));
      }
      case "meeting_status":
        return { active: true, loopback_active: true, segment_count: 0 };
      case "meeting_llm_destination":
        return { configured: true, label: "local", is_local: true };
      // start_meeting/consult_council return `Result<null, _>` — bare `null`
      // is the correct Ok payload (see bindings.ts TAURI_INVOKE wrapping).
      case "start_meeting":
      case "consult_council":
        return null;
      case "stop_meeting":
        return {
          started_at: new Date().toISOString(),
          active: false,
          loopback_active: false,
          segments: [],
        };
      case "analyze_meeting":
        return {
          kind: "brief",
          summary: "Mock smoke summary for the Meeting Copilot browser demo.",
          key_points: [
            "Warm Studio tokens render unchanged",
            "Reduced-motion disables the REC pulse",
          ],
          action_items: ["Verify RTL mirroring"],
          open_questions: [],
        };
      case "save_meeting":
        return "C:/mock/meetings/demo.md";

      // TransportBar's ● Rec button (see comment above `mockDictationActive`).
      case "toggle_dictation":
        mockDictationActive = !mockDictationActive;
        dispatchEvent(
          "recording-state",
          mockDictationActive ? "recording" : "idle",
        );
        return null;

      // Course phoneme-compare browser smoke — espeak-ng + real STT aren't
      // available headless, so mock a representative PhonemeReport (a
      // think→sink example so a `sub` cell renders red) and the record→STT
      // round-trip the Tutor UI drives before scoring.
      case "phoneme_compare":
        return {
          overall: 75,
          note: "Close — check the highlighted sounds.",
          words: [
            {
              text: "think",
              cells: [
                { ipa: "θ", status: "sub", said: "s" },
                { ipa: "ɪ", status: "ok", said: null },
                { ipa: "ŋ", status: "ok", said: null },
                { ipa: "k", status: "ok", said: null },
              ],
            },
          ],
        };
      case "transcribe_file_to_string":
        return "sink";
      case "get_app_dir_path":
        return "C:/mock/appdir";

      // Audio console (Epic A) — drive a gentle random-walk meter so the
      // InputMeter animates in the browser smoke without a real cpal stream.
      case "start_input_monitor": {
        if (monitorTimer === undefined) {
          let level = 0.15;
          monitorTimer = window.setInterval(() => {
            level = Math.min(
              1,
              Math.max(0.02, level + (Math.random() - 0.5) * 0.25),
            );
            const peak = Math.min(1, level + Math.random() * 0.1);
            dispatchEvent("input-monitor-level", { peak, rms: level });
            dispatchEvent("input-spectrum", {
              bands: mockSpectrumFrame(),
              sample_rate: 48000,
            });
            healthTick = (healthTick + 1) % 8;
            if (healthTick === 0) {
              dispatchEvent("input-device-health", {
                status: mockHealthStatus,
                // "starting" means the open is still in flight — the device has
                // not told us what it negotiated yet, so facts are null. Any
                // other status carries them.
                facts:
                  mockHealthStatus === "starting"
                    ? null
                    : {
                        name: "Mock Microphone",
                        sample_rate: 48000,
                        channels: 1,
                        sample_format: "F32",
                      },
                silent_for_ms: 0,
                reason:
                  mockHealthStatus === "unavailable"
                    ? "Mock: input device disconnected"
                    : null,
              });
            }
          }, 33);
        }
        return null;
      }
      case "stop_input_monitor": {
        if (monitorTimer !== undefined) {
          window.clearInterval(monitorTimer);
          monitorTimer = undefined;
        }
        return null;
      }

      // Epic B (Audio console) — dB input-gain slider browser smoke.
      case "get_input_gain":
        return mockInputGainDb;
      case "set_input_gain":
        mockInputGainDb = (args as { db: number }).db;
        return null;

      // Epic C (Audio console) — passthrough toggle + monitor volume/device browser smoke.
      case "get_passthrough_enabled":
        return mockPassthrough;
      case "set_passthrough_enabled":
        mockPassthrough = (args as { enabled: boolean }).enabled;
        return null;
      case "get_monitor_volume":
        return mockMonitorVolume;
      case "set_monitor_volume":
        mockMonitorVolume = (args as { volume: number }).volume;
        return null;
      case "get_monitor_output_device":
        return mockMonitorDevice;
      case "set_monitor_output_device":
        mockMonitorDevice = (args as { deviceName: string }).deviceName;
        return null;

      // Epic D1 (Audio console) — spectrum analyzer frequency axis.
      case "get_spectrum_freqs":
        return mockFreqs;

      // Epic D2 (Audio console) — Reconnect button in DeviceHealthCard.
      case "reconnect_input_device":
        return null;
    }

    // OS / window / webview / global-shortcut plugin calls — inert.
    if (cmd.startsWith("plugin:")) return null;

    // App getters: plural-ish → stable empty list, else null (degrade section
    // internals gracefully; the shell frame + tokens still render).
    if (/^(get|list|fetch|load|has)_/.test(cmd))
      return asArray(cmd) ? EMPTY : null;

    // Setters/actions (change_*, set_*, start_*, …) — accept and return null.
    return null;
  });
}
