/**
 * The inspector for a selected site region: the Theme page's own controls for
 * the header or the footer, under a banner that says the change is site-wide.
 *
 * Its edits are not the page's. They live in the shell as one `SiteWideEdit`
 * (`null` until the first edit and again after a save), so they survive a
 * return to the page settings, reach the live preview as its
 * `o:theme_settings` override, and save through the Theme page's PATCH with
 * the banner's own Save. It is its own lazy chunk, with the Theme page's
 * messages, loaded on the first region selection.
 */
import type { ReactElement } from 'react';
import { useMemo } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { SaveStatus } from '../../../../app/fields';
import { useSave } from '../../../../app/save';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';
import { sites } from '../../../../i18n/namespaces/sites';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from '../../resourcePageBlocksRegistry';
import { resourcePageBlockDrafts } from '../../resourcePageBlocksSchema';
import { AccessNote, useSiteAccess } from '../../SiteFrame';
import { draftFromStored, resolve, setOverride } from '../../themeEditor/draft';
import type { ThemeDraft, TokenValue } from '../../themeEditor/draft';
import { TokenField } from '../../themeEditor/FineTune';
import { FooterSection } from '../../themeEditor/FooterSection';
import { HeaderSection } from '../../themeEditor/HeaderSection';
import { saveThemeSettings, storedThemeSettings, themeSettingsWrite } from '../../themeEditor/siteTheme';
import { REGION_LABEL } from '../regions';
import type { SiteRegion } from '../regions';
import { InspectorCrumbs } from './InspectorCrumbs';
import '../../themeEditor/theme.css';
import './siteRegion.css';

/** A site-wide edit since the last load or save: the draft, the map it writes, and the region it was made in. */
export interface SiteWideEdit {
  readonly draft: ThemeDraft;
  readonly settings: Readonly<Record<string, JsonValue>>;
  readonly region: SiteRegion;
  /**
   * Whether `settings` differ from the stored map. A draft can differ while
   * its map does not, such as an empty footer column just added, so the draft
   * is kept either way.
   */
  readonly dirty: boolean;
}

/** The structural tokens each region's inspector lists after its content controls. */
const REGION_TOKENS: Readonly<Record<SiteRegion, readonly string[]>> = {
  header: ['header_layout', 'header_style', 'search_display'],
  footer: ['footer_layout', 'footer_tone'],
};

export interface SiteRegionInspectorProps {
  readonly region: SiteRegion;
  readonly siteId: SiteId;
  /** The site's theme settings as last loaded or saved. */
  readonly stored: JsonValue | null;
  readonly edit: SiteWideEdit | null;
  readonly onEdit: (edit: SiteWideEdit | null) => void;
  /** A save stored `settings`; the shell takes them as the new baseline. */
  readonly onSaved: (settings: JsonValue | null) => void;
  readonly onPage: () => void;
}

export const SiteRegionInspector = ({
  region,
  siteId,
  stored,
  edit,
  onEdit,
  onSaved,
  onPage,
}: SiteRegionInspectorProps): ReactElement => {
  const { t } = useTranslator(common, sites, editor);
  const access = useSiteAccess();
  const { state, save } = useSave();
  const storedMap = storedThemeSettings(stored);
  const blocks = useMemo(
    () => resourcePageBlockDrafts(RESOURCE_PAGE_BLOCKS_REGISTRY, storedThemeSettings(stored)),
    [stored],
  );
  const baseline = useMemo(
    () => JSON.stringify(themeSettingsWrite(draftFromStored(storedThemeSettings(stored)), blocks)),
    [stored, blocks],
  );
  const draft = edit?.draft ?? draftFromStored(storedMap);
  const resolved = useMemo(() => resolve(draft), [draft]);
  const label = t(REGION_LABEL[region]);

  const change = (next: ThemeDraft): void => {
    const settings = themeSettingsWrite(next, blocks);
    onEdit({ draft: next, settings, region, dirty: JSON.stringify(settings) !== baseline });
  };
  const setToken = (key: string, value: TokenValue | null): void => {
    change(setOverride(draft, key, value));
  };
  const dirty = edit?.dirty === true;
  const submit = (): void => {
    if (edit === null || !edit.dirty) {
      return;
    }
    save(async () => {
      const site = await saveThemeSettings(siteId, edit.settings);
      onSaved(site.themeSettings);
    });
  };
  const canChange = access?.has('changeSite') !== false;

  return (
    <div className="site-region-inspector">
      <InspectorCrumbs label={label} onPage={onPage} />
      <div className="site-wide-banner" role="group" aria-label={t('region.bannerLabel')}>
        <p className="site-wide-banner-text">{t('region.banner')}</p>
        {canChange ? (
          <p className="site-wide-banner-actions">
            <button
              type="button"
              className="site-wide-save"
              onClick={submit}
              disabled={!dirty || state.status === 'saving'}
            >
              {state.status === 'saving' ? t('field.saving') : t('region.save')}
            </button>
            {dirty ? (
              <span className="editor-dirty" role="status">
                <span className="editor-dirty-dot" aria-hidden="true" />
                {t('region.unsaved')}
              </span>
            ) : null}
          </p>
        ) : null}
        <a href={`#/sites/${String(siteId)}/theme`}>{t('region.openTheme')}</a>
        <SaveStatus state={state} />
      </div>
      <h2>{label}</h2>
      {canChange ? (
        <>
          {region === 'header' ? (
            <HeaderSection
              draft={draft}
              onChange={(key, value) => {
                change({ ...draft, content: { ...draft.content, [key]: value } });
              }}
            />
          ) : (
            <FooterSection
              siteId={siteId}
              footer={draft.footer}
              onChange={(footer) => {
                change({ ...draft, footer });
              }}
            />
          )}
          <fieldset className="theme-section">
            <legend>{t('region.style')}</legend>
            {REGION_TOKENS[region].map((key) => (
              <TokenField key={key} settingKey={key} draft={draft} resolved={resolved} onSet={setToken} />
            ))}
          </fieldset>
        </>
      ) : (
        <AccessNote>{t('siteAccess.theme')}</AccessNote>
      )}
    </div>
  );
};
