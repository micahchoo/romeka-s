/**
 * The first look at a draft's files. A draft made over a folder (on the start
 * page, through the API, or as a copy) has no list of its files until someone
 * looks at them, and until then every count reads zero. So a draft with a
 * source never looked at looks once when it opens, and says so meanwhile.
 */
import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../app/errors';
import { useDraft } from './draft-context';
import { usePreview } from './lists/use-preview';
import { useLookAgain } from './look-again';
import { useImportText } from './messages';

export const FirstLook = (): ReactElement | null => {
  const { t } = useImportText();
  const { detail } = useDraft();
  const read = usePreview([], {});
  const { look, lookAgain } = useLookAgain();
  const asked = useRef(false);
  const unread = read.status === 'ready' && read.data.sources.some((source) => source.scanned === null && source.error === null);
  useEffect(() => {
    if (unread && detail.status === 'draft' && !asked.current) {
      asked.current = true;
      lookAgain();
    }
  }, [unread, detail.status, lookAgain]);
  if (look.at === 'looking') {
    return (
      <p role="status" className="import-slow">
        {t('shell.firstLook')}
      </p>
    );
  }
  return look.at === 'failed' ? <ErrorNotice error={look.error} /> : null;
};
