// Side-effect module imported FIRST in main.tsx (before @tauri-apps/plugin-os
// and ./i18n), so the Tauri IPC/OS mock is installed SYNCHRONOUSLY before any
// platform() call or the i18n settings-sync fires. Must NOT be async — a
// dynamic import would lose the race with i18n's getAppSettings() call.
// The `import.meta.env.DEV` guard is statically false in production, so Rollup
// tree-shakes both the call and the side-effect-free @tauri-apps/api/mocks dep.
import { installTauriMock } from "./tauriMock";

if (import.meta.env.DEV && import.meta.env.VITE_TAURI_MOCK) {
  installTauriMock();
}

// Karaoke vocal-pitch ribbon browser smoke (no mic — synthetic pitch sweep).
// Dynamic import + the three-flag gate below are all statically known at
// build time, so Rollup tree-shakes this call and karaokePitchDemo.ts out of
// production bundles.
if (
  import.meta.env.DEV &&
  import.meta.env.VITE_TAURI_MOCK &&
  import.meta.env.VITE_KARAOKE_DEMO
) {
  void import("./karaokePitchDemo").then((m) => m.runKaraokePitchDemo());
}
