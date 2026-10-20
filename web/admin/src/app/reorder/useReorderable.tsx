/**
 * One reorder model for every flat list in the admin, after the page canvas's
 * keyboard pattern.
 *
 * - The ⠿ handle is a button. Activating it (Space or a click) picks the row
 *   up; while it is up, the arrows move it, Space drops it and Escape puts the
 *   whole list back as it was at the pick-up.
 * - The handle is also an HTML5 drag source; a drop on another row of the
 *   same list moves the dragged row to that row's place.
 * - Optional Up and Down icon buttons move one place, for anyone who neither
 *   drags nor uses the keys.
 * - Every step is announced through the list's live region.
 *
 * The caller owns the list: every change leaves through `onReorder`, so the
 * hook works over a draft field, a reducer slice or a local state alike.
 */
import { useEffect, useId, useRef, useState } from 'react';
import type { DragEvent, KeyboardEvent, ReactElement } from 'react';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { moveItem } from './move';
import './reorder.css';

export interface ReorderableOptions<T> {
  readonly items: readonly T[];
  readonly onReorder: (items: readonly T[]) => void;
  /** The row's name in labels and announcements. */
  readonly labelOf: (item: T, index: number) => string;
}

type Part = 'handle' | 'up' | 'down';

/** The control that should hold focus after a move re-renders the list. */
interface FocusTarget {
  readonly index: number;
  readonly part: Part;
}

export interface Reorderable {
  readonly count: number;
  readonly labelAt: (index: number) => string;
  /** The index of the row that is picked up, `null` when none is. */
  readonly picked: number | null;
  readonly announcement: string;
  readonly pick: (index: number) => void;
  readonly key: (index: number, event: KeyboardEvent<HTMLButtonElement>) => void;
  readonly moveBy: (index: number, delta: -1 | 1, part: Part) => void;
  readonly register: (index: number, part: Part, element: HTMLButtonElement | null) => void;
  readonly dragStart: (index: number, event: DragEvent<HTMLElement>) => void;
  readonly dragEnd: () => void;
  readonly row: (index: number) => RowProps;
}

/** What a row element spreads so it takes a drop. */
export interface RowProps {
  readonly 'data-reorder-picked'?: 'true';
  readonly 'data-reorder-drop'?: 'true';
  readonly onDragOver: (event: DragEvent<HTMLElement>) => void;
  readonly onDragLeave: () => void;
  readonly onDrop: (event: DragEvent<HTMLElement>) => void;
}

const DELTAS: Readonly<Record<string, -1 | 1>> = { ArrowUp: -1, ArrowDown: 1 };

