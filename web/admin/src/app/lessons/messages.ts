/**
 * The lessons' messages, read through the `lessons` namespace
 * (`src/i18n/namespaces/lessons/`). A lesson never holds up its page, so the
 * lookup is `null`, and the lesson renders nothing, until its table arrives.
 */
import { useLoadedTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { lessons } from '../../i18n/namespaces/lessons';

export { en } from '../../i18n/namespaces/lessons';
export type { LessonsMessages } from '../../i18n/namespaces/lessons';

/** A lessons message id. */
export type LessonsMessageKey = KeyOf<typeof lessons>;

/** A lookup into the lessons table. */
export type LessonsText = (key: LessonsMessageKey, params?: MessageParams) => string;

/** The active locale's lookup, or `null` until its table arrives. */
export const useLessonsText = (): LessonsText | null => useLoadedTranslator(lessons)?.t ?? null;
