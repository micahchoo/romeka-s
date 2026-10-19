/**
 * The navigation editor (`SiteAdmin/IndexController::navigationAction`).
 *
 * The menu shows as a tree of links beside a small rendering of the menu it
 * builds. Every edit is a `NavigationAction` through `navigationReducer`
 * (`navigationTree.ts`); this file owns only the gesture state. A link moves
 * by drag and drop, or by keyboard with the page canvas's pattern: Space on a
 * link's handle picks it up, the arrows move it (Left and Right change its
 * level), Space drops it and Escape puts the tree back. The tree is written
 * with a partial `PATCH` of `o:navigation`.
 */
import { useEffect, useRef, useState } from 'react';
import type { DragEvent, KeyboardEvent, ReactElement } from 'react';
import { api } from '../../api/client';
import { encodeSiteNavigation } from '../../api/write';
import type { JsonValue, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { useUnsavedGuard } from './editor/shell/useUnsavedGuard';
import { NavigationLinkFields, linkLabel, typeLabel } from './NavigationLinkFields';
import type { PageOption } from './NavigationLinkFields';
import {
  keyboardMove,
  linkIssues,
  navigationJson,
  navigationLinks,
  navigationReducer,
} from './navigationTree';
import type { MoveDirection, NavLink, NavigationAction, Place } from './navigationTree';

const ARROWS: Readonly<Record<string, MoveDirection>> = {
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowRight: 'in',
  ArrowLeft: 'out',
};

/** The drop a pointer over a row makes: its top third, middle or bottom third. */
const placeAt = (event: DragEvent<HTMLElement>): Place => {
  const rect = event.currentTarget.getBoundingClientRect();
  const y = (event.clientY - rect.top) / Math.max(1, rect.height);
  return y < 1 / 3 ? 'before' : y > 2 / 3 ? 'after' : 'inside';
};

/** A link's place for the announcement: its position among its siblings and its level. */
const whereIs = (
  links: readonly NavLink[],
  key: number,
  level = 1,
): { readonly position: number; readonly count: number; readonly level: number } | null => {
  const index = links.findIndex((link) => link.key === key);
  if (index !== -1) {
    return { position: index + 1, count: links.length, level };
  }
  for (const link of links) {
    const found = whereIs(link.links, key, level + 1);
    if (found !== null) {
      return found;
    }
  }
  return null;
};

/** What every row reads and calls; the editor owns all of it. */
interface TreeContext {
  readonly pages: readonly PageOption[];
  readonly issues: ReadonlySet<number>;
  readonly openKey: number | null;
  readonly pickedKey: number | null;
  readonly drop: { readonly key: number; readonly place: Place } | null;
  readonly run: (action: NavigationAction) => void;
  readonly toggleOpen: (key: number) => void;
  readonly addChild: (key: number) => void;
  readonly handleRef: (key: number, element: HTMLButtonElement | null) => void;
  readonly onPick: (link: NavLink) => void;
  readonly onHandleKey: (link: NavLink, event: KeyboardEvent<HTMLButtonElement>) => void;
  readonly onDragStart: (key: number) => void;
  readonly onDragOver: (key: number, place: Place) => void;
  readonly onDrop: (key: number, place: Place) => void;
  readonly onDragEnd: () => void;
}

const LinkRow = ({
  link,
  tree,
}: {
  readonly link: NavLink;
  readonly tree: TreeContext;
}): ReactElement => {
  const { t } = useTranslator();
  const label = linkLabel(link, tree.pages, t);
  const open = tree.openKey === link.key;
  const picked = tree.pickedKey === link.key;
  const drop = tree.drop?.key === link.key ? tree.drop.place : undefined;
  return (
    <li className="nav-tree-item">
      <div
        className={picked ? 'nav-tree-row nav-tree-row-picked' : 'nav-tree-row'}
        data-drop={drop}
        draggable
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = 'move';
          event.dataTransfer.setData('text/plain', label);
          tree.onDragStart(link.key);
        }}
        onDragOver={(event) => {
          event.preventDefault();
          tree.onDragOver(link.key, placeAt(event));
        }}
        onDrop={(event) => {
          event.preventDefault();
          tree.onDrop(link.key, placeAt(event));
        }}
        onDragEnd={tree.onDragEnd}
      >
        <button
          type="button"
          className="nav-tree-handle"
          ref={(element) => {
            tree.handleRef(link.key, element);
          }}
          aria-label={t('siteHub.moveLink', { label })}
          aria-pressed={picked}
          onClick={() => {
            tree.onPick(link);
          }}
          onKeyDown={(event) => {
            tree.onHandleKey(link, event);
          }}
        >
          <span aria-hidden="true">⠿</span>
        </button>
        <span className="nav-tree-label">{label}</span>
        <span className="badge">{typeLabel(link.type, t)}</span>
        {tree.issues.has(link.key) ? (
          <span className="badge badge-danger">{t('siteHub.needsTarget')}</span>
        ) : null}
        <span className="nav-tree-actions">
          <button
            type="button"
            className="link-button"
            aria-expanded={open}
            aria-controls={`nav-${String(link.key)}-fields`}
            aria-label={t('siteHub.editLink', { label })}
            onClick={() => {
              tree.toggleOpen(link.key);
            }}
          >
            {t('siteHub.edit')}
          </button>
          <button
            type="button"
            className="link-button"
            aria-label={t('siteHub.addChildTo', { label })}
            onClick={() => {
              tree.addChild(link.key);
            }}
          >
            {t('navigation.addChild')}
          </button>
          <button
            type="button"
            className="link-button link-button-danger"
            aria-label={t('navigation.remove', { label })}
            onClick={() => {
              tree.run({ type: 'remove', key: link.key });
            }}
          >
            {t('action.remove')}
          </button>
        </span>
      </div>
      {open ? (
        <NavigationLinkFields
          link={link}
          pages={tree.pages}
          onChange={(patch) => {
            tree.run({ type: 'patch', key: link.key, patch });
          }}
        />
      ) : null}
      {link.links.length === 0 ? null : (
        <ul className="nav-tree">
          {link.links.map((child) => (
            <LinkRow key={child.key} link={child} tree={tree} />
          ))}
        </ul>
      )}
    </li>
  );
};

