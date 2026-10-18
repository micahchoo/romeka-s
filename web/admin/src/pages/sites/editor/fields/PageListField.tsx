/**
 * The `pageList` field control: the `listOfPages` page tree, stored as one JSON
 * string (`{text, data, children}`). Upstream's admin writes it with jsTree; the
 * port edits it as nested selects over the site's pages, one node per link.
 *
 * A stored `data.url` is kept, because the renderer prefers it over the page
 * lookup (`ListOfPages::node_url`). A tree that is not valid JSON renders a
 * notice and a reset instead of a silent rewrite.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import { isRecord, isUnknownArray } from '../../../../api/parse';
import type { JsonValue, SiteId, SitePage } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';

/** One stored tree node. */
interface PageListNode {
  readonly id: string;
  readonly text: string;
  readonly url: string;
  readonly children: readonly PageListNode[];
}

const nodeOf = (value: unknown): PageListNode | null => {
  if (!isRecord(value)) {
    return null;
  }
  const data = isRecord(value.data) ? value.data : {};
  const id = typeof data.id === 'number' || typeof data.id === 'string' ? String(data.id) : '';
  const url = typeof data.url === 'string' ? data.url : '';
  const text = typeof value.text === 'string' ? value.text : '';
  const children = isUnknownArray(value.children)
    ? value.children.flatMap((child) => {
        const node = nodeOf(child);
        return node === null ? [] : [node];
      })
    : [];
  return { id, text, url, children };
};

/** The stored string as nodes. `null` when it is not a JSON array. */
export const parsePageList = (text: string): readonly PageListNode[] | null => {
  if (text.trim() === '') {
    return [];
  }
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isUnknownArray(raw)) {
    return null;
  }
  return raw.flatMap((entry) => {
    const node = nodeOf(entry);
    return node === null ? [] : [node];
  });
};

const writeNode = (node: PageListNode): JsonValue => ({
  text: node.text,
  data: {
    id: node.id === '' ? '' : Number(node.id),
    ...(node.url === '' ? {} : { url: node.url }),
  },
  children: node.children.map(writeNode),
});

const formatPageList = (nodes: readonly PageListNode[]): string =>
  JSON.stringify(nodes.map(writeNode));

const emptyNode = (): PageListNode => ({ id: '', text: '', url: '', children: [] });

/** Replaces the node at `path`, or returns the tree unchanged when it is stale. */
const mapAt = (
  nodes: readonly PageListNode[],
  path: readonly number[],
  transform: (node: PageListNode) => PageListNode,
): readonly PageListNode[] => {
  const head = path[0];
  if (head === undefined) {
    return nodes;
  }
  const rest = path.slice(1);
  return nodes.map((node, index) =>
    index !== head
      ? node
      : rest.length === 0
        ? transform(node)
        : { ...node, children: mapAt(node.children, rest, transform) },
  );
};

const removeAt = (nodes: readonly PageListNode[], path: readonly number[]): readonly PageListNode[] => {
  const head = path[0];
  if (head === undefined) {
    return nodes;
  }
  const rest = path.slice(1);
  return rest.length === 0
    ? nodes.filter((_, index) => index !== head)
    : nodes.map((node, index) =>
        index === head ? { ...node, children: removeAt(node.children, rest) } : node,
      );
};

/** Moves one node among its siblings, or returns the tree unchanged at an edge. */
const moveAt = (
  nodes: readonly PageListNode[],
  path: readonly number[],
  delta: number,
): readonly PageListNode[] => {
  const head = path[0];
  const rest = path.slice(1);
  if (head === undefined) {
    return nodes;
  }
  if (rest.length > 0) {
    return nodes.map((node, index) =>
      index === head ? { ...node, children: moveAt(node.children, rest, delta) } : node,
    );
  }
  const target = head + delta;
  const moved = nodes[head];
  if (moved === undefined || target < 0 || target >= nodes.length) {
    return nodes;
  }
  const next = [...nodes];
  next.splice(head, 1);
  next.splice(target, 0, moved);
  return next;
};

