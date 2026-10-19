/**
 * A lesson in place: when its moment comes and no other lesson is open
 * (`queue.ts`), it opens beside the thing it explains; after that its
 * "Lesson" chip brings it back. It never blocks the page and never moves focus
 * on its own.
 */
import { useEffect, useId, useRef, useState, useSyncExternalStore } from 'react';
import type { ReactElement, ReactNode } from 'react';
import './lessons.css';
import { useLessonsText } from './messages';
import { join, openAnchor, release, subscribeQueue } from './queue';
import { LESSONS, RUNGS, STANDARDS, rungNumber } from './registry';
import type { LessonId } from './registry';
import { lessonView, markSeen, subscribeLessons } from './seen';

/** The two halves an anchor places: the chip inline, the lesson below the line. */
export interface LessonParts {
  readonly toggle: ReactNode;
  readonly panel: ReactNode;
}

const NONE: LessonParts = { toggle: null, panel: null };

/** An open book, so the chip never reads as a term's "?" beside it. */
const BookIcon = (): ReactElement => (
  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" focusable="false">
    <path
      d="M8 3.5C6.5 2.5 4.5 2.2 2 2.5v10c2.5-.3 4.5 0 6 1 1.5-1 3.5-1.3 6-1v-10c-2.5-.3-4.5 0-6 1Zm0 0v10"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinejoin="round"
    />
  </svg>
);

/**
 * The lesson `id` at this anchor. `active` is the anchor's own condition, such
 * as "this is the first field": an inactive anchor renders nothing, marks
 * nothing seen and does not wait in the queue.
 */
export const useLesson = (id: LessonId, active = true): LessonParts => {
  const view = useSyncExternalStore(subscribeLessons, () => lessonView(id));
  const opened = useSyncExternalStore(subscribeQueue, openAnchor);
  const t = useLessonsText();
  const [open, setOpen] = useState(false);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const anchor = useId();
  const shown = active && view !== 'off' && t !== null;
  const ours = opened === anchor;
  const waits = shown && (view === 'fresh' || ours);

  useEffect(() => (waits ? join(anchor, id) : undefined), [waits, anchor, id]);
  useEffect(() => {
    if (!shown) {
      setOpen(false);
    } else if (ours) {
      setOpen(true);
      markSeen(id);
    }
  }, [shown, ours, id]);

  if (!shown) {
    return NONE;
  }
  const close = (): void => {
    setOpen(false);
    release(anchor);
  };
  const lesson = LESSONS[id];
  const title = t(lesson.text.title);
  return {
    toggle: (
      <button
        ref={toggleRef}
        type="button"
        className="lesson-toggle"
        aria-expanded={open}
        aria-controls={anchor}
        aria-label={t('lesson.recall', { title })}
        onClick={() => {
          if (open) {
            close();
          } else {
            setOpen(true);
            markSeen(id);
          }
        }}
      >
        <BookIcon />
        {t('lesson.chip')}
      </button>
    ),
    panel: (
      <div
        id={anchor}
        role="note"
        className="lesson"
        aria-label={t('lesson.label', { title })}
        hidden={!open}
      >
        <p className="lesson-step">
          {t('lesson.step', {
            number: rungNumber(lesson.rung),
            count: RUNGS.length,
            rung: t(`rung.${lesson.rung}`),
          })}
        </p>
        <p>
          <strong>{title}.</strong> {t(lesson.text.body)}
        </p>
        <p className="lesson-actions">
          <a href={STANDARDS[lesson.standard]} target="_blank" rel="noreferrer">
            {t('lesson.learnMore', { standard: t(`standard.${lesson.standard}`) })}
          </a>
          <button
            type="button"
            onClick={() => {
              close();
              toggleRef.current?.focus();
            }}
          >
            {t('lesson.gotIt')}
          </button>
        </p>
      </div>
    ),
  };
};

/** "Show lessons", for the user settings fieldset. */
export const LessonsSwitch = ({
  id,
  checked,
  onChange,
}: {
  readonly id: string;
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
}): ReactElement | null => {
  const t = useLessonsText();
  if (t === null) {
    return null;
  }
  return (
    <p className="lessons-switch">
      <label htmlFor={id}>
        <input
          id={id}
          type="checkbox"
          aria-describedby={`${id}-hint`}
          checked={checked}
          onChange={(event) => {
            onChange(event.target.checked);
          }}
        />
        {t('switch.label')}
      </label>
      <span id={`${id}-hint`} className="field-note">
        {t('switch.hint')}
      </span>
    </p>
  );
};