/** The menu as the public site would show it: top-level links in a bar, children below. */
const MenuPreview = ({
  links,
  pages,
}: {
  readonly links: readonly NavLink[];
  readonly pages: readonly PageOption[];
}): ReactElement => {
  const { t } = useTranslator();
  const items = (list: readonly NavLink[]): ReactElement => (
    <ul>
      {list.map((link) => (
        <li key={link.key}>
          <span className="menu-preview-link">{linkLabel(link, pages, t)}</span>
          {link.links.length === 0 ? null : items(link.links)}
        </li>
      ))}
    </ul>
  );
  return (
    <aside className="menu-preview" aria-labelledby="menu-preview-title">
      <h3 id="menu-preview-title">{t('siteHub.menuPreview')}</h3>
      <div className="menu-preview-bar">
        {links.length === 0 ? (
          <ul>
            <li>
              <span className="menu-preview-link">{t('siteHub.homeLink')}</span>
            </li>
          </ul>
        ) : (
          items(links)
        )}
      </div>
      {links.length === 0 ? <p className="field-note">{t('siteHub.emptyMenuNote')}</p> : null}
    </aside>
  );
};

export interface NavigationEditorProps {
  readonly siteId: SiteId;
  readonly pages: readonly PageOption[];
  readonly navigation: readonly JsonValue[];
}

const newestKey = (links: readonly NavLink[]): number =>
  links.reduce((max, link) => Math.max(max, link.key, newestKey(link.links)), -1);

