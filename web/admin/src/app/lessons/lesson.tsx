/**
 * A lesson in place: the first time its moment comes, it opens beside the
 * thing it explains; after that its "?" brings it back. It never blocks the
 * page and never moves focus on its own.
 */
import { useEffect, useId, useRef, useState, useSyncExternalStore } from 'react';
import type { ReactElement, ReactNode } from 'react';
import './lessons.css';
import { useLessonsText } from './messages';
import { LESSONS, RUNGS, STANDARDS, rungNumber } from './registry';
import type { WrittenLessonId } from './registry';
import { lessonView, markSeen, subscribeLessons } from './seen';

/** The two halves an anchor places: the "?" inline, the lesson below the line. */
export interface LessonParts {
  readonly toggle: ReactNode;
  readonly panel: ReactNode;
}

const NONE: LessonParts = { toggle: null, panel: null };

/**
 * The lesson `id` at this anchor. `active` is the anchor's own condition, such
 * as "this is the first field": an inactive anchor renders nothing and marks
 * nothing seen.
 */
export const useLesson = (id: WrittenLessonId, active = true): LessonParts => {
  const view = useSyncExternalStore(subscribeLessons, () => lessonView(id));
  const t = useLessonsText();
  const [open, setOpen] = useState(false);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  const shown = active && view !== 'off' && t !== null;

  useEffect(() => {
    if (shown && view === 'fresh') {
      setOpen(true);
      markSeen(id);
    }
  }, [shown, view, id]);

  if (!shown) {
    return NONE;
  }
  const lesson = LESSONS[id];
  const title = t(lesson.text.title);
  return {
    toggle: (
      <button
        ref={toggleRef}
        type="button"
        className="lesson-toggle"
        aria-expanded={open}
        aria-controls={panelId}
        aria-label={t('lesson.recall', { title })}
        onClick={() => {
          setOpen(!open);
        }}
      >
        ?
      </button>
    ),
    panel: (
      <div
        id={panelId}
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
              setOpen(false);
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
