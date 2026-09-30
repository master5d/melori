import React from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { Home as HomeIcon, Search, ChevronRight } from "lucide-react";
import type { ShellComponentProps } from "./ShellHost";
import { AudioStatusBar } from "./AudioStatusBar";
import { OPEN_COMMAND_PALETTE_EVENT } from "./paletteEvents";
import { paletteKbdHint } from "./shellUx";

/**
 * "Студия" (Home) — the default suite skin: a serif greeting, a Cmd+K
 * palette-trigger row, and a 3x2 grid of module cards. Clicking a card opens
 * that module fullscreen with a "⌂ / Module" breadcrumb to return to the
 * grid. AudioStatusBar is pinned along the bottom, same as Rail. Visual
 * reference: the journal shell mockups.
 *
 * Per-card "live status" is a static per-module subtitle, not a live session
 * feed: there is no existing frontend store for recent-session data (history
 * is fetched ad hoc inside `HistorySettings`, not cached globally), and Task
 * 3's ambiguity resolution #2 says not to add a new backend call just for
 * this. Follow-up: wire real status once a shared history store exists.
 */
export const HomeShell: React.FC<ShellComponentProps> = ({
  modules,
  activeModuleId,
  renderWorkspace,
  onOpenSettings,
  isModuleOpen,
  onOpenModule,
  onCloseModule,
}) => {
  const { t } = useTranslation();
  const activeModule =
    modules.find((m) => m.id === activeModuleId) ?? modules[0];
  // Read once per render, same sync call AccessibilityPermissions.tsx makes
  // (`@tauri-apps/plugin-os`'s `type()` is synchronous). Kept inside the
  // component (not module scope) so importing this file for the resolver
  // test doesn't require a `window` global.
  const isMacOS = type() === "macos";

  const openPalette = () => {
    window.dispatchEvent(new CustomEvent(OPEN_COMMAND_PALETTE_EVENT));
  };

  return (
    <div
      data-testid="home-shell"
      className="h-full w-full flex flex-col bg-ground text-primary"
    >
      {isModuleOpen ? (
        <div className="flex-1 min-h-0 flex flex-col overflow-y-auto custom-scrollbar">
          <div className="flex items-center gap-2 px-6 py-5 border-b border-edge">
            <button
              type="button"
              onClick={onCloseModule}
              aria-label={t("shell.home.breadcrumbHome")}
              className="flex items-center gap-1.5 text-secondary hover:text-accent transition-colors"
            >
              <HomeIcon width={16} height={16} />
            </button>
            <ChevronRight width={14} height={14} className="text-secondary" />
            <h1 className="font-display text-xl text-primary">
              {t(activeModule.titleKey)}
            </h1>
          </div>
          <div className="flex-1 p-6">{renderWorkspace(activeModuleId)}</div>
        </div>
      ) : (
        <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-6 flex flex-col gap-5">
          <div className="flex items-center gap-4">
            <span className="font-display text-2xl whitespace-nowrap">
              {t("shell.home.greetingLead")}{" "}
              <i className="italic text-accent">
                {t("shell.home.greetingEmphasis")}
              </i>
            </span>
            <button
              type="button"
              onClick={openPalette}
              className="flex-1 flex items-center gap-2.5 bg-surface border border-edge rounded-xl px-4 py-2.5 text-secondary text-sm hover:border-accent/40 transition-colors"
            >
              <Search width={15} height={15} className="shrink-0" />
              <span className="truncate">{t("shell.home.paletteHint")}</span>
              <span className="ms-auto font-data text-[10px] border border-edge rounded px-1.5 py-0.5 shrink-0">
                {paletteKbdHint(isMacOS)}
              </span>
            </button>
            <button
              type="button"
              onClick={onOpenSettings}
              className="text-xs text-secondary hover:text-primary transition-colors whitespace-nowrap"
            >
              {t("shell.palette.settings")}
            </button>
          </div>

          <div className="grid grid-cols-3 gap-3">
            {modules.map((mod) => {
              const Icon = mod.icon;
              const isSoon = mod.sections.length === 0;
              return (
                <button
                  key={mod.id}
                  type="button"
                  onClick={() => onOpenModule(mod.id)}
                  className={`flex flex-col items-start gap-2 text-start bg-surface border border-edge rounded-2xl px-4 py-3.5 transition-[transform,border-color] duration-300 hover:-translate-y-0.5 hover:border-accent/40 ${
                    isSoon ? "opacity-60 border-dashed" : ""
                  }`}
                >
                  <span className="flex items-center gap-2 w-full">
                    <Icon width={16} height={16} className="text-accent" />
                    <span className="text-sm font-semibold">
                      {t(mod.titleKey)}
                    </span>
                    {isSoon && (
                      <span className="ms-auto font-data text-[9px] tracking-wide uppercase text-secondary">
                        {t("shell.home.soonTag")}
                      </span>
                    )}
                  </span>
                  <span className="text-xs text-secondary leading-relaxed">
                    {t(`shell.home.cardSubtitle.${mod.id}`)}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      <AudioStatusBar />
    </div>
  );
};