export const SiteNavigationEditor = ({
  siteId,
  pages,
  navigation,
}: NavigationEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [links, setLinks] = useState<readonly NavLink[]>(() => navigationLinks(navigation));
  const [savedJson, setSavedJson] = useState(() => JSON.stringify(navigationJson(links)));
  const [openKey, setOpenKey] = useState<number | null>(null);
  const [picked, setPicked] = useState<{
    readonly key: number;
    readonly before: readonly NavLink[];
  } | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  const [drop, setDrop] = useState<{ readonly key: number; readonly place: Place } | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const handles = useRef(new Map<number, HTMLButtonElement>());
  const { state: saveState, save } = useSave();

  const json = navigationJson(links);
  const dirty = JSON.stringify(json) !== savedJson;
  useUnsavedGuard(dirty, t('siteHub.unsavedConfirm'));
  const issues = linkIssues(links);

  const pickedKey = picked?.key ?? null;
  // A move across levels re-mounts the row, so focus follows the picked link.
  useEffect(() => {
    if (pickedKey !== null) {
      handles.current.get(pickedKey)?.focus();
    }
  }, [links, pickedKey]);

  const run = (action: NavigationAction): readonly NavLink[] => {
    const next = navigationReducer(links, action);
    setLinks(next);
    return next;
  };

  const add = (parent: number | null): void => {
    const next = run({ type: 'add', parent, linkType: 'page' });
    setOpenKey(newestKey(next));
  };

  const tree: TreeContext = {
    pages,
    issues,
    openKey,
    pickedKey,
    drop,
    run,
    toggleOpen: (key) => {
      setOpenKey((current) => (current === key ? null : key));
    },
    addChild: add,
    handleRef: (key, element) => {
      if (element === null) {
        handles.current.delete(key);
      } else {
        handles.current.set(key, element);
      }
    },
    onPick: (link) => {
      const label = linkLabel(link, pages, t);
      if (pickedKey === link.key) {
        setPicked(null);
        setAnnouncement(t('siteHub.dropped', { label }));
        return;
      }
      setPicked({ key: link.key, before: links });
      setAnnouncement(t('siteHub.picked', { label }));
    },
    onHandleKey: (link, event) => {
      if (pickedKey !== link.key) {
        return;
      }
      const label = linkLabel(link, pages, t);
      if (event.key === 'Escape' && picked !== null) {
        event.preventDefault();
        setLinks(picked.before);
        setPicked(null);
        setAnnouncement(t('siteHub.cancelled', { label }));
        return;
      }
      const direction = ARROWS[event.key];
      if (direction === undefined) {
        return;
      }
      event.preventDefault();
      const action = keyboardMove(links, link.key, direction);
      if (action === null) {
        return;
      }
      const where = whereIs(run(action), link.key);
      if (where !== null) {
        setAnnouncement(t('siteHub.moved', { label, ...where }));
      }
    },
    onDragStart: setDragging,
    onDragOver: (key, place) => {
      if (drop?.key !== key || drop.place !== place) {
        setDrop({ key, place });
      }
    },
    onDrop: (key, place) => {
      if (dragging !== null) {
        run({ type: 'move', key: dragging, target: key, place });
      }
      setDragging(null);
      setDrop(null);
    },
    onDragEnd: () => {
      setDragging(null);
      setDrop(null);
    },
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    if (issues.size > 0) {
      setFormError(t('navigation.errorMissingTarget'));
      return;
    }
    setFormError(null);
    save(async () => {
      await api.patchSite(siteId, encodeSiteNavigation(json));
      setSavedJson(JSON.stringify(json));
    });
  };

  return (
    <form className="nav-editor" onSubmit={submit}>
      <div className="nav-editor-tree">
        <p className="field-note">{t('siteHub.treeHelp')}</p>
        {links.length === 0 ? <p>{t('siteHub.noLinks')}</p> : null}
        <ul className="nav-tree" aria-label={t('siteHub.treeLabel')}>
          {links.map((link) => (
            <LinkRow key={link.key} link={link} tree={tree} />
          ))}
        </ul>
        <p className="nav-live" role="status" aria-live="polite">
          {announcement}
        </p>
        <p>
          <button
            type="button"
            onClick={() => {
              add(null);
            }}
          >
            {t('navigation.addLink')}
          </button>
        </p>
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <div className="nav-editor-save">
          <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
          {dirty ? (
            <span className="editor-dirty" role="status">
              <span className="editor-dirty-dot" aria-hidden="true" />
              {t('siteHub.unsaved')}
            </span>
          ) : null}
        </div>
      </div>
      <MenuPreview links={links} pages={pages} />
    </form>
  );
};

interface NavigationLoaded {
  readonly pages: readonly PageOption[];
  readonly navigation: readonly JsonValue[];
}

const loadNavigation = async (siteId: SiteId): Promise<NavigationLoaded> => {
  const [site, pages] = await Promise.all([
    api.getSite(siteId),
    api.listSitePages({ siteId, perPage: 200 }),
  ]);
  return {
    pages: pages.items.map((page) => ({ id: page.id, title: page.title })),
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