export const useReorderable = <T,>({
  items,
  onReorder,
  labelOf,
}: ReorderableOptions<T>): Reorderable => {
  const { t } = useTranslator(common);
  // A drag carries this list's own type, so a row of another list never drops here.
  const dragType = `application/x-romeka-reorder-${useId().replace(/[^a-zA-Z0-9]/g, '').toLowerCase()}`;
  const [picked, setPicked] = useState<{ readonly index: number; readonly before: readonly T[] } | null>(
    null,
  );
  const [dragging, setDragging] = useState<number | null>(null);
  const [dropAt, setDropAt] = useState<number | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const [focus, setFocus] = useState<FocusTarget | null>(null);
  const controls = useRef(new Map<string, HTMLButtonElement>());

  useEffect(() => {
    if (focus === null) {
      return;
    }
    const wanted = controls.current.get(`${String(focus.index)}:${focus.part}`);
    // A move to an edge disables the button that made it, so focus falls back to the handle.
    const target =
      wanted === undefined || wanted.disabled
        ? controls.current.get(`${String(focus.index)}:handle`)
        : wanted;
    target?.focus();
  }, [focus, items]);

  const labelAt = (index: number): string => {
    const item = items[index];
    return item === undefined ? '' : labelOf(item, index);
  };

  const place = (index: number): { readonly position: number; readonly count: number } => ({
    position: index + 1,
    count: items.length,
  });

  const move = (from: number, to: number, part: Part): number | null => {
    const next = moveItem(items, from, to);
    if (next === items) {
      return null;
    }
    const at = Math.max(0, Math.min(to, items.length - 1));
    onReorder(next);
    setFocus({ index: at, part });
    return at;
  };

  const pick = (index: number): void => {
    const label = labelAt(index);
    if (picked?.index === index) {
      setPicked(null);
      setAnnouncement(t('reorder.dropped', { label, ...place(index) }));
      return;
    }
    setPicked({ index, before: items });
    setAnnouncement(t('reorder.picked', { label }));
  };

  const key = (index: number, event: KeyboardEvent<HTMLButtonElement>): void => {
    if (picked?.index !== index) {
      return;
    }
    const label = labelAt(index);
    if (event.key === 'Escape') {
      event.preventDefault();
      const back = picked.before;
      setPicked(null);
      onReorder(back);
      const home = back.findIndex((entry) => entry === items[index]);
      setFocus({ index: Math.max(0, home), part: 'handle' });
      setAnnouncement(t('reorder.cancelled', { label }));
      return;
    }
    const delta = DELTAS[event.key];
    if (delta === undefined) {
      return;
    }
    event.preventDefault();
    const at = move(index, index + delta, 'handle');
    if (at !== null) {
      setPicked({ index: at, before: picked.before });
      setAnnouncement(t('reorder.moved', { label, ...place(at) }));
    }
  };

  const moveBy = (index: number, delta: -1 | 1, part: Part): void => {
    const label = labelAt(index);
    const at = move(index, index + delta, part);
    if (at !== null) {
      setAnnouncement(t('reorder.moved', { label, ...place(at) }));
    }
  };

  const register = (index: number, part: Part, element: HTMLButtonElement | null): void => {
    const id = `${String(index)}:${part}`;
    if (element === null) {
      controls.current.delete(id);
    } else {
      controls.current.set(id, element);
    }
  };

  const dragStart = (index: number, event: DragEvent<HTMLElement>): void => {
    event.dataTransfer.setData(dragType, String(index));
    event.dataTransfer.effectAllowed = 'move';
    setDragging(index);
  };

  const dragEnd = (): void => {
    setDragging(null);
    setDropAt(null);
  };

  const row = (index: number): RowProps => ({
    ...(picked?.index === index ? { 'data-reorder-picked': 'true' } : {}),
    ...(dropAt === index && dragging !== index ? { 'data-reorder-drop': 'true' } : {}),
    onDragOver: (event) => {
      if (event.dataTransfer.types.includes(dragType)) {
        event.preventDefault();
        event.dataTransfer.dropEffect = 'move';
        if (dropAt !== index) {
          setDropAt(index);
        }
      }
    },
    onDragLeave: () => {
      if (dropAt === index) {
        setDropAt(null);
      }
    },
    onDrop: (event) => {
      const raw = event.dataTransfer.getData(dragType);
      if (raw === '') {
        return;
      }
      event.preventDefault();
      const from = Number(raw);
      const label = labelAt(from);
      dragEnd();
      const at = move(from, index, 'handle');
      if (at !== null) {
        setAnnouncement(t('reorder.dropped', { label, ...place(at) }));
      }
    },
  });

  return {
    count: items.length,
    labelAt,
    picked: picked?.index ?? null,
    announcement,
    pick,
    key,
    moveBy,
    register,
    dragStart,
    dragEnd,
    row,
  };
};

/** The ⠿ handle: pick up with Space, drag with the pointer. */
export const ReorderHandle = ({
  reorder,
  index,
}: {
  readonly reorder: Reorderable;
  readonly index: number;
}): ReactElement => {
  const { t } = useTranslator(common);
  const label = reorder.labelAt(index);
  return (
    <button
      type="button"
      className="reorder-handle"
      draggable
      ref={(element) => {
        reorder.register(index, 'handle', element);
      }}
      title={t('reorder.grip')}
      aria-label={t('reorder.handle', { label, position: index + 1, count: reorder.count })}
      aria-pressed={reorder.picked === index}
      onClick={() => {
        reorder.pick(index);
      }}
      onKeyDown={(event) => {
        reorder.key(index, event);
      }}
      onDragStart={(event) => {
        reorder.dragStart(index, event);
      }}
      onDragEnd={reorder.dragEnd}
    >
      <span aria-hidden="true">⠿</span>
    </button>
  );
};

/** Up and Down as small icon buttons, each with its tooltip. */
export const ReorderButtons = ({
  reorder,
  index,
}: {
  readonly reorder: Reorderable;
  readonly index: number;
}): ReactElement => {
  const { t } = useTranslator(common);
  const label = reorder.labelAt(index);
  return (
    <span className="reorder-buttons">
      <button
        type="button"
        className="reorder-step"
        ref={(element) => {
          reorder.register(index, 'up', element);
        }}
        title={t('reorder.upTip')}
        aria-label={t('reorder.up', { label })}
        disabled={index === 0}
        onClick={() => {
          reorder.moveBy(index, -1, 'up');
        }}
      >
        <span aria-hidden="true">↑</span>
      </button>
      <button
        type="button"
        className="reorder-step"
        ref={(element) => {
          reorder.register(index, 'down', element);
        }}
        title={t('reorder.downTip')}
        aria-label={t('reorder.down', { label })}
        disabled={index === reorder.count - 1}
        onClick={() => {
          reorder.moveBy(index, 1, 'down');
        }}
      >
        <span aria-hidden="true">↓</span>
      </button>
    </span>
  );
};

/** The list's live region. It stays mounted so a screen reader hears each change. */
export const ReorderStatus = ({ reorder }: { readonly reorder: Reorderable }): ReactElement => (
  <p className="reorder-status" role="status" aria-live="polite">
    {reorder.announcement}
  </p>
);
