import { describe, expect, it } from "vitest";
import {
  allSelected,
  pruneSelection,
  toggleAll,
  toggleOne,
} from "./sessionSelection";

const ids = ["2026-09-27-01", "2026-09-28-01", "2026-09-28-02"];

describe("session selection", () => {
  it("toggles a single session in and out", () => {
    const one = toggleOne(new Set(), ids[0]);
    expect([...one]).toEqual([ids[0]]);
    expect([...toggleOne(one, ids[0])]).toEqual([]);
  });
  it("select-all selects everything, then clears", () => {
    const all = toggleAll(ids, new Set([ids[1]]));
    expect(allSelected(ids, all)).toBe(true);
    expect([...toggleAll(ids, all)]).toEqual([]);
  });
  it("an empty list is never 'all selected'", () => {
    expect(allSelected([], new Set())).toBe(false);
  });
  it("drops deleted sessions from the selection", () => {
    expect([...pruneSelection([ids[2]], new Set([ids[0], ids[2]]))]).toEqual([
      ids[2],
    ]);
  });
});
