/**
 * A file outside the annotations folder that imports its wire module. ESLint
 * must refuse it (`no-restricted-imports`); `tests/annotations-wire.test.ts`
 * is the check. The default `eslint .` run ignores this directory, so the
 * fixture can only fail when a test asks for that failure.
 */
import { fromW3C } from '../../src/pages/media/annotations/wire';

export const readNote = fromW3C;
