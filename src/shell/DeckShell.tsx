import React from "react";
import { useTranslation } from "react-i18next";
import { Cog } from "lucide-react";
import EchoHand from "../components/icons/EchoHand";
import type { ShellComponentProps } from "./ShellHost";
import { TransportBar } from "./TransportBar";

/**
 * "Пульт" (Deck) — the DAW-style suite skin: modules are top tool-tabs, and
 * a persistent bottom TransportBar (● Rec / timecode / IN-OUT monitors /
 * source / local badge) anchors every module, replacing the role
 * AudioStatusBar plays in Rail/Home. Visual reference:
 * the journal shell mockups.
 */
export const DeckShell: React.FC<ShellComponentProps> = ({
  modules,
  activeModuleId,
  onSelectModule,
  renderWorkspace,
  onOpenSettings,
}) => {
  const { t } = useTranslation();

  return (
    <div
      data-testid="deck-shell"
      className="h-full w-full flex flex-col bg-ground text-primary"
    >
      <div
        role="tablist"
        aria-label={t("shell.rail.navLabel")}
        className="shrink-0 flex items-end gap-1 px-4 pt-2 border-b border-edge bg-surface"
      >
        <EchoHand width={18} height={18} className="mb-2.5 me-1 text-accent" />
        {modules.map((mod) => {
          const isActive = mod.id === activeModuleId;
          return (
            <button
              key={mod.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              onClick={() => onSelectModule(mod.id)}
              className={`relative px-4 pb-2.5 pt-2 text-xs rounded-t-lg border border-transparent border-b-0 transition-[color,transform] duration-300 ${
                isActive
                  ? "text-primary bg-ground border-edge font-semibold"
                  : "text-secondary hover:text-primary hover:-translate-y-px"
              }`}
            >
              {t(mod.titleKey)}
              {isActive && (
                <span className="absolute inset-x-3 -bottom-px h-[2px] rounded-full bg-accent" />
              )}
            </button>
          );
        })}
        <button
          type="button"
          onClick={onOpenSettings}
          aria-label={t("shell.rail.settings")}
          className={
            "ms-auto mb-2 flex items-center justify-center w-8 h-8 rounded-lg text-secondary " +
            "hover:text-primary hover:bg-accent/5 transition-colors"
          }
        >
          <Cog width={16} height={16} />
        </button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-6">
        {renderWorkspace(activeModuleId)}
      </div>

      <TransportBar />
    </div>
  );
};
