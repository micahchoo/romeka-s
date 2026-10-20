/**
 * A stand-in for the vendored Leaflet: `window.L` with the members the admin's
 * small maps call. A test fires the map's own events (`click`, `moveend`) the
 * way a reader's pointer would, and reads the dots the map drew.
 */
import { vi } from 'vitest';

interface FakeMap {
  readonly handlers: Map<string, (event: { latlng?: { lat: number; lng: number } }) => void>;
  centre: { lat: number; lng: number };
  zoom: number;
  readonly dots: { lat: number; lng: number }[];
}

export interface LeafletStub {
  /** The last map a component created. */
  readonly map: () => FakeMap;
  /** A reader's click at a place. */
  readonly click: (lat: number, lng: number) => void;
  /** A reader's pan and zoom, which ends in `moveend`. */
  readonly pan: (lat: number, lng: number, zoom: number) => void;
}

export const installLeaflet = (): LeafletStub => {
  const maps: FakeMap[] = [];
  const layer = (onAdd: (map: FakeMap) => void, onRemove: () => void) => ({
    addTo(target: { fake: FakeMap }) {
      onAdd(target.fake);
      return this;
    },
    remove: onRemove,
  });
  const leaflet = {
    map: () => {
      const fake: FakeMap = { handlers: new Map(), centre: { lat: 0, lng: 0 }, zoom: 0, dots: [] };
      maps.push(fake);
      const api = {
        fake,
        setView(centre: readonly [number, number], zoom: number) {
          fake.centre = { lat: centre[0], lng: centre[1] };
          fake.zoom = zoom;
          return api;
        },
        on(event: string, handler: (event: { latlng?: { lat: number; lng: number } }) => void) {
          fake.handlers.set(event, handler);
          return api;
        },
        getCenter: () => fake.centre,
        getZoom: () => fake.zoom,
        remove: vi.fn(),
        attributionControl: { addAttribution: vi.fn() },
      };
      return api;
    },
    tileLayer: () =>
      layer(
        () => undefined,
        () => undefined,
      ),
    circleMarker: (centre: readonly [number, number]) => {
      const dot = { lat: centre[0], lng: centre[1] };
      let owner: FakeMap | null = null;
      return layer(
        (map) => {
          owner = map;
          map.dots.push(dot);
        },
        () => {
          owner?.dots.splice(owner.dots.indexOf(dot), 1);
        },
      );
    },
  };
  vi.stubGlobal('L', leaflet);
  const map = (): FakeMap => {
    const last = maps[maps.length - 1];
    if (last === undefined) {
      throw new Error('no map was created');
    }
    return last;
  };
  return {
    map,
    click: (lat, lng) => {
      map().handlers.get('click')?.({ latlng: { lat, lng } });
    },
    pan: (lat, lng, zoom) => {
      const fake = map();
      fake.centre = { lat, lng };
      fake.zoom = zoom;
      fake.handlers.get('moveend')?.({});
    },
  };
};
