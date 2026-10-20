/**
 * The item form's place control. A site names its "Location field" in its
 * settings (`romeka_location_property`), and its maps draw the `"lat, lng"`
 * values of that property. This control edits those values with a small map:
 * click the map, or type the coordinates.
 *
 * The values stay ordinary literal rows of the draft, so the values editor
 * shows the same rows and the write needs nothing new.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { Property, PropertyId, Site } from '../../api/types';
import { isRecord } from '../../api/parse';
import { draftKey, isBlank, patchValue, removeValue } from '../../app/draft';
import type { ResourceDraft, ValueDraft } from '../../app/draft';
import { useTranslator } from '../../i18n';
import { resources } from '../../i18n/namespaces/resources';
import { PlaceMap } from '../../map/PlaceMap';
import { DEFAULT_ATTRIBUTION, formatPlace, parsePlace, tileTemplate } from '../../map/place';
import type { Place } from '../../map/place';

/** One location field some site names, with the basemap that site draws. */
export interface LocationField {
  readonly term: string;
  readonly propertyId: PropertyId;
  readonly label: string;
  readonly tiles: string;
  readonly attribution: string;
}

const setting = (site: Site, key: string): unknown =>
  isRecord(site.siteSettings) ? site.siteSettings[key] : undefined;

/** Each property a site names as its location field, once, in site order. */
export const locationFields = (
  sites: readonly Site[],
  properties: readonly Property[],
): readonly LocationField[] => {
  const fields: LocationField[] = [];
  for (const site of sites) {
    const term = setting(site, 'romeka_location_property');
    const property = properties.find((entry) => entry.term === term);
    if (property === undefined || fields.some((field) => field.term === property.term)) {
      continue;
    }
    const attribution = setting(site, 'romeka_map_attribution');
    fields.push({
      term: property.term,
      propertyId: property.id,
      label: property.label ?? property.term,
      tiles: tileTemplate(setting(site, 'romeka_map_tiles')),
      attribution: typeof attribution === 'string' ? attribution : DEFAULT_ATTRIBUTION,
    });
  }
  return fields;
};

const isPlaceRow = (field: LocationField, value: ValueDraft): boolean =>
  value.term === field.term && value.kind === 'literal';

export const PlaceField = ({
  field,
  draft,
  onChange,
}: {
  readonly field: LocationField;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
}): ReactElement => {
  const { t } = useTranslator(resources);
  const rows = draft.values.filter((value) => isPlaceRow(field, value));
  const [chosen, setChosen] = useState<string | null>(null);
  const selected = rows.find((row) => row.key === chosen) ?? rows[rows.length - 1] ?? null;
  const places = rows.flatMap((row) => {
    const place = parsePlace(row.text);
    return place === null ? [] : [place];
  });

  const add = (text: string): void => {
    const row: ValueDraft = {
      key: draftKey(),
      propertyId: field.propertyId,
      term: field.term,
      type: 'literal',
      kind: 'literal',
      text,
      label: '',
      lang: '',
      isPublic: true,
    };
    setChosen(row.key);
    onChange({ ...draft, values: [...draft.values, row] });
  };

  const pick = (place: Place): void => {
    if (selected === null) {
      add(formatPlace(place));
      return;
    }
    onChange(patchValue(draft, selected.key, { text: formatPlace(place) }));
  };

  const id = `place-${field.term.replace(':', '-')}`;
  return (
    <fieldset className="place-field">
      <legend>{t('place.legend', { field: field.label })}</legend>
      <p className="field-note" id={`${id}-help`}>
        {t('place.help', { term: field.term })}
      </p>
      <PlaceMap
        label={t('place.mapLabel')}
        tiles={field.tiles}
        attribution={field.attribution}
        start={null}
        markers={places}
        focus={selected === null ? null : parsePlace(selected.text)}
        onPick={pick}
        unavailable={t('place.unavailable')}
      />
      {rows.map((row, index) => {
        const invalid = !isBlank(row) && parsePlace(row.text) === null;
        const inputId = `${id}-${String(index)}`;
        return (
          <div className="place-row" key={row.key}>
            <label htmlFor={inputId}>{t('place.value', { number: index + 1 })}</label>
            <input
              id={inputId}
              type="text"
              inputMode="decimal"
              value={row.text}
              placeholder="51.5072, -0.1276"
              aria-invalid={invalid}
              aria-describedby={`${id}-help`}
              onFocus={() => {
                setChosen(row.key);
              }}
              onChange={(event) => {
                onChange(patchValue(draft, row.key, { text: event.target.value }));
              }}
              onBlur={() => {
                const place = parsePlace(row.text);
                if (place !== null && formatPlace(place) !== row.text) {
                  onChange(patchValue(draft, row.key, { text: formatPlace(place) }));
                }
              }}
            />
            <button
              type="button"
              onClick={() => {
                onChange(removeValue(draft, row.key));
              }}
            >
              {t('place.remove', { number: index + 1 })}
            </button>
            {invalid ? (
              <p className="field-error" role="alert">
                {t('place.invalid')}
              </p>
            ) : null}
          </div>
        );
      })}
      <button
        type="button"
        onClick={() => {
          add('');
        }}
      >
        {t('place.add')}
      </button>
    </fieldset>
  );
};
