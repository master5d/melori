import "../dev/installMockFirst"; // DEV: install Tauri mock before any platform() call (no-op in prod)
import "../dev/installOverlayDemo"; // DEV: browser smoke cycler for the pill states (no-op in prod)
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/literata";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import RecordingOverlay from "./RecordingOverlay";
import "@/i18n";
import { startThemeSync } from "@/theme/theme";

startThemeSync();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RecordingOverlay />
  </React.StrictMode>,
);
