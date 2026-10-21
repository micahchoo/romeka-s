/**
 * What the overlay draws: every saved region with its list number, the open
 * draft, the rectangle under the pointer and a polygon as it is being clicked.
 *
 * The group is a picture of the list, not a second set of controls: it takes no
 * pointer event, and the overlay hit-tests the geometry in `draw.ts` itself, so
 * a shape never steals a drag from the tool. Sizes are in canvas units derived
 * from `unitPerPx`, so a number or a handle keeps its size on screen at any zoom.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { Point, Region } from './draw';
import { boundsOf, boxBetween } from './draw';
import type { PlacedNote } from './note-list';

/** One note's shape, in the order the list numbers it. */
export interface DrawnRegion {
  readonly row: PlacedNote;
  readonly geometry: Region;
}

/** A rectangle drag in progress: where it began on the canvas and on screen. */
export interface LiveBox {
  readonly start: Point;
  readonly at: Point;
  /** The pointer's first screen point, which decides a click from a drag. */
  readonly from: Point;
}

export interface RegionShapesProps {
  readonly regions: readonly DrawnRegion[];
  /** A note that is not saved yet: the shape is dashed and says so. */
  readonly draft: Region | null;
  readonly selected: string | null;
  readonly hidden: boolean;
  /** Past 200 regions every outline fades and only the selected one keeps its number. */
  readonly dense: boolean;
  readonly unitPerPx: number;
  readonly drag: LiveBox | null;
  readonly corners: readonly Point[];
  /** The keyboard's next polygon corner. */
  readonly cursor: Point;
}

const pointsText = (points: readonly Point[]): string =>
  points.map((point) => `${String(point.x)},${String(point.y)}`).join(' ');

const Geometry = ({
  region,
  className,
}: {
  readonly region: Region;
  readonly className?: string;
}): ReactElement =>
  region.kind === 'rect' ? (
    <rect className={className} x={region.x} y={region.y} width={region.w} height={region.h} />
  ) : (
    <polygon className={className} points={pointsText(region.points)} />
  );

/** The corners a shape shows when it is open: its box, or its own points. */
const handlesOf = (region: Region): readonly Point[] =>
  region.kind === 'rect'
    ? [
        { x: region.x, y: region.y },
        { x: region.x + region.w, y: region.y },
        { x: region.x + region.w, y: region.y + region.h },
        { x: region.x, y: region.y + region.h },
      ]
    : region.points;

const Label = ({
  region,
  unitPerPx,
  children,
}: {
  readonly region: Region;
  readonly unitPerPx: number;
  readonly children: string | number;
}): ReactElement => (
  <text
    className="region-number"
    x={boundsOf(region).x + unitPerPx * 12}
    y={boundsOf(region).y + unitPerPx * 12}
    fontSize={unitPerPx * 12}
    strokeWidth={unitPerPx * 3}
    dominantBaseline="hanging"
  >
    {children}
  </text>
);

export const RegionShapes = (props: RegionShapesProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { regions, draft, selected, hidden, dense, unitPerPx, drag, corners, cursor } = props;
  return (
    <g className={dense ? 'region-shapes is-dense' : 'region-shapes'}>
      {regions.map(({ row, geometry }) => {
        if (hidden && row.note.iri !== selected) {
          return null;
        }
        const chosen = row.note.iri === selected;
        return (
          <g
            key={row.note.iri}
            className={chosen ? 'is-selected' : undefined}
            data-iri={row.note.iri}
            data-number={row.number}
          >
            <Geometry region={geometry} className="region-shape" />
            {dense && !chosen ? null : (
              <Label region={geometry} unitPerPx={unitPerPx}>
                {row.number}
              </Label>
            )}
            {chosen ? (
              <g className="region-handles">
                {handlesOf(geometry).map((point) => (
                  <circle
                    key={`${String(point.x)},${String(point.y)}`}
                    cx={point.x}
                    cy={point.y}
                    r={Math.max(1, unitPerPx * 6)}
                  />
                ))}
              </g>
            ) : null}
          </g>
        );
      })}

      {draft === null ? null : (
        <g className="region-draft" data-draft="true">
          <Geometry region={draft} className="region-shape" />
          <Label region={draft} unitPerPx={unitPerPx}>
            {t('region.draft')}
          </Label>
        </g>
      )}

      {drag === null ? null : (
        <Geometry region={boxBetween(drag.start, drag.at)} className="region-shape is-live" />
      )}

      {corners.length === 0 ? null : (
        <g className="region-outline">
          <polyline points={pointsText(corners)} />
          {corners.map((corner) => (
            <circle
              key={`${String(corner.x)},${String(corner.y)}`}
              className="region-corner"
              cx={corner.x}
              cy={corner.y}
              r={Math.max(1, unitPerPx * 4)}
            />
          ))}
          <circle className="region-cursor" cx={cursor.x} cy={cursor.y} r={Math.max(1, unitPerPx * 5)} />
        </g>
      )}
    </g>
  );
};
