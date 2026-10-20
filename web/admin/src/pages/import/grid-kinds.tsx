/**
 * The grid's problems, one line per kind and field with its count, its bulk
 * fixes, Skip them, and Show them, which filters the rows to that kind.
 */
import type { ReactElement } from 'react';
import type { ProblemKind, RowsPreview } from '../../api/import-preview';
import type { ImportPlan } from '../../api/imports';
import { KIND_FIXES, SKIP, offered } from './grid-fixes';
import type { Fix } from './grid-fixes';
import { useImportText } from './messages';
import { KIND_TEXT } from './problem-text';

export interface KindFilter {
  readonly kind: ProblemKind;
  readonly term: string | null;
}

export type KindEntry = RowsPreview['kinds'][number];

export const sameKind = (a: KindFilter | null, b: KindFilter): boolean =>
  a !== null && a.kind === b.kind && a.term === b.term;

/** The line for one kind; `label` names a term as its column heading reads. */
export const useKindText = (label: (term: string) => string): ((entry: KindEntry) => string) => {
  const { t, number } = useImportText();
  return (entry) =>
    t(KIND_TEXT[entry.kind], { field: entry.term === null ? '' : label(entry.term), count: number(entry.count) });
};

export const ProblemKinds = ({
  kinds,
  plan,
  filter,
  editable,
  busy,
  label,
  onShow,
  onFix,
}: {
  readonly kinds: RowsPreview['kinds'];
  readonly plan: ImportPlan;
  readonly filter: KindFilter | null;
  readonly editable: boolean;
  readonly busy: boolean;
  readonly label: (term: string) => string;
  readonly onShow: (filter: KindFilter | null) => void;
  readonly onFix: (fix: Fix, entry: KindEntry) => void;
}): ReactElement => {
  const { t } = useImportText();
  const text = useKindText(label);
  if (kinds.length === 0) {
    return <p className="query-count">{t('kinds.none')}</p>;
  }
  const sorted = [...kinds].sort((a, b) => b.count - a.count);
  return (
    <div className="import-panel">
      <h3>{t('kinds.heading')}</h3>
      <ul className="import-kinds">
        {sorted.map((entry) => {
          const shown = sameKind(filter, entry);
          return (
            <li key={`${entry.kind}|${entry.term ?? ''}`}>
              <span>{text(entry)}</span>{' '}
              <button
                type="button"
                className="import-link"
                aria-pressed={shown}
                onClick={() => { onShow(shown ? null : { kind: entry.kind, term: entry.term }); }}
              >
                {t(shown ? 'grid.showAll' : 'fix.show')}
              </button>
              {[...offered(KIND_FIXES[entry.kind], plan), SKIP].map((fix) => (
                <button
                  key={fix.label}
                  type="button"
                  className="import-link"
                  disabled={!editable || busy}
                  onClick={() => { onFix(fix, entry); }}
                >
                  {t(fix.label)}
                </button>
              ))}
            </li>
          );
        })}
      </ul>
    </div>
  );
};
