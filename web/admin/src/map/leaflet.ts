/**
 * Leaflet for the admin's small maps, loaded on first use from the theme's
 * vendored copy (`/assets/vendor/leaflet/`), the same bytes the public site
 * serves. The admin bundle never carries it, and a form with no map never
 * fetches it.
 *
 * The script defines `window.L`, which arrives from outside the bundle, so
 * `isLeaflet` checks the members this file's types name before anything calls
 * them.
 */

export interface LatLng {
  readonly lat: number;
  readonly lng: number;
}

export interface LeafletEvent {
  readonly latlng?: LatLng;
}

export interface LeafletLayer {
  addTo: (map: LeafletMap) => LeafletLayer;
  remove: () => void;
}

export interface LeafletMap {
  setView: (centre: readonly [number, number], zoom: number) => LeafletMap;
  on: (event: string, handler: (event: LeafletEvent) => void) => LeafletMap;
  getCenter: () => LatLng;
  getZoom: () => number;
  remove: () => void;
  attributionControl: { addAttribution: (text: string) => unknown };
}

export interface Leaflet {
  map: (element: HTMLElement, options: Readonly<Record<string, unknown>>) => LeafletMap;
  tileLayer: (template: string, options: Readonly<Record<string, unknown>>) => LeafletLayer;
  circleMarker: (
    centre: readonly [number, number],
    options: Readonly<Record<string, unknown>>,
  ) => LeafletLayer;
}

const isLeaflet = (value: unknown): value is Leaflet =>
  typeof value === 'object' &&
  value !== null &&
  typeof Reflect.get(value, 'map') === 'function' &&
  typeof Reflect.get(value, 'tileLayer') === 'function' &&
  typeof Reflect.get(value, 'circleMarker') === 'function';

const BASE = '/assets/vendor/leaflet';

let loading: Promise<Leaflet> | null = null;

const append = (element: HTMLElement): void => {
  document.head.appendChild(element);
};

/** Leaflet, loading it the first time any map asks. A failed load can be retried. */
export const loadLeaflet = (): Promise<Leaflet> => {
  const present: unknown = Reflect.get(window, 'L');
  if (isLeaflet(present)) {
    return Promise.resolve(present);
  }
  loading ??= new Promise<Leaflet>((resolve, reject) => {
    const sheet = document.createElement('link');
    sheet.rel = 'stylesheet';
    sheet.href = `${BASE}/leaflet.css`;
    append(sheet);
    const script = document.createElement('script');
    script.src = `${BASE}/leaflet.js`;
    script.onload = () => {
      const loaded: unknown = Reflect.get(window, 'L');
      if (isLeaflet(loaded)) {
        resolve(loaded);
      } else {
        reject(new Error('Leaflet did not define L'));
      }
    };
    script.onerror = () => {
      loading = null;
      reject(new Error('Leaflet did not load'));
    };
    append(script);
  });
  return loading;
};
