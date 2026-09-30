/**
 * Pure shell-UX helpers (Task 2 of the Shell UX Polish epic) — no Tauri/React
 * deps so they're plain-Node vitest-testable, matching this repo's pattern
 * for `pillLogic.ts` / `modules.ts`.
 */

/** The Cmd+K palette-trigger glyph, platform-aware: macOS shows the ⌘
 * modifier, everywhere else shows the literal "Ctrl+K". */
export function paletteKbdHint(isMac: boolean): string {
  return isMac ? "⌘K" : "Ctrl+K";
}

/** Section ids owned by no module passed in `moduleSections` (App passes the
 * consult module AND the hidden echo modules) — i.e. the system sections
 * reachable only through the settings gear. Order-preserving over
 * `allSectionIds`. */
export function systemSectionIds(
  allSectionIds: string[],
  moduleSections: string[][],
): string[] {
  const inAnyModule = new Set(moduleSections.flat());
  return allSectionIds.filter((id) => !inAnyModule.has(id));
}
