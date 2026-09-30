import "./dev/installMockFirst"; // DEV: install Tauri mock before any platform() call (no-op in prod)
import "./dev/installTransportDemo"; // DEV: browser smoke cycler for the shell status bars (no-op in prod)
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/literata";
import "@fontsource-variable/literata/wght-italic.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import { platform } from "@tauri-apps/plugin-os";
import "./App.css";
import App from "./App";
import { startThemeSync } from "./theme/theme";

startThemeSync();

// Set platform before render so CSS can scope per-platform (e.g. scrollbar styles)
document.documentElement.dataset.platform = platform();

// Initialize i18n
import "./i18n";

// Initialize model store (loads models and sets up event listeners)
import { useModelStore } from "./stores/modelStore";
useModelStore.getState().initialize();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
