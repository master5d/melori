import React from "react";
import { useTranslation } from "react-i18next";
import { Cog, Mic, UsersRound } from "lucide-react";
import { commands } from "@/bindings";
import type { ShellComponentProps } from "./ShellHost";

/** The single melori layout: a 76px journal rail and one consultation surface. */
export const RailShell: React.FC<ShellComponentProps> = ({
  modules,
  activeModuleId,
  onSelectModule,
  renderWorkspace,
  onOpenSettings,
}) => {
  const { t } = useTranslation();
  const activeModule =
    modules.find((module) => module.id === activeModuleId) ?? modules[0];

  return (
    <div
      data-testid="rail-shell"
      className="flex h-full w-full bg-ground text-primary"
    >
      <nav
        aria-label={t("shell.rail.navLabel")}
        className="flex w-[var(--rail-w)] shrink-0 flex-col items-center gap-7 border-e border-rule px-0 py-7"
      >
        <div
          aria-hidden="true"
          className="font-serif text-3xl font-semibold leading-none text-accent"
        >
          {t("shell.rail.mark")}
        </div>
        <button
          type="button"
          aria-label={t("shell.rail.clients")}
          aria-current={activeModule?.id === "consult" ? "page" : undefined}
          onClick={() => onSelectModule("consult")}
          className={`relative flex h-11 w-11 items-center justify-center border-b-2 transition-colors ${
            activeModule?.id === "consult"
              ? "border-accent text-ink"
              : "border-transparent text-secondary hover:text-primary"
          }`}
        >
          <UsersRound width={22} height={22} strokeWidth={1.6} />
        </button>
        <button
          type="button"
          aria-label={t("shell.rail.meeting")}
          onClick={() => void commands.showMeetingCopilot()}
          className="flex h-11 w-11 items-center justify-center border-b-2 border-transparent text-secondary transition-colors hover:text-primary"
        >
          <Mic width={22} height={22} strokeWidth={1.6} />
        </button>
        <button
          type="button"
          aria-label={t("shell.rail.settings")}
          onClick={onOpenSettings}
          className="mt-auto flex h-11 w-11 items-center justify-center border-b-2 border-transparent text-secondary transition-colors hover:text-primary"
        >
          <Cog width={22} height={22} strokeWidth={1.6} />
        </button>
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto custom-scrollbar">
        <div className="flex min-h-full flex-col gap-7 px-12 py-12">
          <div className="flex-1">{renderWorkspace("consult")}</div>
        </div>
      </main>
    </div>
  );
};
