/**
 * A small Leaflet map for a form: it shows places as dots, reports a click as
 * a place, and reports where the reader panned and zoomed to. It loads Leaflet
 * the first time one mounts (`loadLeaflet`). When Leaflet cannot load, the
 * form's typed "lat, lng" input still works, and the map says so.
 */
import type { ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import { loadLeaflet } from './leaflet';
import type { Leaflet, LeafletLayer, LeafletMap } from './leaflet';
import { formatPlace, placeOf } from './place';
import type { Place } from './place';
import './place-map.css';

export interface PlaceMapProps {
  /** The map's accessible name, which says what a click does. */
  readonly label: string;
  readonly tiles: string;
  readonly attribution: string;
  /** Where the map opens; `null` opens on the first marker, or the world. */
  readonly start: { readonly centre: Place; readonly zoom: number } | null;
  readonly markers: readonly Place[];
  /** A place to bring into view when it changes, such as one just typed. */
  readonly focus?: Place | null;
  readonly onPick?: (place: Place) => void;
  readonly onView?: (centre: Place, zoom: number) => void;
  /** The line shown when Leaflet cannot load. */
  readonly unavailable: string;
}

const WORLD = { centre: { lat: 20, lng: 0 }, zoom: 1 } as const;
const CLOSE_ZOOM = 10;

export const PlaceMap = (props: PlaceMapProps): ReactElement => {
  const element = useRef<HTMLDivElement>(null);
  const leaflet = useRef<Leaflet | null>(null);
  const map = useRef<LeafletMap | null>(null);
  const dots = useRef<readonly LeafletLayer[]>([]);
  const latest = useRef(props);
  latest.current = props;
  const [failed, setFailed] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadLeaflet().then(
      (L) => {
        const host = element.current;
        if (cancelled || host === null) {
          return;
        }
        const initial = latest.current;
        const first = initial.markers[0];
        const start =
          initial.start ?? (first === undefined ? WORLD : { centre: first, zoom: CLOSE_ZOOM });
        const created = L.map(host, { scrollWheelZoom: false }).setView(
          [start.centre.lat, start.centre.lng],
          start.zoom,
        );
        L.tileLayer(initial.tiles, { maxZoom: 19 }).addTo(created);
        const credit = document.createElement('span');
        credit.textContent = initial.attribution;
        created.attributionControl.addAttribution(credit.outerHTML);
        created.on('click', (event) => {
          const place =
            event.latlng === undefined ? null : placeOf(event.latlng.lat, event.latlng.lng);
          if (place !== null) {
            latest.current.onPick?.(place);
          }
        });
        created.on('moveend', () => {
          const centre = created.getCenter();
          const place = placeOf(centre.lat, centre.lng);
          if (place !== null) {
            latest.current.onView?.(place, created.getZoom());
          }
        });
        leaflet.current = L;
        map.current = created;
        setReady(true);
      },
      () => {
        if (!cancelled) {
          setFailed(true);
        }
      },
    );
    return () => {
      cancelled = true;
      map.current?.remove();
      map.current = null;
    };
  }, []);

  // The markers as text, so a new array with the same places draws nothing.
  const markerKey = props.markers.map(formatPlace).join(';');
  useEffect(() => {
    const L = leaflet.current;
    const current = map.current;
    if (!ready || L === null || current === null) {
      return;
    }
    for (const dot of dots.current) {
      dot.remove();
    }
    dots.current = latest.current.markers.map((place) =>
      L.circleMarker([place.lat, place.lng], { radius: 7, weight: 2, fillOpacity: 0.8 }).addTo(
        current,
      ),
    );
  }, [ready, markerKey]);

  const focusLat = props.focus?.lat ?? null;
  const focusLng = props.focus?.lng ?? null;
  useEffect(() => {
    const current = map.current;
    if (!ready || current === null || focusLat === null || focusLng === null) {
      return;
    }
    current.setView([focusLat, focusLng], Math.max(current.getZoom(), CLOSE_ZOOM));
  }, [ready, focusLat, focusLng]);

  return (
    <div>
      <div ref={element} className="place-map" role="application" aria-label={props.label} />
      {failed ? (
        <p className="place-map-status" role="status">
          {props.unavailable}
        </p>
      ) : null}
    </div>
  );
};