interface PageListNodeProps {
  readonly id: string;
  readonly node: PageListNode;
  /** The tree root every edit rewrites, so a nested node can reach it. */
  readonly tree: readonly PageListNode[];
  readonly path: readonly number[];
  readonly index: number;
  readonly count: number;
  readonly pages: readonly SitePage[];
  readonly onChange: (nodes: readonly PageListNode[]) => void;
}

function PageListNode({
  id,
  node,
  tree,
  path,
  index,
  count,
  pages,
  onChange,
}: PageListNodeProps): ReactElement {
  const { t } = useTranslator();
  const key = path.join('.');
  return (
    <li>
      <p>
        <label htmlFor={`${id}-${key}`}>{t('pageList.page')}</label>
        <select
          id={`${id}-${key}`}
          value={node.id}
          onChange={(event) => {
            const chosen = pages.find((page) => String(page.id) === event.target.value);
            onChange(
              mapAt(tree, path, (entry) => ({
                ...entry,
                id: event.target.value,
                text: chosen === undefined ? entry.text : chosen.title,
              })),
            );
          }}
        >
          <option value="">{t('common.none')}</option>
          {node.id !== '' && !pages.some((page) => String(page.id) === node.id) ? (
            <option value={node.id}>{t('common.pageFallback', { id: node.id })}</option>
          ) : null}
          {pages.map((page) => (
            <option key={String(page.id)} value={String(page.id)}>
              {page.title === '' ? t('common.untitled') : page.title}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => {
            onChange(
              mapAt(tree, path, (entry) => ({ ...entry, children: [...entry.children, emptyNode()] })),
            );
          }}
        >
          {t('pageList.addChild')}
        </button>
        <button
          type="button"
          disabled={index === 0}
          aria-label={t('pageList.moveUp', { position: index + 1 })}
          onClick={() => {
            onChange(moveAt(tree, path, -1));
          }}
        >
          {t('blocks.moveUpText')}
        </button>
        <button
          type="button"
          disabled={index === count - 1}
          aria-label={t('pageList.moveDown', { position: index + 1 })}
          onClick={() => {
            onChange(moveAt(tree, path, 1));
          }}
        >
          {t('blocks.moveDownText')}
        </button>
        <button
          type="button"
          aria-label={t('pageList.remove', { position: index + 1 })}
          onClick={() => {
            onChange(removeAt(tree, path));
          }}
        >
          {t('action.remove')}
        </button>
      </p>
      {node.children.length === 0 ? null : (
        <ul className="page-list">
          {node.children.map((child, childIndex) => (
            <PageListNode
              key={[...path, childIndex].join('.')}
              id={id}
              node={child}
              tree={tree}
              path={[...path, childIndex]}
              index={childIndex}
              count={node.children.length}
              pages={pages}
              onChange={onChange}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export interface PageListFieldProps {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly siteId: SiteId;
}

export const PageListField = ({
  id,
  label,
  value,
  onChange,
  siteId,
}: PageListFieldProps): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.listSitePages({ siteId, perPage: 200 }), [siteId]);
  const pages = state.status === 'ready' ? state.data.items : [];
  const nodes = parsePageList(value);
  const change = (next: readonly PageListNode[]): void => {
    onChange(formatPageList(next));
  };

  if (nodes === null) {
    return (
      <fieldset className="block-field page-list-field">
        <legend>{label}</legend>
        <p className="field-error" role="alert">
          {t('pageList.invalid')}
        </p>
        <p>
          <button
            type="button"
            onClick={() => {
              change([]);
            }}
          >
            {t('pageList.reset')}
          </button>
        </p>
      </fieldset>
    );
  }
  return (
    <fieldset className="block-field page-list-field">
      <legend>{label}</legend>
      <ul className="page-list">
        {nodes.map((node, index) => (
          <PageListNode
            key={String(index)}
            id={id}
            node={node}
            tree={nodes}
            path={[index]}
            index={index}
            count={nodes.length}
            pages={pages}
            onChange={change}
          />
        ))}
      </ul>
      <p>
        <button
          type="button"
          onClick={() => {
            change([...nodes, emptyNode()]);
          }}
        >
          {t('pageList.add')}
        </button>
      </p>
    </fieldset>
  );
};
