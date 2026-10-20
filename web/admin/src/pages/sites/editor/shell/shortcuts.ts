/**
 * The editor's window-wide keys: one table from a key press to the command it
 * asks for. The shell runs the command; the Keys popover lists the same keys.
 */

export type EditorCommand = 'save' | 'undo' | 'redo' | 'deselect';

/** The fields of an event this table reads, so a test can pass a plain object. */
export interface KeyPress {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly defaultPrevented: boolean;
  readonly target: EventTarget | null;
}

/** Input types whose own editing keeps the browser's native undo. */
const TEXT_INPUTS = new Set(['', 'text', 'search', 'url', 'email', 'tel', 'password', 'number']);

/**
 * Whether the target edits text, so Escape and Ctrl+Z belong to it: a text
 * input, a textarea, a select, or the HTML editor's editable surface.
 */
export const isTextEntry = (target: EventTarget | null): boolean => {
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUTS.has(target.type);
  }
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true;
  }
  return (
    target instanceof HTMLElement &&
    target.closest('[contenteditable]:not([contenteditable="false"])') !== null
  );
};

/**
 * The command a key press asks for, or `null`. A press a control already
 * handled (a picked-up card's Escape, a popover's Escape) asks for nothing,
 * and Save works from inside a text field as well.
 */
export const commandFor = (press: KeyPress): EditorCommand | null => {
  if (press.defaultPrevented) {
    return null;
  }
  const mod = press.ctrlKey || press.metaKey;
  const key = press.key.toLowerCase();
  if (mod && key === 's') {
    return 'save';
  }
  if (isTextEntry(press.target)) {
    return null;
  }
  if (mod && key === 'z') {
    return press.shiftKey ? 'redo' : 'undo';
  }
  if (press.key === 'Escape' && !mod && !press.shiftKey) {
    return 'deselect';
  }
  return null;
};
