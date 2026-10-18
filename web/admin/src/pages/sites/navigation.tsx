/**
 * The navigation editor (`SiteAdmin/IndexController::navigationAction`).
 *
 * `site.navigation` is a nested list of `{type, data, links}` nodes; the port
 * parses and resolves it in `crates/omeka-site/src/nav.rs`. The editor models
 * the four registered link types (`page`, `url`, `browse`, `browseItemSets`)
 * plus the fallback for a type the port does not know, whose `data` it keeps
 * verbatim. It writes the tree back with a partial `PATCH` of `o:navigation`,
 * so the rest of the site document is untouched.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { pageIdFromParam } from '../../api/parse';
import { encodeSiteNavigation } from '../../api/write';
import type { JsonValue, PageId, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { CheckboxField, OptionSelect, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';
import type { PageSummary } from './editor';

/** The link types the port models (`src/Site/Navigation/Link/*`). */
export const NAVIGATION_TYPES = ['page', 'url', 'browse', 'browseItemSets'] as const;

export type NavigationType = (typeof NAVIGATION_TYPES)[number];

const isNavigationType = (value: string): value is NavigationType =>
  NAVIGATION_TYPES.some((type) => type === value);

const TYPE_LABELS: Readonly<Record<NavigationType, MessageKey>> = {
  page: 'navigation.typePage',
  url: 'navigation.typeUrl',
  browse: 'navigation.typeBrowse',
  browseItemSets: 'navigation.typeBrowseItemSets',
};

/** One node of the tree while it is being edited. */
export interface NavigationDraft {
  readonly key: number;
  readonly type: string;
  readonly label: string;
  readonly pageId: PageId | null;
  readonly url: string;
  readonly targetBlank: boolean;
  readonly query: string;
  readonly links: readonly NavigationDraft[];
  /** The stored `data` of a link type the editor does not model. */
  readonly rawData: JsonValue | null;
}

let nextKey = 0;

const draft = (partial: Partial<NavigationDraft> & { readonly type: string }): NavigationDraft => {
  nextKey += 1;
  return {
    key: nextKey,
    label: '',
    pageId: null,
    url: '',
    targetBlank: false,
    query: '',
    links: [],
    rawData: null,
    ...partial,
  };
};

const text = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

const isJsonObject = (value: JsonValue): value is { readonly [key: string]: JsonValue } =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const isJsonArray = (value: JsonValue): value is readonly JsonValue[] => Array.isArray(value);

const pageId = (value: JsonValue | undefined): PageId | null => {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value > 0) {
    return pageIdFromParam(String(value));
  }
  if (typeof value === 'string') {
    return pageIdFromParam(value.trim());
  }
  return null;
};

const draftFromJson = (node: JsonValue): NavigationDraft => {
  if (!isJsonObject(node)) {
    return draft({ type: 'browse' });
  }
  const type = typeof node.type === 'string' ? node.type : 'browse';
  const data = node.data !== undefined && isJsonObject(node.data) ? node.data : {};
  const links =
    node.links !== undefined && isJsonArray(node.links) ? node.links.map(draftFromJson) : [];
  const parsed = draft({
    type,
    label: text(data.label),
    pageId: pageId(data.id),
    url: text(data.url),
    targetBlank: data.target_blank === true,
    query: text(data.query),
    links,
    rawData: isNavigationType(type) ? null : (node.data ?? null),
  });
  return parsed;
};

/** The stored tree as editable drafts. */
export const navigationDrafts = (value: readonly JsonValue[]): readonly NavigationDraft[] =>
  value.map(draftFromJson);

const emptyToNull = (value: string): string | null => {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
};

const jsonFromDraft = (node: NavigationDraft): JsonValue => {
  const links = node.links.map(jsonFromDraft);
  if (!isNavigationType(node.type) || node.rawData !== null) {
    return { type: node.type, data: node.rawData ?? {}, links };
  }
  switch (node.type) {
    case 'page':
      return {
        type: 'page',
        data: { id: node.pageId, label: emptyToNull(node.label) },
        links,
      };
    case 'url':
      return {
        type: 'url',
        data: {
          label: node.label.trim(),
          url: node.url.trim(),
          target_blank: node.targetBlank,
        },
        links,
      };
    case 'browse':
    case 'browseItemSets':
      return {
        type: node.type,
        data: { label: emptyToNull(node.label), query: node.query.trim() },
        links,
      };
  }
};

