import type { ReactElement } from 'react';
import { useState } from 'react';
import { useStandardsText } from './messages';

/**
 * A term with a "?" that opens its stored `rdfs:comment` in place, so a reader
 * learns what a standard term means without leaving the table. A term with no
 * comment shows no button.
 */
export const DefinedTerm = ({
  term,
  comment,
  children,
}: {
  readonly term: string;
  readonly comment: string | null;
  readonly children: ReactElement | string;
}): ReactElement => {
  const text = useStandardsText();
  const [open, setOpen] = useState(false);
  if (comment === null || comment.trim() === '') {
    return <>{children}</>;
  }
  return (
    <span className="defined-term">
      {children}
      <button
        type="button"
        className="definition-toggle"
        aria-expanded={open}
        aria-label={text('standards.define', { term })}
        onClick={() => {
          setOpen(!open);
        }}
      >
        ?
      </button>
      {open ? <span className="definition">{comment}</span> : null}
    </span>
  );
};
