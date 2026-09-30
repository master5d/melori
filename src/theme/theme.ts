import { listen } from "@tauri-apps/api/event";
import { commands } from "@/bindings";

export type ThemeSetting = "system" | "day" | "evening";
export type Theme = "day" | "evening";

type ThemeSyncDeps = {
  getSetting: () => Promise<ThemeSetting>;
  onSettingsChanged: (cb: () => void) => () => void;
  matchMedia: (query: string) => MediaQueryList;
  doc: Document;
};

export function resolveTheme(
  setting: ThemeSetting,
  prefersDark: boolean,
): Theme {
  if (setting === "day" || setting === "evening") return setting;
  return prefersDark ? "evening" : "day";
}

export function applyTheme(doc: Document, theme: Theme): void {
  doc.documentElement.dataset.theme = theme;
}

function isThemeSetting(value: unknown): value is ThemeSetting {
  return value === "system" || value === "day" || value === "evening";
}

function defaultDeps(): ThemeSyncDeps {
  return {
    getSetting: async () => {
      try {
        const result = await commands.getAppSettings();
        return result.status === "ok" && isThemeSetting(result.data.ui_theme)
          ? result.data.ui_theme
          : "system";
      } catch {
        return "system";
      }
    },
    onSettingsChanged: (cb) => {
      let unlisten: (() => void) | undefined;
      void listen("settings-changed", () => cb()).then((dispose) => {
        unlisten = dispose;
      });
      return () => unlisten?.();
    },
    matchMedia: (query) => window.matchMedia(query),
    doc: globalThis.document,
  };
}

export function startThemeSync(overrides?: Partial<ThemeSyncDeps>): () => void {
  const deps = { ...defaultDeps(), ...overrides };
  const media = deps.matchMedia("(prefers-color-scheme: dark)");
  let setting: ThemeSetting = "system";

  const apply = () =>
    applyTheme(deps.doc, resolveTheme(setting, media.matches));
  const readSetting = async () => {
    const next = await deps.getSetting();
    setting = isThemeSetting(next) ? next : "system";
    apply();
  };
  const onSystemThemeChanged = (event: MediaQueryListEvent) => {
    if (setting === "system")
      applyTheme(deps.doc, resolveTheme(setting, event.matches));
  };

  media.addEventListener("change", onSystemThemeChanged);
  const unlistenSettings = deps.onSettingsChanged(() => {
    void readSetting();
  });
  void readSetting();

  return () => {
    media.removeEventListener("change", onSystemThemeChanged);
    unlistenSettings();
  };
}
