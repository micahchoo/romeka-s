/**
 * The lessons: the registry is a complete ladder, and a lesson in place shows
 * once, writes itself seen to the user's settings, and comes back on its
 * "Lesson" chip. Two lessons whose moments come together open one at a time,
 * lowest rung first. The seen-state arrives with the session user, as
 * `GET /api/me` answers it.
 */
import { act, fireEvent, render, screen } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isRecord, isUnknownArray, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { useLesson } from '../src/app/lessons/lesson';
import { en } from '../src/app/lessons/messages';
import { es } from '../src/app/lessons/locales/es';
import { LESSONS, RUNGS } from '../src/app/lessons/registry';
import type { LessonId } from '../src/app/lessons/registry';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const userJson = (): Record<string, unknown> => {
  const page = loadFixture('users-page.json');
  const entry = isUnknownArray(page) ? page[0] : undefined;
  if (!isRecord(entry)) {
    throw new Error('expected a user at index 0');
  }
  return entry;
};

const userPath = (): string => `/api/users/${String(userJson()['o:id'])}`;

/** Sign in a user whose stored settings are `settings`, as `GET /api/me` answers. */
const signIn = (settings: Record<string, unknown> | null): void => {
  act(() => {
    setSession(parseUser(settings === null ? userJson() : { ...userJson(), 'o:user_settings': settings }));
  });
};

const Anchor = ({
  id,
  active = true,
}: {
  readonly id: LessonId;
  readonly active?: boolean;
}): ReactElement => {
  const lesson = useLesson(id, active);
  return (
    <div>
      <p>
        The anchor {lesson.toggle}
      </p>
      {lesson.panel}
    </div>
  );
};

const seenWrites = (fetchMock: ReturnType<typeof installFetch>): readonly unknown[] =>
  requestsFor(fetchMock, userPath()).map((request) => {
    const settings = requestBody(request)['o:user_settings'];
    return isRecord(settings) ? settings.romeka_lessons_seen : undefined;
  });

const lessonRegion = (title: string): HTMLElement | null =>
  screen.queryByRole('note', { name: `Lesson: ${title}` });

afterEach(() => {
  act(() => {
    setSession(null);
  });
});

describe('the lesson registry', () => {
  const entries = Object.entries(LESSONS);

  it('every_rung_of_the_ladder_has_a_lesson', () => {
    for (const rung of RUNGS) {
      expect(entries.some(([, lesson]) => lesson.rung === rung), rung).toBe(true);
    }
  });

  it('a_lesson_is_one_or_two_sentences_in_english_and_spanish', () => {
    const sentences = (text: string): number => text.split(/[.!?](?:\s|$)/u).filter(Boolean).length;
    for (const [id, lesson] of entries) {
      for (const table of [en, es]) {
        const count = sentences(table[lesson.text.body]);
        expect(count, `${id}: ${table[lesson.text.body]}`).toBeGreaterThanOrEqual(1);
        expect(count, `${id}: ${table[lesson.text.body]}`).toBeLessThanOrEqual(2);
      }
    }
  });
});

describe('a lesson in place', () => {
  it('opens_the_first_time_writes_itself_seen_and_folds_into_its_question_mark', async () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn(null);

    render(<Anchor id="term" />);

    expect(await screen.findByRole('note', { name: 'Lesson: Standard terms' })).toHaveTextContent(
      'dcterms:creator',
    );
    await vi.waitFor(() => {
      expect(seenWrites(fetchMock)).toEqual([['term']]);
    });
    expect(requestsFor(fetchMock, userPath())[0]?.init?.method).toBe('PATCH');

    fireEvent.click(screen.getByRole('button', { name: 'Got it' }));
    expect(lessonRegion('Standard terms')).toBeNull();
    const recall = screen.getByRole('button', { name: 'Show the lesson: Standard terms' });
    expect(recall).toHaveFocus();

    fireEvent.click(recall);
    expect(lessonRegion('Standard terms')).not.toBeNull();
    expect(seenWrites(fetchMock)).toHaveLength(1);
  });

  it('a_lesson_the_user_has_seen_stays_folded_and_its_question_mark_recalls_it', () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn({ romeka_lessons_seen: ['term'] });

    render(<Anchor id="term" />);

    expect(lessonRegion('Standard terms')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Show the lesson: Standard terms' }));
    expect(lessonRegion('Standard terms')).not.toBeNull();
    expect(seenWrites(fetchMock)).toEqual([]);
  });

  it('two_lessons_at_once_open_one_at_a_time_lowest_rung_first', async () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn(null);

    render(
      <>
        <Anchor id="type-class" />
        <Anchor id="term" />
      </>,
    );

    await screen.findByRole('note', { name: 'Lesson: Standard terms' });
    expect(lessonRegion('Standard classes')).toBeNull();
    await vi.waitFor(() => {
      expect(seenWrites(fetchMock)).toEqual([['term']]);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Got it' }));
    await screen.findByRole('note', { name: 'Lesson: Standard classes' });
    expect(lessonRegion('Standard terms')).toBeNull();
    await vi.waitFor(() => {
      expect(seenWrites(fetchMock).at(-1)).toEqual(['term', 'type-class']);
    });
  });

  it('a_waiting_lesson_the_user_leaves_stays_unseen_for_the_next_visit', async () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn(null);

    const page = render(
      <>
        <Anchor id="term" />
        <Anchor id="standards" />
      </>,
    );
    await screen.findByRole('note', { name: 'Lesson: Standard terms' });
    page.unmount();

    await vi.waitFor(() => {
      expect(seenWrites(fetchMock)).toEqual([['term']]);
    });
    render(<Anchor id="standards" />);
    expect(await screen.findByRole('note', { name: 'Lesson: Linked data' })).toBeVisible();
  });

  it('a_lesson_chip_reads_apart_from_a_term_question_mark', () => {
    installFetch(() => jsonResponse(userJson()));
    signIn({ romeka_lessons_seen: ['term'] });

    render(<Anchor id="term" />);

    const chip = screen.getByRole('button', { name: 'Show the lesson: Standard terms' });
    expect(chip).toHaveTextContent('Lesson');
    expect(chip).not.toHaveTextContent('?');
  });

  it('an_inactive_anchor_shows_nothing_and_marks_nothing', async () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn(null);

    render(<Anchor id="term" active={false} />);

    await Promise.resolve();
    expect(screen.queryByRole('button')).toBeNull();
    expect(seenWrites(fetchMock)).toEqual([]);
  });

  it('the_switch_off_hides_every_lesson_and_every_question_mark', () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));
    signIn({ romeka_hide_lessons: true, romeka_lessons_seen: ['term'] });

    render(
      <>
        <Anchor id="term" />
        <Anchor id="standards" />
      </>,
    );

    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.queryByRole('note')).toBeNull();
    expect(seenWrites(fetchMock)).toEqual([]);
  });

  it('with_nobody_signed_in_no_lesson_shows_because_none_could_stay_dismissed', () => {
    const fetchMock = installFetch(() => jsonResponse(userJson()));

    render(<Anchor id="term" />);

    expect(screen.queryByRole('button')).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
