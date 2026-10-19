/**
 * The Publish panel's messages, on the term list page and the vocabulary page,
 * read through the `publish` namespace (`src/i18n/namespaces/publish/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { publish } from '../../i18n/namespaces/publish';

export type { PublishMessages } from '../../i18n/namespaces/publish';

/** A Publish panel message id. */
export type PublishMessageKey = KeyOf<typeof publish>;

/** The Publish panel's message lookup in the active locale. */
export const usePublishText = (): ((key: PublishMessageKey, params?: MessageParams) => string) =>
  useTranslator(publish).t;
