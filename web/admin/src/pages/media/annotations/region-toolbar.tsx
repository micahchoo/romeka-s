/**
 * The region editor's toolbar: the three tools as one segmented control, the
 * six zoom steps, the hide switch, and under Advanced the form that takes a
 * region as four numbers (the rectangle's keyboard path).
 *
 * A tool is a short-lived mode made visible here: the pressed button, the
 * crosshair on the overlay, the hint line under it. The hint always names the
 * gesture of the tool that is pressed, so nothing here has to be discovered.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { BoxFields, CanvasSize, Region } from './draw';
import { boxFromText, MAX_ZOOM, MIN_ZOOM } from './draw';

export type Tool = 'move' | 'rect' | 'polygon';

export interface RegionToolbarProps {
  readonly tool: Tool;
  readonly zoom: number;
  readonly hidden: boolean;
  readonly advanced: boolean;
  readonly size: CanvasSize;
  readonly onTool: (tool: Tool) => void;
  readonly onZoom: (direction: -1 | 1) => void;
  readonly onHidden: (hidden: boolean) => void;
  readonly onAdvanced: (open: boolean) => void;
  /** A region typed as numbers, already checked against the canvas. */
  readonly onAdd: (region: Region) => void;
}

const NO_NUMBERS: BoxFields = { x: '', y: '', width: '', height: '' };

/** The four numbers of a typed region, with the label that names each one. */
const NUMBER_FIELDS: readonly {
  readonly name: keyof BoxFields;
  readonly label:
    | 'region.byNumbers.x'
    | 'region.byNumbers.y'
    | 'region.byNumbers.width'
    | 'region.byNumbers.height';
}[] = [
  { name: 'x', label: 'region.byNumbers.x' },
  { name: 'y', label: 'region.byNumbers.y' },
  { name: 'width', label: 'region.byNumbers.width' },
  { name: 'height', label: 'region.byNumbers.height' },
];

const BoxForm = ({
  size,
  onAdd,
}: {
  readonly size: CanvasSize;
  readonly onAdd: (region: Region) => void;
}): ReactElement => {
  const { t } = useTranslator(annotations);
  const [fields, setFields] = useState<BoxFields>(NO_NUMBERS);
  const [refused, setRefused] = useState(false);

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const region = boxFromText(fields, size);
    if (region === null) {
      setRefused(true);
      return;
    }
    setRefused(false);
    setFields(NO_NUMBERS);
    onAdd(region);
  };

  return (
    <form className="region-by-numbers" onSubmit={submit}>
      <p className="region-advanced-title">{t('region.byNumbers.title')}</p>
      <p className="region-hint">
        {t('region.byNumbers.hint', { width: size.width, height: size.height })}
      </p>
      {NUMBER_FIELDS.map((field) => (
        <label key={field.name} className="region-number-field">
          <span>{t(field.label)}</span>
          <input
            type="number"
            min={0}
            step={1}
            value={fields[field.name]}
            onChange={(event) => {
              const value = event.target.value;
              setFields((current) => ({ ...current, [field.name]: value }));
            }}
          />
        </label>
      ))}
      <button type="submit">{t('region.byNumbers.add')}</button>
      {refused ? (
        <p className="region-refusal" role="alert">
          {t('region.byNumbers.invalid')}
        </p>
      ) : null}
    </form>
  );
};

/** The three drawing tools, as one segmented control: one is always pressed. */
const TOOLS: readonly { readonly tool: Tool; readonly label: 'region.tool.move' | 'region.tool.rect' | 'region.tool.polygon' }[] = [
  { tool: 'move', label: 'region.tool.move' },
  { tool: 'rect', label: 'region.tool.rect' },
  { tool: 'polygon', label: 'region.tool.polygon' },
];

export const RegionToolbar = (props: RegionToolbarProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { tool, zoom, hidden, advanced, size, onTool, onZoom, onHidden, onAdvanced, onAdd } = props;
  return (
    <>
      <div className="region-toolbar">
        <div className="tool-segments" role="group" aria-label={t('region.tools')}>
          {TOOLS.map((entry) => (
            <button
              key={entry.tool}
              type="button"
              aria-pressed={tool === entry.tool}
              onClick={() => {
                onTool(entry.tool);
              }}
            >
              {t(entry.label)}
            </button>
          ))}
        </div>
        {/* The zoom keeps its place at either end of the ladder: a control
            that vanishes moves every one after it. */}
        <div className="tool-segments region-zoom">
          <button
            type="button"
            aria-label={t('region.zoom.out')}
            title={t('region.zoom.out')}
            disabled={zoom <= MIN_ZOOM}
            onClick={() => {
              onZoom(-1);
            }}
          >
            −
          </button>
          <span className="region-readout">{t('region.zoom', { zoom })}</span>
          <button
            type="button"
            aria-label={t('region.zoom.in')}
            title={t('region.zoom.in')}
            disabled={zoom >= MAX_ZOOM}
            onClick={() => {
              onZoom(1);
            }}
          >
            +
          </button>
        </div>
        <button
          type="button"
          className="tool-plain"
          aria-pressed={hidden}
          onClick={() => {
            onHidden(!hidden);
          }}
        >
          {hidden ? t('region.show') : t('region.hide')}
        </button>
        <button
          type="button"
          className="tool-plain region-more"
          aria-expanded={advanced}
          onClick={() => {
            onAdvanced(!advanced);
          }}
        >
          {t('region.advanced')}
        </button>
      </div>

      {advanced ? (
        <div className="region-advanced">
          <BoxForm size={size} onAdd={onAdd} />
        </div>
      ) : null}
    </>
  );
};
