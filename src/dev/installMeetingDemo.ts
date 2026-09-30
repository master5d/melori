// Side-effect module imported by the meeting webview's main.tsx, after the
// mock IPC bus is installed (installMockFirst.ts), to drive the DEV-only
// Meeting Copilot browser smoke (meetingDemo.ts). Gated the same way as
// installOverlayDemo.ts: import.meta.env flags are statically known at
// build time, so Rollup tree-shakes both this call and meetingDemo.ts out
// of production bundles.
import { runMeetingDemo } from "./meetingDemo";

if (
  import.meta.env.DEV &&
  import.meta.env.VITE_TAURI_MOCK &&
  import.meta.env.VITE_MEETING_DEMO
) {
  void runMeetingDemo();
}
