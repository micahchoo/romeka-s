/**
 * The unsaved-changes mark and guard every site tab with a Save button shares:
 * Navigation, Theme, Settings and Users. A tab is dirty while what it would
 * save differs from what it last loaded or saved.
 */
import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { useTranslator } from '../../i18n';
import { sites } from '../../i18n/namespaces/sites';
import { useUnsavedGuard } from './editor/shell/useUnsavedGuard';

export interface UnsavedTab {
  readonly dirty: boolean;
  /** Record what a save just stored, so the tab is clean again. */
  readonly saved: () => void;
}

/** `value` is what a save would send; it is compared as JSON. */
export const useUnsavedTab = (value: unknown): UnsavedTab => {
  const { t } = useTranslator(sites);
  const json = JSON.stringify(value);
  const [savedJson, setSavedJson] = useState(json);
  const dirty = json !== savedJson;
  useUnsavedGuard(dirty, t('siteHub.unsavedTabConfirm'));
  return {
    dirty,
    saved: () => {
      setSavedJson(json);
    },
  };
};

/** The Save control with the dirty mark beside it. */
export const SaveBar = ({
  dirty,
  children,
}: {
  readonly dirty: boolean;
  readonly children: ReactNode;
}): ReactElement => {
  const { t } = useTranslator(sites);
  return (
    <div className="save-bar">
      {children}
      {dirty ? (
        <span className="editor-dirty" role="status">
          <span className="editor-dirty-dot" aria-hidden="true" />
          {t('siteHub.unsaved')}
        </span>
      ) : null}
    </div>
  );
};
