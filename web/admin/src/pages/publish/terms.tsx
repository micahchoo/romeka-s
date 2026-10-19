/**
 * The Publish panel's term table: each term's URI and state, the deprecate
 * control for a published term, and, on a term list, its links to an
 * authority.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import type { Publication, PublicationCommand, PublicationTerm } from '../../api/publication';
import { useLesson } from '../../app/lessons/lesson';
import { usePublishText } from './messages';
import type { PublishMessageKey } from './messages';

/** A command the panel sends, answered by the next view. */
export type Send = (command: PublicationCommand) => Promise<void>;

const isWebUri = (text: string): boolean => {
  if (!/^https?:\/\//.test(text)) {
    return false;
  }
  try {
    return new URL(text).host !== '';
  } catch {
    return false;
  }
};

const stateKey = (term: PublicationTerm): PublishMessageKey => {
  if (term.deprecationPublished) {
    return 'publish.termDeprecated';
  }
  if (term.deprecated) {
    return 'publish.termDeprecationPending';
  }
  return term.published ? 'publish.termPublished' : 'publish.termDraft';
};

/** The term a merge moved a deprecated term's values to. */
const Replacement = ({
  term,
  terms,
}: {
  readonly term: PublicationTerm;
  readonly terms: readonly PublicationTerm[];
}): ReactElement | null => {
  const text = usePublishText();
  const next = terms.find((candidate) => candidate.key === term.replacedBy);
  return next === undefined ? null : (
    <span className="field-hint">{text('publish.replacedBy', { term: next.label })}</span>
  );
};

const TermLinks = ({
  term,
  send,
  busy,
}: {
  readonly term: PublicationTerm;
  readonly send: Send;
  readonly busy: boolean;
}): ReactElement => {
  const text = usePublishText();
  const [exact, setExact] = useState(term.exactMatch ?? '');
  const [close, setClose] = useState(term.closeMatch ?? '');
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState(false);
  const lesson = useLesson('authority-link', opened);
  const id = `publish-links-${encodeURIComponent(term.key).replace(/%/g, '_')}`;
  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const bad = [exact, close].map((value) => value.trim()).some((value) => value !== '' && !isWebUri(value));
    if (bad) {
      setError(text('publish.matchInvalid'));
      return;
    }
    setError(null);
    void send({
      action: 'link',
      key: term.key,
      exact_match: exact.trim() === '' ? null : exact.trim(),
      close_match: close.trim() === '' ? null : close.trim(),
    });
  };
  return (
    <details
      className="publish-links"
      onToggle={(event) => {
        setOpened(event.currentTarget.open);
      }}
    >
      <summary aria-label={text('publish.linksLabel', { term: term.label })}>
        {text('publish.links')}
      </summary>
      <form onSubmit={submit} aria-label={text('publish.linksLabel', { term: term.label })}>
        <p>
          <label htmlFor={`${id}-exact`}>{text('publish.exactMatch')}</label>
          <input
            id={`${id}-exact`}
            type="url"
            value={exact}
            aria-describedby={`${id}-hint`}
            onChange={(event) => {
              setExact(event.target.value);
            }}
          />
        </p>
        <p>
          <label htmlFor={`${id}-close`}>{text('publish.closeMatch')}</label>
          <input
            id={`${id}-close`}
            type="url"
            value={close}
            aria-describedby={`${id}-hint`}
            onChange={(event) => {
              setClose(event.target.value);
            }}
          />
        </p>
        <p id={`${id}-hint`} className="field-hint">
          {text('publish.linksHint')} {lesson.toggle}
        </p>
        {lesson.panel}
        {error === null ? null : <p role="alert">{error}</p>}
        <p className="form-actions">
          <button type="submit" disabled={busy}>
            {text('publish.saveLinks')}
          </button>
        </p>
      </form>
    </details>
  );
};

const TermChange = ({
  term,
  send,
  busy,
}: {
  readonly term: PublicationTerm;
  readonly send: Send;
  readonly busy: boolean;
}): ReactElement | null => {
  const text = usePublishText();
  if (!term.published || term.deprecationPublished) {
    return null;
  }
  return term.deprecated ? (
    <button
      type="button"
      disabled={busy}
      aria-label={text('publish.undeprecateLabel', { term: term.label })}
      onClick={() => {
        void send({ action: 'undeprecate', key: term.key });
      }}
    >
      {text('publish.undeprecate')}
    </button>
  ) : (
    <button
      type="button"
      disabled={busy}
      aria-label={text('publish.deprecateLabel', { term: term.label })}
      onClick={() => {
        void send({ action: 'deprecate', key: term.key });
      }}
    >
      {text('publish.deprecate')}
    </button>
  );
};

export const TermTable = ({
  publication,
  send,
  busy,
}: {
  readonly publication: Publication;
  readonly send: Send;
  readonly busy: boolean;
}): ReactElement => {
  const text = usePublishText();
  const linkable = publication.kind === 'term_list';
  return (
    <div className="table-scroll">
      <table className="publish-terms">
        <caption>{text('publish.termsHeading')}</caption>
        <thead>
          <tr>
            <th scope="col">{text('publish.columnTerm')}</th>
            <th scope="col">{text('publish.columnUri')}</th>
            <th scope="col">{text('publish.columnState')}</th>
            <th scope="col">{text('publish.columnChange')}</th>
          </tr>
        </thead>
        <tbody>
          {publication.terms.map((term) => (
            <tr key={term.key}>
              <td>{term.label}</td>
              <td>
                <code>{term.uri}</code>
              </td>
              <td>
                {text(stateKey(term))}
                <Replacement term={term} terms={publication.terms} />
              </td>
              <td>
                <TermChange term={term} send={send} busy={busy} />
                {linkable ? <TermLinks term={term} send={send} busy={busy} /> : null}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};
