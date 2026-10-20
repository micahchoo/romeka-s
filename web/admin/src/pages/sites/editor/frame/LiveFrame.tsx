/**
 * The live preview surface: two script-free frames and the overlay the parent
 * draws over them.
 *
 * A new render goes into the back frame. On its `load` the shell reads the
 * front frame's scroll anchor, parses the back document (`readFrame`), restores
 * the anchor in it, guards it and swaps, so the user never sees a blank
 * document and the scroll position is named by a block, not by pixels.
 *
 * The overlay is a sibling of the frames, `pointer-events: none` except on the
 * insert chips, and `aria-hidden`: the Outline already offers every action it
 * does to the keyboard and to screen readers.
 */
import type { CSSProperties, ReactElement } from 'react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../../i18n';
import type { TemplateChoice } from '../../templates';
import { BlockPicker } from '../canvas/BlockPicker';
import { layoutLabel } from '../canvas/summary';
import type { BlockKey, PageDraft } from '../draft';
import { fitFrame, frameStyle, useBoxSize } from '../frameFit';
import type { DeviceWidth } from '../frameFit';
import { isGroup, parseNodes } from '../groups';
import { counted } from '../messages';
import type { PreviewRender } from '../preview';
import { REGION_LABEL } from '../regions';
import type { SiteRegion } from '../regions';
import { sameSubject, selectedKey } from '../shell/selection';
import type { Selection, Subject } from '../shell/selection';
import { restoreAnchor, takeAnchor } from './anchor';
import { boxOf, insertSlots } from './geometry';
import type { Box, InsertSlot, InsertTarget } from './geometry';
import { guardFrame } from './guard';
import { readFrame } from './markers';
import type { FrameMap } from './markers';
import { sites } from '../../../../i18n/namespaces/sites';
import { EDITOR_NAMESPACES } from '../messages';
import './frame.css';

export type PickerChoice =
  | { readonly kind: 'layout'; readonly layout: string }
  | { readonly kind: 'template'; readonly template: TemplateChoice };

export interface LiveFrameProps {
  readonly render: PreviewRender;
  readonly device: DeviceWidth;
  readonly draft: PageDraft;
  readonly selection: Selection | null;
  readonly templates: readonly TemplateChoice[];
  /** `null` for a click on the page outside every block and region. */
  readonly onSelect: (target: Subject | null) => void;
  readonly onInsert: (target: InsertTarget, choice: PickerChoice) => void;
}

type Buffer = 0 | 1;

interface Outline {
  readonly box: Box;
  readonly label: string;
  /** A site region, drawn so it reads as site-wide rather than as a block. */
  readonly region: boolean;
}

interface Overlay {
  readonly hover: Outline | null;
  readonly selected: Outline | null;
  readonly slots: readonly InsertSlot[];
}

const EMPTY: Overlay = { hover: null, selected: null, slots: [] };

const groupKeys = (draft: PageDraft): ReadonlySet<BlockKey> =>
  new Set(draft.blocks.filter(isGroup).map((block) => block.key));

const px = (box: Box): CSSProperties => ({
  top: `${String(box.top)}px`,
  left: `${String(box.left)}px`,
  width: `${String(box.width)}px`,
  height: `${String(box.height)}px`,
});

/** The label rides above the outline unless the block starts too close to the
 * top of the frame for it to show there. */
const LABEL_HEIGHT = 20;

const outlineClass = (kind: 'hover' | 'selected', outline: Outline): string =>
  [
    'live-outline',
    `live-outline--${kind}`,
    outline.box.top < LABEL_HEIGHT ? 'live-outline--top' : '',
    outline.region ? 'live-outline--region' : '',
  ].join(' ');

/** The box around every element of a region: the banner and the header together. */
const unionBox = (boxes: readonly Box[]): Box | null => {
  if (boxes.length === 0) {
    return null;
  }
  const top = Math.min(...boxes.map((box) => box.top));
  const left = Math.min(...boxes.map((box) => box.left));
  const bottom = Math.max(...boxes.map((box) => box.top + box.height));
  const right = Math.max(...boxes.map((box) => box.left + box.width));
  return { top, left, width: right - left, height: bottom - top };
};

