/**
 * The parts of one file path that a pattern's named groups take, for showing
 * the pattern on a real name. Only the display reads it: the counts and the
 * groups that make items are the server's. JavaScript reads the same
 * `(?<name>…)` groups as the server's regex; a pattern it cannot compile
 * (syntax only the server knows) shows no parts.
 */

export interface Part {
  readonly text: string;
  /** The innermost named group that holds the text, or `null`. */
  readonly group: string | null;
}

export const partsOf = (pattern: string, path: string): readonly Part[] | null => {
  let found: RegExpExecArray | null;
  try {
    found = new RegExp(pattern, 'du').exec(path);
  } catch {
    return null;
  }
  const spans = found?.indices?.groups;
  if (found === null || spans === undefined) {
    return null;
  }
  const named = Object.entries<readonly [number, number] | undefined>(spans).flatMap(([group, span]) => (span === undefined ? [] : [{ group, start: span[0], end: span[1] }]));
  const groupAt = (at: number): string | null =>
    named
      .filter((span) => span.start <= at && at < span.end)
      .sort((a, b) => b.start - a.start || a.end - b.end)[0]?.group ?? null;
  const parts: Part[] = [];
  for (let at = 0; at < path.length; at += 1) {
    const group = groupAt(at);
    const last = parts[parts.length - 1];
    const char = path.charAt(at);
    if (last !== undefined && last.group === group) {
      parts[parts.length - 1] = { text: last.text + char, group };
    } else {
      parts.push({ text: char, group });
    }
  }
  return parts;
};