/** The tree as the `o:navigation` member stores it. */
export const navigationJson = (drafts: readonly NavigationDraft[]): readonly JsonValue[] =>
  drafts.map(jsonFromDraft);

const mapNodes = (
  drafts: readonly NavigationDraft[],
  key: number,
  map: (node: NavigationDraft) => NavigationDraft,
): readonly NavigationDraft[] =>
  drafts.map((node) =>
    node.key === key
      ? map(node)
      : { ...node, links: mapNodes(node.links, key, map) },
  );

export const addNavigationLink = (
  drafts: readonly NavigationDraft[],
  key: number | null,
  type: string = 'browse',
): readonly NavigationDraft[] => {
  const child = draft({ type });
  if (key === null) {
    return [...drafts, child];
  }
  return mapNodes(drafts, key, (node) => ({ ...node, links: [...node.links, child] }));
};

const removeFrom = (
  drafts: readonly NavigationDraft[],
  key: number,
): readonly NavigationDraft[] =>
  drafts
    .filter((node) => node.key !== key)
    .map((node) => ({ ...node, links: removeFrom(node.links, key) }));

export const removeNavigationLink = removeFrom;

const moveWithin = (
  drafts: readonly NavigationDraft[],
  key: number,
  delta: number,
): readonly NavigationDraft[] => {
  const index = drafts.findIndex((node) => node.key === key);
  if (index !== -1) {
    const target = index + delta;
    if (target < 0 || target >= drafts.length) {
      return drafts;
    }
    const reordered = [...drafts];
    const moved = reordered[index];
    const swap = reordered[target];
    if (moved === undefined || swap === undefined) {
      return drafts;
    }
    reordered[index] = swap;
    reordered[target] = moved;
    return reordered;
  }
  return drafts.map((node) => ({ ...node, links: moveWithin(node.links, key, delta) }));
};

export const moveNavigationLink = moveWithin;

export const patchNavigationLink = (
  drafts: readonly NavigationDraft[],
  key: number,
  patch: Partial<NavigationDraft>,
): readonly NavigationDraft[] => mapNodes(drafts, key, (node) => ({ ...node, ...patch }));

