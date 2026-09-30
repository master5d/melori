// Side-effect module imported by the overlay's main.tsx, after the mock IPC
// bus is installed (installMockFirst.ts), to drive the DEV-only browser
// smoke cycler (overlayStateCycler.ts). Gated the same way as
// installMockFirst.ts: import.meta.env flags are statically known at build
// time, so Rollup tree-shakes both this call and overlayStateCycler.ts out
// of production bundles.
import { installOverlayDemo } from "./overlayStateCycler";

if (
  import.meta.env.DEV &&
  import.meta.env.VITE_TAURI_MOCK &&
  import.meta.env.VITE_OVERLAY_DEMO
) {
  installOverlayDemo();
}
