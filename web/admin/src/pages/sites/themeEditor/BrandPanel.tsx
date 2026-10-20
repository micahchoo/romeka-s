/**
 * The brand colour, the switch that derives the palette from it, the
 * swatches of the palette the page will show, and the contrast readout with a
 * passing colour offered for each failing pair.
 */
import type { ReactElement } from 'react';
import { useThemeText } from './messages';
import type { ThemeMessageKey } from './messages';
import { COLOUR_ROLES, roleColour } from './palette';
import type { PairCheck, Role, Shade } from './palette';
import { THEME_REGISTRY, settingFor } from './registry';
import { shownShades } from './draft';
import type { Resolved, ThemeDraft } from './draft';
import { ColourInput, choiceKey } from './controls';

const ROLES: readonly Role[] = [...COLOUR_ROLES.slice(0, 1), 'on-brand', ...COLOUR_ROLES.slice(1)];

/** The label of a colour role: its token's label, or "Text on brand". */
export const roleLabelKey = (role: Role): ThemeMessageKey => {
  if (role === 'on-brand') {
    return 'theme.onBrand';
  }
  const key = THEME_REGISTRY.colours.find((colour) => colour.role === role)?.key ?? '';
  return settingFor(key)?.labelKey ?? 'theme.palette';
};

const shadeKey = (shade: Shade): ThemeMessageKey => choiceKey(shade) ?? 'theme.palette';

export const BrandPanel = ({
  draft,
  resolved,
  onBrand,
  onDerive,
}: {
  readonly draft: ThemeDraft;
  readonly resolved: Resolved;
  readonly onBrand: (value: string) => void;
  readonly onDerive: (derive: boolean) => void;
}): ReactElement => {
  const t = useThemeText();
  const shown = resolved.scheme === 'dark' ? resolved.dark : resolved.light;
  const brandValue = draft.overrides['brand_color'];
  const brand = typeof brandValue === 'string' ? brandValue : shown.brand;
  return (
    <section className="theme-section" aria-labelledby="theme-brand">
      <h3 id="theme-brand">{t('themeSetting.brandColor')}</h3>
      <div className="theme-row">
        <label htmlFor="theme-setting-brand_color">{t('themeSetting.brandColor')}</label>
        <ColourInput
          id="theme-setting-brand_color"
          label={t('themeSetting.brandColor')}
          value={brand}
          fallback={shown.brand}
          onChange={onBrand}
        />
        <p className="field-note theme-help">{t('themeSetting.brandColorHelp')}</p>
      </div>
      <div className="theme-row theme-toggle">
        <label>
          <input
            type="checkbox"
            checked={draft.derive}
            onChange={(event) => {
              onDerive(event.target.checked);
            }}
          />{' '}
          {t('themeSetting.derivePalette')}
        </label>
        <p className="field-note theme-help">{t('themeSetting.derivePaletteHelp')}</p>
      </div>
      {shownShades(resolved.scheme).map((shade) => {
        const palette = shade === 'dark' ? resolved.dark : resolved.light;
        return (
          <figure key={shade} className="theme-swatches">
            <figcaption>
              {t('theme.palette')}: {t(shadeKey(shade))}
            </figcaption>
            <ul>
              {ROLES.map((role) => (
                <li key={role} data-role={role}>
                  <span className="theme-swatch" style={{ background: roleColour(palette, role) }} />
                  <span className="theme-swatch-name">{t(roleLabelKey(role))}</span>
                  <code>{roleColour(palette, role)}</code>
                </li>
              ))}
            </ul>
          </figure>
        );
      })}
    </section>
  );
};

const formatRatio = (ratio: number): string => `${ratio.toFixed(2)}:1`;

/** The contrast readout: all passing in one line, or each failing pair with its fix. */
export const ContrastPanel = ({
  checks,
  onUse,
}: {
  readonly checks: readonly PairCheck[];
  readonly onUse: (role: Role, colour: string, shade: Shade) => void;
}): ReactElement => {
  const t = useThemeText();
  const failing = checks.filter((check) => !check.passes);
  const auto = new Set(checks.map((check) => check.shade)).size > 1;
  return (
    <section className="theme-contrast" aria-labelledby="theme-contrast">
      <h3 id="theme-contrast">{t('theme.contrast')}</h3>
      {failing.length === 0 ? (
        <p className="contrast-pass" role="status">
          {t('theme.contrastPass')}
        </p>
      ) : (
        <>
          <p className="field-error" role="alert">
            {t('theme.contrastFail', { count: failing.length })}
          </p>
          <ul>
            {failing.map((check) => {
              const role = t(roleLabelKey(check.foreground));
              return (
                <li
                  key={`${check.shade}-${check.foreground}-${check.background}`}
                  data-contrast={`${check.foreground}-${check.background}`}
                >
                  <span className="contrast-label">
                    {t('theme.contrastPair', {
                      foreground: role,
                      background: t(roleLabelKey(check.background)),
                    })}
                    {auto ? ` (${t(shadeKey(check.shade))})` : ''}
                  </span>
                  <span className="contrast-ratio">{formatRatio(check.ratio)}</span>
                  {check.suggestion === null ? null : (
                    <button
                      type="button"
                      aria-label={t('theme.contrastUseLabel', { colour: check.suggestion, role })}
                      onClick={() => {
                        if (check.suggestion !== null) {
                          onUse(check.foreground, check.suggestion, check.shade);
                        }
                      }}
                    >
                      <span className="theme-swatch" style={{ background: check.suggestion }} />
                      {t('theme.contrastUse', { colour: check.suggestion })}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}
    </section>
  );
};
