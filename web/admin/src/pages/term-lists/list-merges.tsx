/**
 * The merge prompts on a list of words: its entries and the values stored
 * with it, grouped by the near-duplicate rule. An entry no value uses yet
 * still counts, so "Co. Cork" in the list is offered beside "Cork".
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { getCensus } from '../../api/tags';
import type { ValueCount } from '../../api/tags';
import type { TermList } from '../../api/types';
import { useAsync } from '../../app/useAsync';
import { MergePrompt } from './merge-prompt';
import { similarGroups } from './similar';
import './term-lists.css';

export const ListMerges = ({
  list,
  onMerged,
}: {
  readonly list: TermList;
  /** Called once a merge or a "different" answer is stored. */
  readonly onMerged: (message: string) => void;
}): ReactElement | null => {
  const scope = { kind: 'list', id: list.id } as const;
  const [version, setVersion] = useState(0);
  const state = useAsync(() => getCensus(api, scope), [list.id, version]);
  if (list.source.kind !== 'terms' || state.status !== 'ready') {
    return null;
  }
  const used = state.data.values;
  const unused: readonly ValueCount[] =
    list.source.terms
      .filter((term) => !used.some((count) => count.value === term))
      .map((term) => ({ value: term, resources: 0, editable: 0 }));
  const groups = similarGroups([...used, ...unused], state.data.distinct);
  return (
    <>
      {groups.map((group) => (
        <MergePrompt
          key={`${String(version)}:${group.map((count) => count.value).join('|')}`}
          scope={scope}
          group={group}
          onDone={(message) => {
            setVersion((current) => current + 1);
            onMerged(message);
          }}
        />
      ))}
    </>
  );
};
