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
import type { Read } from './lists/use-preview';
import type { PreviewWith } from '../../api/import-sections';
import { useLookAgain } from './look-again';
import { useImportText } from './messages';

/** `sources` is the frame's read of the last look, which the title shares. */
export const FirstLook = ({ sources: read }: { readonly sources: Read<PreviewWith<never>> }): ReactElement | null => {
  const { t } = useImportText();
  const { detail } = useDraft();
  const { look, lookAgain } = useLookAgain();
  const asked = useRef(false);
  const unread = read.status === 'ready' && read.data.sources.some((source) => source.scanned === null && source.error === null);
  useEffect(() => {
    if (unread && detail.status === 'draft' && !asked.current) {
      asked.current = true;
      lookAgain();
    }
  }, [unread, detail.status, lookAgain]);
  if (look.at === 'looking' || look.at === 'working') {
    return (
      <p role="status" className="import-slow">
        {t('shell.firstLook')}
      </p>
    );
  }
  return look.at === 'failed' ? <ErrorNotice error={look.error} /> : null;
};
