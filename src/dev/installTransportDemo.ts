// Side-effect module imported by the main window's main.tsx, after the mock
// IPC bus is installed (installMockFirst.ts), to drive the DEV-only shell
// transport browser smoke (transportDemo.ts). Gated the same way as
// installMeetingDemo.ts/installOverlayDemo.ts: import.meta.env flags are
// statically known at build time, so Rollup tree-shakes both this call and
// transportDemo.ts out of production bundles.
import { installTransportDemo } from "./transportDemo";

if (
  import.meta.env.DEV &&
  import.meta.env.VITE_TAURI_MOCK &&
  import.meta.env.VITE_TRANSPORT_DEMO
) {
  installTransportDemo();
}
