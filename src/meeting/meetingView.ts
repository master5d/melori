export interface MeetingViewAccess {
  engine: string;
  permissions: readonly string[];
}

export function councilTabVisible({
  engine,
  permissions,
}: MeetingViewAccess): boolean {
  return engine === "ready" && permissions.includes("council");
}

export function protectionBadgeVisible(setting: boolean): boolean {
  return setting;
}

/** Panel geometry (logical px) — mirrors window_policy.rs. */
export const PANEL_W = 540;
export const PANEL_H = 520;
export const PILL_H = 52;

/** A primary-button press on the pill row drags the window, unless it lands on a control. */
export function isDragStart(button: number, target: Element | null): boolean {
  if (button !== 0 || !target) return false;
  return target.closest("button, select, input, textarea, a") === null;
}
