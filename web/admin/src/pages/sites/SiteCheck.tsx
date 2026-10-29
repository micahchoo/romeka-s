/**
 * The site check panel on the site hub: one short list a curator reads before
 * sharing the site's link. Each line is a count from `siteCheck.ts`, shown only
 * above zero, with links to where it is fixed. A private site's check first
 * says visitors see none of it, and the counts follow as what visitors meet
 * once it is public. The check runs when the curator presses it, and again on
 * "Check again", so the hub's own load asks nothing more of the server.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf } from '../../i18n';
import { siteCheck } from '../../i18n/namespaces/site-check';
import { sites } from '../../i18n/namespaces/sites';
import { siteHref } from './SiteFrame';
import type { CheckKind, CheckLine, SiteCheckResult } from './siteCheck';

type Key = KeyOf<typeof siteCheck>;

/** Each line's words: one block (on one page), several on one page, several on several. */
const WORDS: Readonly<Record<CheckKind, { readonly one: Key; readonly onePage: Key; readonly other: Key }>> = {
  unwritten: {
    one: 'siteCheck.unwrittenOne',
    onePage: 'siteCheck.unwrittenOnePage',
    other: 'siteCheck.unwritten',
  },
  emptyLists: {
    one: 'siteCheck.emptyListsOne',
    onePage: 'siteCheck.emptyListsOnePage',
    other: 'siteCheck.emptyLists',
  },
  privateResources: {
    one: 'siteCheck.privateOne',
    onePage: 'siteCheck.privateOnePage',
    other: 'siteCheck.private',
  },
  // A line of pages has as many pages as its count, so it has no "on one page" form.
  outsideMenu: {
    one: 'siteCheck.outsideMenuOne',
    onePage: 'siteCheck.outsideMenuOne',
    other: 'siteCheck.outsideMenu',
  },
};

const NOTES: Readonly<Partial<Record<CheckKind, Key>>> = {
  privateResources: 'siteCheck.privateNote',
  outsideMenu: 'siteCheck.outsideMenuNote',
};

const Line = ({ siteId, line }: { readonly siteId: SiteId; readonly line: CheckLine }): ReactElement => {
  const { t, number } = useTranslator(siteCheck);
  const words = WORDS[line.kind];
  const key = line.count === 1 ? words.one : line.pages.length === 1 ? words.onePage : words.other;
  const text = t(key, { count: number(line.count), pages: number(line.pages.length) });
  const note = NOTES[line.kind];
  return (
    <li>
      {line.kind === 'outsideMenu' ? <a href={siteHref(siteId, 'navigation')}>{text}</a> : text}
      {note === undefined ? null : <p className="site-check-note">{t(note)}</p>}
      {line.kind === 'outsideMenu' ? null : (
        <ul className="site-check-pages">
          {line.pages.map((page) => (
            <li key={String(page.id)}>
              <a href={`#/sites/${String(siteId)}/pages/${String(page.id)}`}>{page.title}</a>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
};

const Lines = ({
  siteId,
  lines,
}: {
  readonly siteId: SiteId;
  readonly lines: readonly CheckLine[];
}): ReactElement => (
  <ul className="site-check-lines">
    {lines.map((line) => (
      <Line key={line.kind} siteId={siteId} line={line} />
    ))}
  </ul>
);

const Result = ({
  siteId,
  result,
  settings,
}: {
  readonly siteId: SiteId;
  readonly result: SiteCheckResult;
  /** The curator may open Site settings, where the site is made public. */
  readonly settings: boolean;
}): ReactElement => {
  const { t } = useTranslator(siteCheck, sites);
  if (result.sitePublic) {
    return result.lines.length === 0 ? <p>{t('siteCheck.nothing')}</p> : <Lines siteId={siteId} lines={result.lines} />;
  }
  return (
    <>
      <ul className="site-check-lines">
        <li>
          {t('siteCheck.privateSite')}{' '}
          {settings ? <a href={siteHref(siteId, 'settings')}>{t('page.siteSettings')}</a> : null}
        </li>
      </ul>
      {result.lines.length === 0 ? null : (
        <>
          <p className="site-check-note">{t('siteCheck.oncePublic')}</p>
          <Lines siteId={siteId} lines={result.lines} />
        </>
      )}
    </>
  );
};

export const SiteCheck = ({
  siteId,
  settings,
}: {
  readonly siteId: SiteId;
  /** The curator may open Site settings, so a private site's line links there. */
  readonly settings: boolean;
}): ReactElement => {
  const { t } = useTranslator(siteCheck);
  const [run, setRun] = useState(0);
  const state = useAsync(
    async () => (run === 0 ? null : (await import('./siteCheck')).readSiteCheck(siteId)),
    [siteId, run],
  );
  const running = run > 0 && state.status === 'loading';
  return (
    <section className="site-check" aria-labelledby="site-check-title">
      <h4 id="site-check-title">{t('siteCheck.title')}</h4>
      <p className="site-check-note">{t('siteCheck.intro')}</p>
      {running ? <p role="status">{t('siteCheck.running')}</p> : null}
      {state.status === 'error' ? <ErrorNotice error={state.error} /> : null}
      {state.status === 'ready' && state.data !== null ? <Result siteId={siteId} result={state.data} settings={settings} /> : null}
      {running ? null : (
        <button
          type="button"
          onClick={() => {
            setRun(run + 1);
          }}
        >
          {run === 0 ? t('siteCheck.run') : t('siteCheck.again')}
        </button>
      )}
    </section>
  );
};
