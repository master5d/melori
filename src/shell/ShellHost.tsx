import React from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "../hooks/useSettings";
import { SECTIONS_CONFIG } from "../components/Sidebar";
import { SUITE_MODULES, type ModuleId } from "./modules";
import { RailShell } from "./RailShell";
import { CommandPalette, type PaletteActions } from "./CommandPalette";
import { OPEN_COMMAND_PALETTE_EVENT } from "./paletteEvents";

export interface ShellComponentProps {
  modules: typeof SUITE_MODULES;
  activeModuleId: ModuleId;
  onSelectModule: (id: ModuleId) => void;
  renderWorkspace: (moduleId: ModuleId) => React.ReactNode;
  onOpenSettings: () => void;
  /** Navigation callbacks are kept at the host boundary so the global
   * palette can select the consultation workspace. */
  isModuleOpen: boolean;
  onOpenModule: (id: ModuleId) => void;
  onCloseModule: () => void;
}

/** The melori shell has one layout; legacy skin values are compatibility data. */
export function resolveShellComponent(
  skin: string | null | undefined,
): React.ComponentType<ShellComponentProps> {
  void skin;
  return RailShell;
}

const ModulePlaceholder: React.FC<{ moduleId: ModuleId }> = ({ moduleId }) => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center gap-2 h-full text-secondary">
      <p className="font-display text-lg text-primary">
        {t(`shell.modules.${moduleId}`)}
      </p>
      <p className="text-sm">{t("shell.placeholder.comingSoon")}</p>
    </div>
  );
};

interface ShellHostProps {
  onOpenSettings: () => void;
  /** Task 1: Settings now renders as an overlay in App.tsx instead of
   * unmounting ShellHost, so ShellHost needs to know when it's open (to
   * close it before surfacing the palette / a navigation target) and how
   * to close it. */
  settingsOpen: boolean;
  onCloseSettings: () => void;
}

/**
 * Reads `shell_skin` from the settings store and renders the matching shell,
 * handing it the module registry + a `renderWorkspace` callback that mounts
 * each module's existing section components as a vertical stack (unchanged
 * section internals — this task only relocates their mounting point).
 */
export const ShellHost: React.FC<ShellHostProps> = ({
  onOpenSettings,
  settingsOpen,
  onCloseSettings,
}) => {
  const { settings } = useSettings();
  const [activeModuleId, setActiveModuleId] = React.useState<ModuleId>(
    SUITE_MODULES[0].id,
  );
  const [isPaletteOpen, setIsPaletteOpen] = React.useState(false);
  const [isModuleOpen, setIsModuleOpen] = React.useState(false);

  const onOpenModule = React.useCallback((id: ModuleId) => {
    setActiveModuleId(id);
    setIsModuleOpen(true);
  }, []);
  const onCloseModule = React.useCallback(() => setIsModuleOpen(false), []);

  const renderWorkspace = React.useCallback(
    (moduleId: ModuleId): React.ReactNode => {
      const mod = SUITE_MODULES.find((m) => m.id === moduleId);
      if (!mod || mod.sections.length === 0) {
        return <ModulePlaceholder moduleId={moduleId} />;
      }
      return (
        <div className="flex flex-col items-center gap-4 w-full">
          {mod.sections
            .filter((id) => SECTIONS_CONFIG[id].enabled(settings))
            .map((id) => {
              const Section = SECTIONS_CONFIG[id].component;
              return <Section key={id} />;
            })}
        </div>
      );
    },
    [settings],
  );

  const Shell = resolveShellComponent(settings?.shell_skin);

  // Global Ctrl+K / Cmd+K listener — the palette must open from any shell,
  // so it's wired once here rather than duplicated per-shell component.
  React.useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        // Settings is now a sibling overlay (Task 1) rather than a swap that
        // unmounts ShellHost, so it can be covering the shell when Cmd+K
        // fires. Close it first so the palette (and whatever it navigates
        // to) is visible on the still-mounted shell underneath, instead of
        // opening hidden behind the overlay.
        if (settingsOpen) onCloseSettings();
        setIsPaletteOpen(true);
      }
    };
    const handleOpenEvent = () => {
      if (settingsOpen) onCloseSettings();
      setIsPaletteOpen(true);
    };
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener(OPEN_COMMAND_PALETTE_EVENT, handleOpenEvent);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener(OPEN_COMMAND_PALETTE_EVENT, handleOpenEvent);
    };
  }, [settingsOpen, onCloseSettings]);

  const paletteActions: PaletteActions = React.useMemo(
    () => ({
      onOpenClients: () => {
        onCloseSettings();
        onOpenModule("consult");
      },
      onNewClient: () => {
        onCloseSettings();
        onOpenModule("consult");
      },
      onStartMeeting: () => {
        onCloseSettings();
        void commands.showMeetingCopilot();
      },
      onOpenSettings,
      onChangeTheme: (theme) => {
        void commands.changeUiThemeSetting(theme);
      },
    }),
    [onOpenSettings, onCloseSettings, onOpenModule],
  );

  return (
    <>
      <Shell
        modules={SUITE_MODULES}
        activeModuleId={activeModuleId}
        onSelectModule={setActiveModuleId}
        renderWorkspace={renderWorkspace}
        onOpenSettings={onOpenSettings}
        isModuleOpen={isModuleOpen}
        onOpenModule={onOpenModule}
        onCloseModule={onCloseModule}
      />
      <CommandPalette
        isOpen={isPaletteOpen}
        onClose={() => setIsPaletteOpen(false)}
        actions={paletteActions}
      />
    </>
  );
};
