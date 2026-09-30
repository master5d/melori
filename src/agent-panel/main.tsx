import React from "react";
import ReactDOM from "react-dom/client";
import AgentPanel from "./AgentPanel";
import "@/App.css"; // Tailwind + theme — without it the panel renders unstyled (invisible on its transparent window)
import "@/i18n";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AgentPanel />
  </React.StrictMode>,
);