const NavigationNode = ({
  node,
  pages,
  depth,
  onChange,
  onRemove,
  onMove,
  onAddChild,
}: {
  readonly node: NavigationDraft;
  readonly pages: readonly PageSummary[];
  readonly depth: number;
  readonly onChange: (key: number, patch: Partial<NavigationDraft>) => void;
  readonly onRemove: (key: number) => void;
  readonly onMove: (key: number, delta: number) => void;
  readonly onAddChild: (key: number) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const label = node.label === '' ? `#${String(depth + 1)}` : node.label;
  const id = (suffix: string): string => `nav-${String(node.key)}-${suffix}`;
  return (
    <li className="navigation-node">
      <p>
        <button
          type="button"
          aria-label={t('navigation.moveUp', { label })}
          onClick={() => {
            onMove(node.key, -1);
          }}
        >
          {t('navigation.moveUpShort')}
        </button>
        <button
          type="button"
          aria-label={t('navigation.moveDown', { label })}
          onClick={() => {
            onMove(node.key, 1);
          }}
        >
          {t('navigation.moveDownShort')}
        </button>
        <button
          type="button"
          aria-label={t('navigation.remove', { label })}
          onClick={() => {
            onRemove(node.key);
          }}
        >
          {t('action.remove')}
        </button>
      </p>
      <p>
        <label htmlFor={id('type')}>{t('navigation.type')}</label>
        <select
          id={id('type')}
          value={node.type}
          onChange={(event) => {
            onChange(node.key, { type: event.target.value });
          }}
        >
          {isNavigationType(node.type) ? null : (
            <option value={node.type}>{node.type}</option>
          )}
          {NAVIGATION_TYPES.map((type) => (
            <option key={type} value={type}>
              {t(TYPE_LABELS[type])}
            </option>
          ))}
        </select>
      </p>
      {node.type === 'page' ? (
        <OptionSelect
          id={id('page')}
          label={t('navigation.page')}
          options={pages.map((page) => ({
            id: page.id,
            label: page.title ?? t('common.pageFallback', { id: page.id }),
          }))}
          value={node.pageId}
          onChange={(value) => {
            onChange(node.key, { pageId: value });
          }}
          emptyLabel={t('common.none')}
        />
      ) : null}
      {node.type === 'url' ? (
        <TextField
          id={id('url')}
          label={t('navigation.url')}
          value={node.url}
          onChange={(value) => {
            onChange(node.key, { url: value });
          }}
        />
      ) : null}
      {node.type === 'url' ? (
        <CheckboxField
          id={id('target-blank')}
          label={t('navigation.targetBlank')}
          checked={node.targetBlank}
          onChange={(checked) => {
            onChange(node.key, { targetBlank: checked });
          }}
        />
      ) : null}
      {node.type === 'browse' || node.type === 'browseItemSets' ? (
        <TextField
          id={id('query')}
          label={t('navigation.query')}
          value={node.query}
          onChange={(value) => {
            onChange(node.key, { query: value });
          }}
        />
      ) : null}
      {isNavigationType(node.type) ? (
        <TextField
          id={id('label')}
          label={t('navigation.label')}
          value={node.label}
          onChange={(value) => {
            onChange(node.key, { label: value });
          }}
        />
      ) : null}
      <p>
        <button
          type="button"
          onClick={() => {
            onAddChild(node.key);
          }}
        >
          {t('navigation.addChild')}
        </button>
      </p>
      {node.links.length === 0 ? null : (
        <ul>
          {node.links.map((child) => (
            <NavigationNode
              key={child.key}
              node={child}
              pages={pages}
              depth={depth + 1}
              onChange={onChange}
              onRemove={onRemove}
              onMove={onMove}
              onAddChild={onAddChild}
            />
          ))}
        </ul>
      )}
    </li>
  );
};

export interface NavigationEditorProps {
  readonly siteId: SiteId;
  readonly pages: readonly PageSummary[];
  readonly navigation: readonly JsonValue[];
}

export const SiteNavigationEditor = ({
  siteId,
  pages,
  navigation,
}: NavigationEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [nodes, setNodes] = useState<readonly NavigationDraft[]>(() =>
    navigationDrafts(navigation),
  );
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    const pageLinkWithoutPage = nodes.some(
      (node) => node.type === 'page' && node.pageId === null,
    );
    const urlLinkWithoutUrl = nodes.some(
      (node) =>
        node.type === 'url' && (node.url.trim() === '' || node.label.trim() === ''),
    );
    if (pageLinkWithoutPage || urlLinkWithoutUrl) {
      setFormError(t('navigation.errorMissingTarget'));
      return;
    }
    setFormError(null);
    const body = encodeSiteNavigation(navigationJson(nodes));
    save(async () => {
      await api.patchSite(siteId, body);
    });
  };

  const onChange = (key: number, patch: Partial<NavigationDraft>): void => {
    setNodes((current) => patchNavigationLink(current, key, patch));
  };
  const onRemove = (key: number): void => {
    setNodes((current) => removeNavigationLink(current, key));
  };
  const onMove = (key: number, delta: number): void => {
    setNodes((current) => moveNavigationLink(current, key, delta));
  };
  const onAddChild = (key: number): void => {
    setNodes((current) => addNavigationLink(current, key));
  };

  return (
    <form onSubmit={submit}>
      <ul className="navigation-nodes">
        {nodes.map((node) => (
          <NavigationNode
            key={node.key}
            node={node}
            pages={pages}
            depth={0}
            onChange={onChange}
            onRemove={onRemove}
            onMove={onMove}
            onAddChild={onAddChild}
          />
        ))}
      </ul>
      <p>
        <button
          type="button"
          onClick={() => {
            setNodes((current) => addNavigationLink(current, null));
          }}
        >
          {t('navigation.addLink')}
        </button>
      </p>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

interface NavigationLoaded {
  readonly pages: readonly PageSummary[];
  readonly navigation: readonly JsonValue[];
}

const loadNavigation = async (siteId: SiteId): Promise<NavigationLoaded> => {
  const [site, pages] = await Promise.all([
    api.getSite(siteId),
    api.listSitePages({ siteId, perPage: 200 }),
  ]);
  return {
    pages: pages.items.map((page) => ({
      id: page.id,
      title: page.title,
      isPublic: page.isPublic,
    })),
    navigation: site.navigation,
  };
};

/** Loads the site navigation and its page list, then renders the editor. */
export const SiteNavigationPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => loadNavigation(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <SiteNavigationEditor
      key={String(siteId)}
      siteId={siteId}
      pages={state.data.pages}
      navigation={state.data.navigation}
    />
  );
};
