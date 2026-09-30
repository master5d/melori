/** Batch selection of a client's sessions on the card. Pure, so it is unit-tested. */

export function toggleOne(
  selected: ReadonlySet<string>,
  id: string,
): Set<string> {
  const next = new Set(selected);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

export function allSelected(
  ids: readonly string[],
  selected: ReadonlySet<string>,
): boolean {
  return ids.length > 0 && ids.every((id) => selected.has(id));
}

/** Select every session, or clear the selection when all are already selected. */
export function toggleAll(
  ids: readonly string[],
  selected: ReadonlySet<string>,
): Set<string> {
  return allSelected(ids, selected) ? new Set() : new Set(ids);
}

/** Drop ids that no longer exist (after a reload or a deletion). */
export function pruneSelection(
  ids: readonly string[],
  selected: ReadonlySet<string>,
): Set<string> {
  const present = new Set(ids);
  return new Set([...selected].filter((id) => present.has(id)));
}
