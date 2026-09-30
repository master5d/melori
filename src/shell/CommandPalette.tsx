import React from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import type { ThemeSetting } from "../theme/theme";

export const THEME_SETTINGS = ["system", "day", "evening"] as const;

const THEME_KEYWORDS_RU: Record<(typeof THEME_SETTINGS)[number], string> = {
  system: "как в системе",
  day: "дневная",
  evening: "вечерняя",
};

export type PaletteActionType =
  | "open-clients"
  | "new-client"
  | "start-meeting"
  | "open-settings"
  | "change-theme";

export interface PaletteActionShape {
  type: PaletteActionType;
  payload?: string;
}

export interface PaletteCommand {
  id: string;
  titleKey: string;
  keywords: string[];
  action: PaletteActionShape;
  run: () => void;
}

export interface PaletteActions {
  onOpenClients: () => void;
  onNewClient: () => void;
  onStartMeeting: () => void;
  onOpenSettings: () => void;
  onChangeTheme: (theme: ThemeSetting) => void;
}

const COMMAND_DEFINITIONS: Array<{
  id: string;
  titleKey: string;
  keywords: string[];
  action: PaletteActionShape;
  run: (actions: PaletteActions) => void;
}> = [
  {
    id: "open-clients",
    titleKey: "shell.palette.clients",
    keywords: ["clients", "consult", "клиенты", "консультации"],
    action: { type: "open-clients" },
    run: (a) => a.onOpenClients(),
  },
  {
    id: "new-client",
    titleKey: "shell.palette.newClient",
    keywords: [
      "new client",
      "create client",
      "новый клиент",
      "создать клиента",
    ],
    action: { type: "new-client" },
    run: (a) => a.onNewClient(),
  },
  {
    id: "start-meeting",
    titleKey: "shell.palette.startMeeting",
    keywords: ["meeting", "start meeting", "встреча", "начать встречу"],
    action: { type: "start-meeting" },
    run: (a) => a.onStartMeeting(),
  },
  {
    id: "open-settings",
    titleKey: "shell.palette.settings",
    keywords: ["settings", "preferences", "настройки"],
    action: { type: "open-settings" },
    run: (a) => a.onOpenSettings(),
  },
];

/** Pure command-list builder for the melori consultation shell. */
export function paletteCommands(actions: PaletteActions): PaletteCommand[] {
  const commands = COMMAND_DEFINITIONS.map((definition) => ({
    ...definition,
    run: () => definition.run(actions),
  }));
  const themes = THEME_SETTINGS.map((theme) => ({
    id: `theme:${theme}`,
    titleKey: `shell.palette.theme.${theme}`,
    keywords: [theme, `theme ${theme}`, "тема", THEME_KEYWORDS_RU[theme]],
    action: { type: "change-theme" as const, payload: theme },
    run: () => actions.onChangeTheme(theme),
  }));
  return [...commands, ...themes];
}

export function filterPaletteCommands(
  commands: PaletteCommand[],
  query: string,
): PaletteCommand[] {
  const q = query.trim().toLowerCase();
  if (!q) return commands;
  return commands.filter(
    (command) =>
      command.id.toLowerCase().includes(q) ||
      command.keywords.some((keyword) => keyword.toLowerCase().includes(q)),
  );
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  actions: PaletteActions;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen,
  onClose,
  actions,
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = React.useState("");
  const [highlight, setHighlight] = React.useState(0);
  const inputRef = React.useRef<HTMLInputElement>(null);
  const commands = React.useMemo(() => paletteCommands(actions), [actions]);
  const filtered = React.useMemo(
    () => filterPaletteCommands(commands, query),
    [commands, query],
  );

  React.useEffect(() => {
    if (!isOpen) return undefined;
    setQuery("");
    setHighlight(0);
    const id = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(id);
  }, [isOpen]);

  React.useEffect(() => setHighlight(0), [query]);

  if (!isOpen) return null;
  const runHighlighted = () => {
    const command = filtered[highlight];
    if (command) {
      command.run();
      onClose();
    }
  };
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlight((h) => (filtered.length ? (h + 1) % filtered.length : 0));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight((h) =>
        filtered.length ? (h - 1 + filtered.length) % filtered.length : 0,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      runHighlighted();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-ground/70 pt-[15vh]"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("shell.palette.placeholder")}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={handleKeyDown}
        className="mx-4 w-full max-w-lg overflow-hidden border border-rule bg-surface"
      >
        <div className="flex items-center gap-2.5 border-b border-rule px-4 py-3">
          <Search width={16} height={16} className="shrink-0 text-secondary" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("shell.palette.placeholder")}
            className="flex-1 bg-transparent text-sm text-primary outline-none placeholder:text-secondary"
          />
          <span className="font-data text-[10px] text-secondary">
            {t("shell.palette.escHint")}
          </span>
        </div>
        <ul className="max-h-80 overflow-y-auto py-1.5" role="listbox">
          {filtered.length === 0 && (
            <li className="px-4 py-3 text-sm text-secondary">
              {t("shell.palette.empty")}
            </li>
          )}
          {filtered.map((command, index) => (
            <li
              key={command.id}
              role="option"
              aria-selected={index === highlight}
            >
              <button
                type="button"
                onMouseEnter={() => setHighlight(index)}
                onClick={() => {
                  command.run();
                  onClose();
                }}
                className={`w-full px-4 py-2.5 text-start text-sm transition-colors ${index === highlight ? "bg-accent/10 text-accent" : "text-primary hover:bg-accent/5"}`}
              >
                {t(command.titleKey)}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
};