const slotKey = (target: InsertTarget): string =>
  target.kind === 'top' ? `top-${String(target.index)}` : `group-${target.group}`;

export const LiveFrame = ({
  render,
  device,
  draft,
  selection,
  templates,
  onSelect,
  onInsert,
}: LiveFrameProps): ReactElement => {
  const { t } = useTranslator(sites, ...EDITOR_NAMESPACES);
  const [buffers, setBuffers] = useState<readonly [PreviewRender | null, PreviewRender | null]>([
    null,
    null,
  ]);
  const [front, setFront] = useState<Buffer | null>(null);
  const [map, setMap] = useState<FrameMap | null>(null);
  const [hovered, setHovered] = useState<Subject | null>(null);
  const [picker, setPicker] = useState<InsertTarget | null>(null);
  const [tick, setTick] = useState(0);
  const [overlay, setOverlay] = useState<Overlay>(EMPTY);
  const frames = useRef<[HTMLIFrameElement | null, HTMLIFrameElement | null]>([null, null]);
  const detach = useRef<(() => void) | null>(null);
  const frame = useRef<number | null>(null);
  const holder = useRef<HTMLDivElement | null>(null);
  const box = useBoxSize(holder);
  const latest = useRef({ buffers, front, map, selection, draft, onSelect });
  latest.current = { buffers, front, map, selection, draft, onSelect };

  // The new render goes into whichever frame is not on screen.
  useEffect(() => {
    setBuffers((current) => {
      if (current[0] === render || current[1] === render) {
        return current;
      }
      return front === 0 ? [current[0], render] : [render, current[1]];
    });
  }, [render, front]);

  useEffect(
    () => () => {
      detach.current?.();
      if (frame.current !== null) {
        cancelAnimationFrame(frame.current);
      }
    },
    [],
  );

  const remeasure = (): void => {
    if (frame.current === null) {
      frame.current = requestAnimationFrame(() => {
        frame.current = null;
        setTick((count) => count + 1);
      });
    }
  };

  const onLoad = (index: Buffer): void => {
    const shown = latest.current;
    const incoming = shown.buffers[index];
    const element = frames.current[index];
    const doc = element?.contentDocument;
    const win = element?.contentWindow;
    // The first `load` of an empty frame is `about:blank`, not a render.
    if (index === shown.front || incoming === null || !doc || !win || doc.URL !== 'about:srcdoc') {
      return;
    }
    const next = readFrame(doc, incoming.order, groupKeys(shown.draft));
    const shownWin = shown.front === null ? null : frames.current[shown.front]?.contentWindow;
    if (shownWin && shown.map !== null) {
      restoreAnchor(win, next, takeAnchor(shownWin, shown.map, selectedKey(shown.selection)));
    }
    detach.current?.();
    const unguard = guardFrame(doc, {
      onPoint: (node) => {
        setHovered(node === null ? null : next.targetAt(node));
      },
      onPick: (node) => {
        latest.current.onSelect(next.targetAt(node));
      },
    });
    win.addEventListener('scroll', remeasure, { passive: true });
    detach.current = () => {
      unguard();
      win.removeEventListener('scroll', remeasure);
    };
    setMap(next);
    setFront(index);
  };

  // A selection made in the Outline or by an issue scrolls its block into view.
  useEffect(() => {
    if (selection?.kind !== 'block' || selection.from === 'preview') {
      return;
    }
    const shown = latest.current;
    const win = shown.front === null ? null : frames.current[shown.front]?.contentWindow;
    const element = shown.map?.elements.get(selection.key);
    if (!win || element === undefined) {
      return;
    }
    const rect = element.getBoundingClientRect();
    if (rect.top >= 0 && rect.bottom <= win.innerHeight) {
      return;
    }
    const top =
      rect.top < 0 || rect.height > win.innerHeight
        ? win.scrollY + rect.top
        : win.scrollY + rect.bottom - win.innerHeight;
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    win.scrollTo({ top: Math.max(0, top), behavior: reduce ? 'auto' : 'smooth' });
  }, [selection]);

  const fit = fitFrame(device, box);

  useLayoutEffect(() => {
    const shownFrame = front === null ? null : frames.current[front];
    if (shownFrame === null || map === null) {
      setOverlay(EMPTY);
      return;
    }
    const regionOutline = (region: SiteRegion): Outline | null => {
      const box = unionBox((map.regions.get(region) ?? []).map((element) => boxOf(element, shownFrame)));
      return box === null ? null : { box, label: t(REGION_LABEL[region]), region: true };
    };
    const blockOutline = (key: BlockKey): Outline | null => {
      const element = map.elements.get(key);
      const node = parseNodes(draft.blocks).find(
        (candidate) => (candidate.kind === 'block' ? candidate.block.key : candidate.marker.key) === key,
      );
      const block = draft.blocks.find((candidate) => candidate.key === key);
      if (element === undefined || block === undefined) {
        return null;
      }
      const label =
        node?.kind === 'group'
          ? counted(t, 'groupBlocks', node.members.length)
          : layoutLabel(draft.registry, block, t);
      return { box: boxOf(element, shownFrame), label, region: false };
    };
    const outline = (subject: Subject | null): Outline | null => {
      if (subject === null) {
        return null;
      }
      return subject.kind === 'block' ? blockOutline(subject.key) : regionOutline(subject.region);
    };
    setOverlay({
      hover: sameSubject(hovered, selection) ? null : outline(hovered),
      selected: outline(selection),
      slots: insertSlots(draft, map, shownFrame),
    });
  }, [front, map, hovered, selection, draft, tick, box, device, t]);

  const choose = (target: InsertTarget, choice: PickerChoice): void => {
    setPicker(null);
    onInsert(target, choice);
  };

  return (
    <div className="editor-preview-frame live-frame" ref={holder}>
      {front === null ? <p className="editor-preview-empty">{t('editor.previewLoading')}</p> : null}
      {([0, 1] as const).map((index) => (
        <iframe
          key={index}
          ref={(element) => {
            frames.current[index] = element;
          }}
          className={index === front ? 'live-frame-doc' : 'live-frame-doc live-frame-doc--back'}
          sandbox="allow-same-origin"
          srcDoc={buffers[index]?.html}
          title={t('editor.previewFrame')}
          aria-hidden={index === front ? undefined : true}
          tabIndex={index === front ? undefined : -1}
          style={frameStyle(fit)}
          onLoad={() => {
            onLoad(index);
          }}
        />
      ))}
      <div
        className={
          overlay.slots.some((slot) => slot.shape === 'chip')
            ? 'live-overlay live-overlay--chips'
            : 'live-overlay'
        }
        aria-hidden="true"
      >
        {overlay.selected === null ? null : (
          <div className={outlineClass('selected', overlay.selected)} style={px(overlay.selected.box)}>
            <span className="live-label">{overlay.selected.label}</span>
          </div>
        )}
        {overlay.hover === null ? null : (
          <div className={outlineClass('hover', overlay.hover)} style={px(overlay.hover.box)}>
            <span className="live-label">{overlay.hover.label}</span>
          </div>
        )}
        {overlay.slots.map((slot) => (
          <div
            key={slotKey(slot.target)}
            className={`live-slot live-slot--${slot.shape}${slot.target.kind === 'group' ? ' live-slot--group' : ''}`}
            style={px(slot.box)}
          >
            <button
              type="button"
              tabIndex={-1}
              className="live-slot-button"
              data-insert-index={slot.target.index}
              data-insert-group={slot.target.kind === 'group' ? slot.target.group : undefined}
              title={t(slot.target.kind === 'group' ? 'preview.insertIntoGroup' : 'preview.insertHere')}
              onClick={() => {
                setPicker(slot.target);
              }}
            >
              +
            </button>
          </div>
        ))}
      </div>
      {picker === null ? null : (
        <BlockPicker
          registry={draft.registry}
          templates={templates}
          index={picker.index}
          onChoose={(layout) => {
            choose(picker, { kind: 'layout', layout });
          }}
          onChooseTemplate={(template) => {
            choose(picker, { kind: 'template', template });
          }}
          onClose={() => {
            setPicker(null);
          }}
        />
      )}
    </div>
  );
};
