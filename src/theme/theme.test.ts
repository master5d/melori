import { describe, expect, it, vi } from "vitest";
import { applyTheme, resolveTheme, startThemeSync } from "./theme";

function fakeMql(dark: boolean) {
  const listeners: Array<(e: { matches: boolean }) => void> = [];
  return {
    mql: {
      matches: dark,
      addEventListener: (_: string, cb: any) => listeners.push(cb),
      removeEventListener: vi.fn(),
    } as any,
    flip(next: boolean) {
      listeners.forEach((cb) => cb({ matches: next }));
    },
  };
}

describe("theme", () => {
  const doc = { documentElement: { dataset: {} } } as unknown as Document;

  it("resolves explicit settings", () => {
    expect(resolveTheme("day", true)).toBe("day");
    expect(resolveTheme("evening", false)).toBe("evening");
  });
  it("system follows prefers-color-scheme", () => {
    expect(resolveTheme("system", true)).toBe("evening");
    expect(resolveTheme("system", false)).toBe("day");
  });
  it("unknown setting falls back to system", () => {
    expect(resolveTheme("sepia" as any, true)).toBe("evening");
  });
  it("applies data-theme on html", () => {
    applyTheme(doc, "evening");
    expect(doc.documentElement.dataset.theme).toBe("evening");
  });
  it("follows a live system theme change when setting is system", async () => {
    const m = fakeMql(false);
    startThemeSync({
      getSetting: async () => "system",
      onSettingsChanged: () => () => {},
      matchMedia: () => m.mql,
      doc,
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(doc.documentElement.dataset.theme).toBe("day");
    m.flip(true);
    expect(doc.documentElement.dataset.theme).toBe("evening");
  });
  it("re-applies when settings change in another window", async () => {
    let setting: any = "day";
    let fire = () => {};
    startThemeSync({
      getSetting: async () => setting,
      onSettingsChanged: (cb) => {
        fire = cb;
        return () => {};
      },
      matchMedia: () => fakeMql(false).mql,
      doc,
    });
    await Promise.resolve();
    await Promise.resolve();
    setting = "evening";
    fire();
    await Promise.resolve();
    await Promise.resolve();
    expect(doc.documentElement.dataset.theme).toBe("evening");
  });
});
