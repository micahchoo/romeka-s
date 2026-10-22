/**
 * Where the person is in one import: the mode switch, the Simple step bar or
 * the Advanced side index, and Simple's Back and Next. Every link names a
 * place (`screens.ts`), so the browser's own back button walks the same path.
 */
import type { ReactElement } from 'react';
import type { ImportId } from '../../api/import-plan';
import { useHash } from '../../app/router';
import { useImportText } from './messages';
import { counterpart, parentOf, placeHref, samePlace, screenOf, stepsOf } from './screens';
import type { Mode, Place } from './screens';

const MODE_KEYS = {
  simple: { name: 'shell.simple', note: 'shell.simpleNote' },
  advanced: { name: 'shell.advanced', note: 'shell.advancedNote' },
} as const;

/**
 * Two links, one per mode; the other mode's link lands on the screen that
 * shows the same thing. Both zooms of Structure read the node the address
 * names (`picked-node.ts`), so the switch keeps it.
 */
export const ModeSwitch = ({ id, place }: { readonly id: ImportId; readonly place: Place }): ReactElement => {
  const { t } = useImportText();
  const other = counterpart(place);
  const node = useHash().split('/')[5];
  const kept = node === undefined || other.screen !== 'structure' ? '' : `/${node}`;
  const hrefOf = (mode: Mode): string => (mode === place.mode ? placeHref(id, place) : placeHref(id, other) + kept);
  return (
    <div className="import-mode-row">
      <div className="import-mode" role="group" aria-label={t('shell.mode')}>
        {(['simple', 'advanced'] as const).map((mode) => (
          <a key={mode} href={hrefOf(mode)} aria-current={mode === place.mode ? 'page' : undefined}>
            {t(MODE_KEYS[mode].name)}
          </a>
        ))}
      </div>
      <span className="muted">{t(MODE_KEYS[place.mode].note)}</span>
    </div>
  );
};

/** Simple's numbered steps, or Advanced's index of settings; a sub-screen marks its parent. */
export const Steps = ({ id, place }: { readonly id: ImportId; readonly place: Place }): ReactElement => {
  const { t } = useImportText();
  const { steps, current } = stepsOf(place);
  const links = steps.map((step) => {
    const key = screenOf(step).step;
    const here = samePlace(step, current);
    return (
      <li key={step.screen}>
        <a href={placeHref(id, step)} aria-current={here ? (place.mode === 'simple' ? 'step' : 'page') : undefined}>
          {key === undefined ? step.screen : t(key)}
        </a>
      </li>
    );
  });
  return place.mode === 'simple' ? (
    <ol className="import-steps" aria-label={t('shell.steps')}>
      {links}
    </ol>
  ) : (
    <nav className="import-index" aria-label={t('shell.settings')}>
      <ul>{links}</ul>
    </nav>
  );
};

/** Back to the step before (or to the screen a sub-screen opens from), and Next to the step after. */
export const StepNav = ({ id, place }: { readonly id: ImportId; readonly place: Place }): ReactElement | null => {
  const { t } = useImportText();
  if (place.mode !== 'simple') {
    return null;
  }
  const { steps, current } = stepsOf(place);
  const parent = parentOf(place);
  const index = steps.findIndex((step) => samePlace(step, current));
  const back = parent ?? steps[index - 1];
  const next = parent === undefined ? steps[index + 1] : undefined;
  return (
    <div className="import-step-nav">
      {back === undefined ? <span /> : <a className="import-back" href={placeHref(id, back)}>{t('shell.back')}</a>}
      {next === undefined ? null : (
        <a className="button-link" href={placeHref(id, next)}>
          {t('shell.next')}
        </a>
      )}
    </div>
  );
};
