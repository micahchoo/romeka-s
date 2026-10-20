/**
 * The `mapView` field control: where a map block opens. "Fit every place"
 * stores no centre; "Start at this view" stores the centre as `"lat, lng"`
 * and the zoom, set by panning and zooming a small map or by typing them. The
 * small map draws the site's own basemap, read from its settings.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import { isRecord } from '../../../../api/parse';
import type { JsonValue, SiteId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { PlaceMap } from '../../../../map/PlaceMap';
import { DEFAULT_ATTRIBUTION, formatPlace, parsePlace, tileTemplate } from '../../../../map/place';
import { useEditorText } from '../messages';
import { Help, helpId } from '../Labelled';
import './controls.css';

/** The zoom a new set view starts at: the whole world. */
const WORLD_ZOOM = 2;
const MAX_ZOOM = 19;

const textOf = (value: JsonValue | undefined): string => (typeof value === 'string' ? value : '');

const zoomOf = (value: JsonValue | undefined): number | null =>
  typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= MAX_ZOOM
    ? value
    : null;

export interface MapViewFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly centreKey: string;
  readonly zoomKey: string;
  readonly record: Readonly<Record<string, JsonValue>>;
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
  readonly siteId: SiteId;
}

/** The site's basemap, OpenStreetMap's until its settings load. */
const useBasemap = (siteId: SiteId): { readonly tiles: string; readonly attribution: string } => {
  const site = useAsync(() => api.getSite(siteId), [siteId]);
  const settings =
    site.status === 'ready' && isRecord(site.data.siteSettings) ? site.data.siteSettings : {};
  const attribution = settings.romeka_map_attribution;
  return {
    tiles: tileTemplate(settings.romeka_map_tiles),
    attribution: typeof attribution === 'string' ? attribution : DEFAULT_ATTRIBUTION,
  };
};

export const MapViewField = ({
  id,
  label,
  help,
  centreKey,
  zoomKey,
  record,
  onPatch,
  siteId,
}: MapViewFieldProps): ReactElement => {
  const t = useEditorText();
  const basemap = useBasemap(siteId);
  const centreText = textOf(record[centreKey]);
  const centre = parsePlace(centreText);
  const zoom = zoomOf(record[zoomKey]);
  const set = centreText !== '';
  return (
    <fieldset className="block-field map-view-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      <div className="link-kinds">
        <label className="block-check">
          <input
            type="radio"
            name={`${id}-start`}
            checked={!set}
            onChange={() => {
              onPatch({ [centreKey]: '', [zoomKey]: null });
            }}
          />
          {t('mapView.fit')}
        </label>
        <label className="block-check">
          <input
            type="radio"
            name={`${id}-start`}
            checked={set}
            onChange={() => {
              onPatch({ [centreKey]: '20, 0', [zoomKey]: WORLD_ZOOM });
            }}
          />
          {t('mapView.set')}
        </label>
      </div>
      {set ? (
        <>
          <PlaceMap
            label={t('mapView.mapLabel')}
            tiles={basemap.tiles}
            attribution={basemap.attribution}
            start={centre === null ? null : { centre, zoom: zoom ?? WORLD_ZOOM }}
            markers={[]}
            onView={(next, nextZoom) => {
              const text = formatPlace(next);
              if (text !== centreText || nextZoom !== zoom) {
                onPatch({ [centreKey]: text, [zoomKey]: nextZoom });
              }
            }}
            unavailable={t('mapView.unavailable')}
          />
          <div className="map-view-typed">
            <div>
              <label htmlFor={id}>{t('mapView.centre')}</label>
              <input
                id={id}
                type="text"
                inputMode="decimal"
                value={centreText}
                aria-invalid={centre === null}
                onChange={(event) => {
                  onPatch({ [centreKey]: event.target.value });
                }}
              />
            </div>
            <div>
              <label htmlFor={`${id}-zoom`}>{t('mapView.zoom')}</label>
              <input
                id={`${id}-zoom`}
                type="number"
                min={0}
                max={MAX_ZOOM}
                value={zoom ?? ''}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  onPatch({
                    [zoomKey]:
                      event.target.value === ''
                        ? null
                        : Math.min(Math.max(Math.round(next), 0), MAX_ZOOM),
                  });
                }}
              />
            </div>
          </div>
          {centre === null ? (
            <p className="field-error" role="alert">
              {t('mapView.invalid')}
            </p>
          ) : null}
        </>
      ) : null}
      <Help id={id} text={help} />
    </fieldset>
  );
};
