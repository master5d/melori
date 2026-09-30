import "../dev/installMockFirst"; // DEV: install Tauri mock before any platform() call (no-op in prod)
import "../dev/installMeetingDemo"; // DEV: browser smoke harness for the transcript demo (no-op in prod)
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/literata";
import "@fontsource-variable/literata/wght-italic.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import MeetingCopilot from "./MeetingCopilot";
import "@/i18n";
import { startThemeSync } from "@/theme/theme";

startThemeSync();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <MeetingCopilot />
  </React.StrictMode>,
);
