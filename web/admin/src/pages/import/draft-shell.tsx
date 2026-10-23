/**
 * The frame of one import: its title and status, the save word, the mode
 * switch, the steps, the status line and the screen's heading and body. The
 * frame owns the import's one `PlanStore`; a route change inside the import,
 * a mode switch included, keeps the frame and so keeps every answer.
 */
import { Suspense, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import type { ImportId } from '../../api/import-plan';
import { importApi } from '../../api/import-routes';
import type { ImportDetail } from '../../api/import-view';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { ADVANCED_BODIES } from './advanced/bodies';
import { DraftContext, useDraft } from './draft-context';
import { FirstLook } from './first-look';
import { usePreview } from './lists/use-preview';
import type { Draft } from './draft-context';
import { useImportText } from './messages';
import { PlanStore } from './plan-store';
import { SaveNotice, SaveWord } from './save-notice';
import { firstPlace, importHref, screenOf } from './screens';
import type { Place } from './screens';
import { SIMPLE_BODIES } from './simple/bodies';
import { StatusLine } from './status-line';
import { ModeSwitch, StepNav, Steps } from './steps';
import { statusKey, whereWords } from './words';

const Body = ({ place }: { readonly place: Place }): ReactElement | null => {
  const { t } = useImportText();
  const Screen = place.mode === 'simple' ? SIMPLE_BODIES[place.screen] : ADVANCED_BODIES[place.screen];
  if (Screen === undefined) {
    return null;
  }
  return (
    <Suspense fallback={<p role="status" className="import-slow">{t('shell.loading')}</p>}>
      <Screen />
    </Suspense>
  );
};

const Frame = ({ initial, place }: { readonly initial: ImportDetail; readonly place: Place }): ReactElement => {
  const [store] = useState(() => new PlanStore(importApi, initial));
  const snapshot = useSyncExternalStore(store.subscribe, store.snapshot);
  const [looks, setLooks] = useState(0);
  useEffect(() => () => {
    store.close();
  }, [store]);
  const draft = useMemo<Draft>(
    () => ({
      ...snapshot,
      id: initial.id,
      place,
      change: (edit) => { store.change(edit); },
      adopt: (next) => { store.adopt(next); },
      looks,
      looked: () => { setLooks((count) => count + 1); },
    }),
    [snapshot, initial.id, place, store, looks],
  );
  return (
    <DraftContext.Provider value={draft}>
      <FrameView store={store} />
    </DraftContext.Provider>
  );
};

/**
 * The frame inside the draft's context. A draft's one read of the sources (the
 * last look) names the import in the title and tells the first look whether
 * the files were ever looked at. A started import reads no preview: its title
 * comes from the plan alone.
 */
const FrameView = ({ store }: { readonly store: PlanStore }): ReactElement => {
  const { t } = useImportText();
  const { detail, place, save } = useDraft();
  const sources = usePreview([], {}, detail.status === 'draft');
  const where = whereWords(detail.plan.sources, t, sources.status === 'ready' ? sources.data.sources : []);
  // The counts say what adding would write, so they mean nothing once the import has started,
  // and nothing on the first screen, before there are files.
  const counted = detail.status === 'draft' && !(place.mode === 'simple' && place.screen === 'files');
  return (
    <section className={`import-draft import-${place.mode}`}>
      <p className="import-up">
        <a href="#/import">{t('shell.home')}</a>
      </p>
      <header className="import-head">
        <div className="import-title">
          <h2>{where}</h2>
          <span className="muted">
            {t(statusKey(detail))}
            {save.status === 'closed' ? null : (
              <>
                {' · '}
                <SaveWord save={save} />
              </>
            )}
          </span>
        </div>
        <ModeSwitch id={detail.id} place={place} />
      </header>
      <SaveNotice id={detail.id} save={save} store={store} />
      <div className="import-layout">
        <Steps id={detail.id} place={place} />
        <div className="import-screen">
          {detail.status === 'draft' ? <FirstLook sources={sources} /> : null}
          {counted ? <StatusLine /> : null}
          <h3 className="import-screen-title">{t(screenOf(place).title)}</h3>
          <Body key={`${place.mode}/${place.screen}`} place={place} />
          <StepNav id={detail.id} place={place} />
        </div>
      </div>
      {detail.status === 'draft' ? (
        <DeleteControl
          label={t('shell.delete')}
          rowName={where}
          onConfirm={async () => {
            await importApi.remove(detail.id);
            window.location.hash = '#/import';
          }}
        />
      ) : null}
    </section>
  );
};

export const DraftShell = ({ id, place }: { readonly id: ImportId; readonly place: Place | null | 'unknown' }): ReactElement => {
  const { t } = useImportText();
  const state = useAsync(() => importApi.get(id), [id]);
  if (place === 'unknown') {
    return (
      <div role="alert">
        <p>{t('shell.badScreen')}</p>
        <p>
          <a href={importHref(id)}>{t('shell.fromStart')}</a>
        </p>
      </div>
    );
  }
  if (state.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <Frame initial={state.data} place={place ?? firstPlace(state.data)} />;
};
