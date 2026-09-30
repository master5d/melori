import { describe, expect, it } from "vitest";
import {
  councilTabVisible,
  isDragStart,
  protectionBadgeVisible,
} from "./meetingView";

describe("councilTabVisible", () => {
  it("requires a ready engine and council permission", () => {
    expect(
      councilTabVisible({ engine: "ready", permissions: ["council"] }),
    ).toBe(true);
    expect(
      councilTabVisible({ engine: "down", permissions: ["council"] }),
    ).toBe(false);
    expect(councilTabVisible({ engine: "ready", permissions: [] })).toBe(false);
  });
});

describe("protectionBadgeVisible", () => {
  it("follows the setting", () => {
    expect(protectionBadgeVisible(true)).toBe(true);
    expect(protectionBadgeVisible(false)).toBe(false);
  });
});

describe("isDragStart", () => {
  // minimal Element stand-in (vitest runs without a DOM): closest() answers
  // whether the pressed node sits inside a control
  const node = (insideControl: boolean) =>
    ({ closest: () => (insideControl ? {} : null) }) as unknown as Element;

  it("a primary press on the pill row drags the window", () => {
    expect(isDragStart(0, node(false))).toBe(true);
  });
  it("a press on a control is a click, not a drag", () => {
    expect(isDragStart(0, node(true))).toBe(false);
  });
  it("secondary buttons and missing targets never drag", () => {
    expect(isDragStart(2, node(false))).toBe(false);
    expect(isDragStart(0, null)).toBe(false);
  });
});
