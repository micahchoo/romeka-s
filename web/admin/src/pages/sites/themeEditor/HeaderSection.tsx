/**
 * The header content: logo, banner and banner text, each with its help line.
 * The Theme page and the page editor's site header inspector both show it.
 */
import type { ReactElement } from 'react';
import { AssetPicker } from '../editor/fields/AssetPicker';
import type { ContentKey, ThemeDraft } from './draft';
import { useThemeText } from './messages';
import { settingFor } from './registry';

export const HeaderSection = ({
  draft,
  onChange,
}: {
  readonly draft: ThemeDraft;
  readonly onChange: (key: ContentKey, value: string) => void;
}): ReactElement => {
  const tt = useThemeText();
  const help = (key: ContentKey): ReactElement | null => {
    const setting = settingFor(key);
    return setting === undefined ? null : <p className="field-note theme-help">{tt(setting.helpKey)}</p>;
  };
  return (
    <fieldset className="theme-section">
      <legend>{tt('theme.header')}</legend>
      <AssetPicker
        id="theme-setting-logo"
        label={tt('themeSetting.logo')}
        value={draft.content.logo}
        accepts="image"
        decorative={false}
        onChange={(value) => {
          onChange('logo', value);
        }}
      />
      {help('logo')}
      <AssetPicker
        id="theme-setting-banner"
        label={tt('themeSetting.banner')}
        value={draft.content.banner}
        accepts="image"
        decorative={false}
        onChange={(value) => {
          onChange('banner', value);
        }}
      />
      {help('banner')}
      <p>
        <label htmlFor="theme-setting-banner_text">{tt('themeSetting.bannerText')}</label>
        <input
          id="theme-setting-banner_text"
          type="text"
          value={draft.content.banner_text}
          onChange={(event) => {
            onChange('banner_text', event.target.value);
          }}
        />
      </p>
      {help('banner_text')}
    </fieldset>
  );
};
