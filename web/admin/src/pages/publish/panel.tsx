/**
 * The Publish panel on a term list page and a vocabulary page: the draft or
 * published state, the address, the terms, the versions and the Turtle the
 * next publish serves. Every change goes through one command
 * (`api/publication.ts`) and the panel shows the view the server answers.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { getPublication, sendPublicationCommand } from '../../api/publication';
import type { Publication, PublicationCommand, PublicationTarget } from '../../api/publication';
import { ErrorNotice } from '../../app/errors';
import { formatDateTime } from '../../app/format';

/** A version's `created`, which the server writes in UTC without a zone. */
const dateOf = (created: string, locale: Locale): string => formatDateTime(`${created}Z`, locale);
import { useAsync, toError } from '../../app/useAsync';
import { useLocale } from '../../i18n';
import type { Locale } from '../../i18n';
import { usePublishText } from './messages';
import { TermTable } from './terms';
import './publish.css';

type NamespaceMode = 'archive' | 'persistent';

const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const PERSISTENT = /^https?:\/\/(w3id\.org|purl\.org)\/\S+[/#]$/;


const archiveUrl = (publication: Publication, slug: string): string =>
  publication.url.replace(/\/vocab\/[^/]+\/$/, `/vocab/${slug}/`);

/** The address fields of a first publish, and the publish button. */
const PublishForm = ({
  publication,
  send,
  busy,
}: {
  readonly publication: Publication;
  readonly send: (command: PublicationCommand) => Promise<void>;
  readonly busy: boolean;
}): ReactElement => {
  const text = usePublishText();
  const [slug, setSlug] = useState(publication.slug);
  const [mode, setMode] = useState<NamespaceMode>('archive');
  const [namespace, setNamespace] = useState('');
  const [error, setError] = useState<string | null>(null);
  const latest = publication.versions.at(-1);
  const url = archiveUrl(publication, slug);
  const chooseAddress = !publication.slugFixed || !publication.namespaceFixed;

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (!publication.slugFixed && !SLUG.test(slug)) {
      setError(text('publish.slugInvalid'));
      return;
    }
    if (!publication.namespaceFixed && mode === 'persistent' && !PERSISTENT.test(namespace.trim())) {
      setError(text('publish.namespaceInvalid'));
      return;
    }
    setError(null);
    void send({
      action: 'publish',
      ...(publication.slugFixed ? {} : { slug }),
      ...(publication.namespaceFixed || mode === 'archive' ? {} : { namespace: namespace.trim() }),
    });
  };

  return (
    <form onSubmit={submit} className="publish-form">
      {chooseAddress ? (
        <>
          {publication.slugFixed ? null : (
            <p>
              <label htmlFor="publish-slug">{text('publish.slugLabel')}</label>
              <input
                id="publish-slug"
                type="text"
                value={slug}
                aria-describedby="publish-slug-hint"
                onChange={(event) => {
                  setSlug(event.target.value);
                }}
              />
              <span id="publish-slug-hint" className="field-hint">
                {text('publish.slugHint', { url })}
              </span>
            </p>
          )}
          {publication.namespaceFixed ? null : (
            <fieldset>
              <legend>{text('publish.namespaceLegend')}</legend>
              <label>
                <input
                  type="radio"
                  name="publish-namespace"
                  checked={mode === 'archive'}
                  onChange={() => {
                    setMode('archive');
                  }}
                />
                {text('publish.namespaceArchive', { url })}
              </label>
              <label>
                <input
                  type="radio"
                  name="publish-namespace"
                  checked={mode === 'persistent'}
                  onChange={() => {
                    setMode('persistent');
                  }}
                />
                {text('publish.namespacePersistent')}
              </label>
              <p className="field-hint">{text('publish.namespaceWhy')}</p>
              {mode === 'persistent' ? (
                <>
                  <p>
                    <label htmlFor="publish-namespace">{text('publish.namespaceLabel')}</label>
                    <input
                      id="publish-namespace"
                      type="url"
                      value={namespace}
                      aria-describedby="publish-namespace-hint"
                      onChange={(event) => {
                        setNamespace(event.target.value);
                      }}
                    />
                    <span id="publish-namespace-hint" className="field-hint">
                      {text('publish.namespaceHint')}
                    </span>
                  </p>
                  <p>{text('publish.redirectHelp')}</p>
                  <pre className="publish-code">{`RewriteEngine on\nRewriteRule ^(.*)$ ${url}$1 [R=303,L]`}</pre>
                </>
              ) : null}
            </fieldset>
          )}
        </>
      ) : null}
      {publication.state === 'draft' ? <p className="publish-promise">{text('publish.promise')}</p> : null}
      {latest !== undefined && !publication.changed ? (
        <p>{text('publish.noChanges', { version: latest.version })}</p>
      ) : null}
      {latest !== undefined && publication.changed ? (
        <p>{text('publish.pending', { version: latest.version })}</p>
      ) : null}
      {error === null ? null : <p role="alert">{error}</p>}
      <p className="form-actions">
        <button type="submit" disabled={busy || !publication.changed}>
          {latest === undefined
            ? text('publish.publish')
            : text('publish.publishVersion', { version: latest.version + 1 })}
        </button>
      </p>
    </form>
  );
};

