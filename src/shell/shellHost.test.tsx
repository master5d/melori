import { describe, it, expect } from "vitest";
import { resolveShellComponent } from "./ShellHost";
import { RailShell } from "./RailShell";

// This repo's vitest runs in a plain Node environment (no jsdom /
// @testing-library/react — see vitest.config.ts), so these assert against
// the exact resolver ShellHost uses to pick a shell component, rather than
// rendering to a DOM. That resolver *is* the fallback contract: ShellHost
// renders whatever this function returns.
describe("ShellHost skin resolution", () => {
  it.each(["rail", "home", "deck", "some-future-skin", null, undefined])(
    "renders RailShell for shell_skin %s",
    (skin) => {
      expect(resolveShellComponent(skin)).toBe(RailShell);
    },
  );
});
