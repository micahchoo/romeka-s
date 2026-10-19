import type { ReactElement } from 'react';
import { termListIdFromParam } from '../../api/term-lists';
import { useTermListText } from './messages';
import { TermListCreatePage, TermListEditPage } from './form';
import { TermListList } from './list';

/** `#/term-lists` lists, `.../new` creates, `.../<id>` edits. */
export const TermListsPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const tl = useTermListText();
  if (param === null) {
    return <TermListList />;
  }
  if (param === 'new') {
    return <TermListCreatePage />;
  }
  const id = termListIdFromParam(param);
  if (id === null) {
    return <p role="alert">{tl('termLists.invalidId', { param })}</p>;
  }
  return <TermListEditPage key={String(id)} id={id} />;
};
