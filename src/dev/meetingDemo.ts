// DEV-ONLY Meeting Copilot browser-smoke harness — drives the panel's
// transcript through a few source-tagged segments via the mock event bus
// installed by ./tauriMock.ts, so the amber (`me`) and green (`others`)
// gutters and the analysis tabs can be screenshotted without a native
// Tauri window. Mirrors src/dev/overlayStateCycler.ts.
// Gated behind VITE_TAURI_MOCK + VITE_MEETING_DEMO by the caller
// (installMeetingDemo.ts) so this module is dead-code-eliminated from prod
// the same way overlayStateCycler.ts is (see installMockFirst.ts).
import { emit } from "@tauri-apps/api/event";

interface DemoSegment {
  source: "me" | "others";
  text: string;
}

const DEMO_SEGMENTS: DemoSegment[] = [
  {
    source: "me",
    text: "Let's kick off with the roadmap review for this sprint.",
  },
  {
    source: "others",
    text: "Sounds good — I pulled the burndown chart this morning.",
  },
  {
    source: "me",
    text: "Great, can you share the blockers on the auth migration?",
  },
  {
    source: "others",
    text: "Just one: waiting for the local service configuration to be ready.",
  },
];

const SEGMENT_INTERVAL_MS = 500;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

/**
 * Emits a `meeting-state-event` (active + loopback capture) followed by a
 * few `segment-event` frames alternating `me`/`others`, so both transcript
 * gutter colors and the elapsed timer are visible for the smoke screenshot.
 *
 * Segments are emitted after a short delay so the panel's async mount
 * effect (which registers `events.segmentEvent.listen(...)`) has had a
 * chance to run first — a synchronous module-load emit would be dropped by
 * the mock event bus (see overlayStateCycler.ts's identical note).
 */
export async function runMeetingDemo(): Promise<void> {
  await delay(400);
  await emit("meeting-state-event", { active: true, loopback_active: true });

  let id = 1;
  let cursorMs = 0;
  for (const seg of DEMO_SEGMENTS) {
    const start_ms = cursorMs;
    const end_ms = start_ms + 2000;
    cursorMs = end_ms + 200;
    await emit("segment-event", {
      segment: {
        id: id++,
        source: seg.source,
        speaker: null,
        start_ms,
        end_ms,
        text: seg.text,
      },
    });
    await delay(SEGMENT_INTERVAL_MS);
  }

  // Flip to the document (ended) phase so the Granola document view can be
  // screenshotted. The mock `analyze_meeting` (tauriMock.ts) returns a brief
  // analysis, so a manual/scripted Analyze click then fills Notes/Actions.
  await delay(1500);
  await emit("meeting-state-event", { active: false, loopback_active: false });
}
