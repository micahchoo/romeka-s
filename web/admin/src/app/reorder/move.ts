/**
 * The one list move every reorderable list makes. A target past either end is
 * clamped to it, and a move that changes nothing returns the same list, so a
 * caller can skip the write by identity.
 */
export const moveItem = <T>(items: readonly T[], from: number, to: number): readonly T[] => {
  const moved = items[from];
  const target = Math.max(0, Math.min(to, items.length - 1));
  if (moved === undefined || target === from) {
    return items;
  }
  const next = items.filter((_, index) => index !== from);
  next.splice(target, 0, moved);
  return next;
};
