/**
 * The editor's save: create a new page, or save an edited one with the
 * `o:modified` its draft is based on. A save the server refuses because the
 * page was saved since becomes a conflict, which the user resolves by loading
 * their version (mine stays in undo) or by saving mine over it.
 */
import { useState } from 'react';
import { api } from '../../../../api/client';
import type { PageAuthor, SiteId, SitePage } from '../../../../api/types';
import { useSave } from '../../../../app/save';
import type { SaveState } from '../../../../app/save';
import type { DraftAction, PageContent } from '../draft';
import { toPageWrite } from '../wire';

/** A save the server refused: who saved the stored page, and when. */
export interface Conflict {
  readonly modified: string | null;
  readonly author: PageAuthor | null;
}

export interface PageSaveOptions {
  readonly page: SitePage | null;
  readonly siteId: SiteId;
  readonly draft: PageContent;
  readonly valid: boolean;
  readonly dispatch: (action: DraftAction) => void;
  /** After a create, before the route moves to the new page. */
  readonly onCreated: (page: SitePage) => void;
  /** The status line's message for a refused save. */
  readonly notSaved: string;
}

export interface PageSave {
  readonly state: SaveState;
  /** The `o:modified` of the stored page the draft is based on. */
  readonly loadedModified: string | null;
  readonly conflict: Conflict | null;
  /** True while their version loads. */
  readonly resolving: boolean;
  readonly save: () => void;
  readonly showTheirs: () => void;
  readonly keepMine: () => void;
  readonly dismiss: () => void;
}

export const usePageSave = ({
  page,
  siteId,
  draft,
  valid,
  dispatch,
  onCreated,
  notSaved,
}: PageSaveOptions): PageSave => {
  const { state, save } = useSave();
  const [loadedModified, setLoadedModified] = useState<string | null>(page?.modified ?? null);
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [resolving, setResolving] = useState(false);

  const saveAs = (claim: string | null): void => {
    if (!valid || state.status === 'saving') {
      return;
    }
    const write = toPageWrite(draft, siteId);
    save(async () => {
      if (page === null) {
        onCreated(await api.createSitePage(write));
        return;
      }
      const result = await api.updateSitePage(page.id, write, claim);
      if (result.kind === 'stale') {
        setConflict({ modified: result.modified, author: result.author });
        throw new Error(notSaved);
      }
      setConflict(null);
      setLoadedModified(result.page.modified);
      dispatch({ type: 'markSaved' });
    });
  };

  const showTheirs = (): void => {
    if (page === null) {
      return;
    }
    setResolving(true);
    api.getSitePage(page.id).then(
      (theirs) => {
        setResolving(false);
        setConflict(null);
        setLoadedModified(theirs.modified);
        dispatch({ type: 'loadPage', page: theirs, saved: true });
      },
      () => {
        setResolving(false);
      },
    );
  };

  return {
    state,
    loadedModified,
    conflict,
    resolving,
    save: () => {
      saveAs(loadedModified);
    },
    showTheirs,
    keepMine: () => {
      const claim = conflict?.modified ?? null;
      setConflict(null);
      saveAs(claim);
    },
    dismiss: () => {
      setConflict(null);
    },
  };
};
