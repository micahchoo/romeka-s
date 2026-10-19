import type { ReactElement } from 'react';
import type { Rewrite } from '../../api/tags';
import { useTermListText } from './messages';
import type { TermListMessageKey } from './messages';

/** What a bulk rewrite would change, shown before it runs. */
export const RewriteCounts = ({
  rewrite,
  skipped = 'rewrite.skipped',
}: {
  readonly rewrite: Rewrite;
  /** What happens to the resources the caller may not edit. */
  readonly skipped?: TermListMessageKey;
}): ReactElement => {
  const tl = useTermListText();
  return (
    <dl className="rewrite-counts">
      <dt>{tl('rewrite.changes')}</dt>
      <dd>{rewrite.changed}</dd>
      {rewrite.skipped === 0 ? null : (
        <>
          <dt>{tl(skipped)}</dt>
          <dd>{rewrite.skipped}</dd>
        </>
      )}
    </dl>
  );
};
