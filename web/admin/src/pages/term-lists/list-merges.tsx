/**
 * The merge prompts on a list of words: its entries and the values stored
 * with it, grouped by the near-duplicate rule. An entry no value uses yet
 * still counts, so "Co. Cork" in the list is offered beside "Cork". A
 * deprecated entry no value uses is done with, so it is not offered again.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { getDeprecatedKeys } from '../../api/publication';
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
  const state = useAsync(
    () => Promise.all([getCensus(api, scope), getDeprecatedKeys(api, list.id)]),
    [list.id, version],
  );
  if (list.source.kind !== 'terms' || state.status !== 'ready') {
    return null;
  }
  const [census, deprecated] = state.data;
  const used = census.values;
  const unused: readonly ValueCount[] = list.source.terms
    .filter((term) => !deprecated.has(term) && !used.some((count) => count.value === term))
    .map((term) => ({ value: term, resources: 0, editable: 0 }));
  const groups = similarGroups([...used, ...unused], census.distinct);
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
