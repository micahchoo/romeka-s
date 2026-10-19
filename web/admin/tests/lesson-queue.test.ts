/**
 * The lesson queue: at most one lesson open at a time, the lowest rung of the
 * ones that arrive together first, the next one only once the open one is
 * released.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { join, openAnchor, release } from '../src/app/lessons/queue';

const withdrawals: (() => void)[] = [];

const waitFor = (anchor: string, lesson: Parameters<typeof join>[1]): void => {
  withdrawals.push(join(anchor, lesson));
};

/** Let the queue choose, as it does after the render that joined. */
const settled = async (): Promise<string | null> => {
  await Promise.resolve();
  return openAnchor();
};

afterEach(() => {
  for (const withdraw of withdrawals.splice(0)) {
    withdraw();
  }
});

describe('the lesson queue', () => {
  it('opens_the_lowest_rung_of_lessons_that_arrive_together', async () => {
    waitFor('publish-panel', 'publish');
    waitFor('links', 'authority-link');
    waitFor('merge', 'term-merge');

    expect(await settled()).toBe('merge');
  });

  it('within_one_rung_follows_the_ladder_order_then_arrival', async () => {
    waitFor('promote', 'tag-promote');
    waitFor('merge-two', 'term-merge');
    waitFor('merge-one', 'term-merge');

    expect(await settled()).toBe('merge-two');
  });

  it('opens_the_next_only_after_the_open_one_is_released', async () => {
    waitFor('merge', 'term-merge');
    expect(await settled()).toBe('merge');

    waitFor('field', 'term');
    expect(await settled()).toBe('merge');

    release('merge');
    expect(await settled()).toBe('field');
  });

  it('an_open_lesson_that_leaves_the_page_lets_the_next_open', async () => {
    const leave = join('links', 'authority-link');
    waitFor('publish-panel', 'publish');
    expect(await settled()).toBe('links');

    leave();
    expect(await settled()).toBe('publish-panel');
  });

  it('a_waiting_lesson_that_leaves_never_opens', async () => {
    waitFor('merge', 'term-merge');
    const leave = join('publish-panel', 'publish');
    expect(await settled()).toBe('merge');

    leave();
    release('merge');
    expect(await settled()).toBeNull();
  });
});