const Versions = ({ publication }: { readonly publication: Publication }): ReactElement | null => {
  const text = usePublishText();
  const locale = useLocale();
  if (publication.versions.length === 0) {
    return null;
  }
  return (
    <>
      <h4>{text('publish.versionsHeading')}</h4>
      <ul className="publish-versions">
        {[...publication.versions].reverse().map((version) => (
          <li key={version.version}>
            <a href={`${publication.url}?version=${String(version.version)}`}>
              {text('publish.versionLine', {
                version: version.version,
                date: dateOf(version.created, locale),
                terms: version.terms,
              })}
            </a>
          </li>
        ))}
      </ul>
    </>
  );
};

const Body = ({
  target,
  initial,
}: {
  readonly target: PublicationTarget;
  readonly initial: Publication;
}): ReactElement => {
  const text = usePublishText();
  const locale = useLocale();
  const [publication, setPublication] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const send = async (command: PublicationCommand): Promise<void> => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const next = await sendPublicationCommand(api, target, command);
      setPublication(next);
      const latest = next.versions.at(-1);
      if (command.action === 'publish' && latest !== undefined) {
        setNotice(text('publish.published', { version: latest.version }));
      }
    } catch (failure: unknown) {
      setError(toError(failure));
    } finally {
      setBusy(false);
    }
  };

  if (publication.refusal !== null) {
    return <p className="publish-refusal">{publication.refusal}</p>;
  }
  const latest = publication.versions.at(-1);
  return (
    <>
      <p className="publish-state">
        {latest === undefined
          ? text('publish.stateDraft')
          : text('publish.statePublished', { version: latest.version, date: dateOf(latest.created, locale) })}
      </p>
      <dl className="publish-address">
        {latest === undefined ? null : (
          <>
            <dt>{text('publish.publicUrl')}</dt>
            <dd>
              <a href={publication.url}>{publication.url}</a>
            </dd>
          </>
        )}
        {publication.namespaceFixed || latest !== undefined ? (
          <>
            <dt>{text('publish.namespace')}</dt>
            <dd>
              <code>{publication.namespace}</code>
            </dd>
          </>
        ) : null}
      </dl>
      <TermTable publication={publication} send={send} busy={busy} />
      <PublishForm
        key={publication.versions.length}
        publication={publication}
        send={send}
        busy={busy}
      />
      {notice === null ? null : <p role="status">{notice}</p>}
      {error === null ? null : <ErrorNotice error={error} />}
      <Versions publication={publication} />
      <details className="publish-preview">
        <summary>{text('publish.previewHeading')}</summary>
        <pre className="publish-code">{publication.preview}</pre>
      </details>
    </>
  );
};

/** The Publish section of one term list or vocabulary. */
export const PublishPanel = ({ target }: { readonly target: PublicationTarget }): ReactElement => {
  const text = usePublishText();
  const state = useAsync(() => getPublication(api, target), [target.kind, target.id]);
  return (
    <section className="publish-panel" aria-labelledby="publish-heading">
      <h3 id="publish-heading">{text('publish.heading')}</h3>
      <p className="lede">
        {target.kind === 'term_list' ? text('publish.ledeTermList') : text('publish.ledeVocabulary')}
      </p>
      {state.status === 'loading' ? <p role="status">{text('publish.loading')}</p> : null}
      {state.status === 'error' ? <ErrorNotice error={state.error} /> : null}
      {state.status === 'ready' ? <Body target={target} initial={state.data} /> : null}
    </section>
  );
};
