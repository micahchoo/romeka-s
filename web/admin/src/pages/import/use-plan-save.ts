/**
 * The draft's plan, saved a moment after it last changed (`PUT
 * /api/romeka/imports/{id}`). Saves run one after another, so an older plan
 * never lands after a newer one, and a change still waiting when the page
 * closes is saved then. Once the import starts its plan is fixed: a change
 * still waiting is dropped, and a save the server refuses then is expected.
 */
import { useEffect, useRef, useState } from 'react';
import { api } from '../../api/client';
import { saveImportPlan } from '../../api/imports';
import type { ImportId, ImportPlan } from '../../api/imports';
import { toError } from '../../app/useAsync';

export const SAVE_DELAY_MS = 600;

export type SaveState =
  | { readonly status: 'idle' }
  | { readonly status: 'saving' }
  | { readonly status: 'saved' }
  | { readonly status: 'error'; readonly error: Error };

export const usePlanSave = (id: ImportId, plan: ImportPlan, editable: boolean): SaveState => {
  const [state, setState] = useState<SaveState>({ status: 'idle' });
  const stored = useRef(plan);
  const waiting = useRef<ImportPlan | null>(null);
  const chain = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useRef(true);
  const open = useRef(editable);
  open.current = editable;

  const save = (next: ImportPlan): void => {
    waiting.current = null;
    setState({ status: 'saving' });
    chain.current = chain.current
      .then(() => saveImportPlan(api, id, next))
      .then(
        () => {
          stored.current = next;
          if (mounted.current && waiting.current === null) {
            setState({ status: 'saved' });
          }
        },
        (error: unknown) => {
          if (mounted.current && open.current) {
            setState({ status: 'error', error: toError(error) });
          }
        },
      );
  };
  const saveRef = useRef(save);
  saveRef.current = save;

  useEffect(() => {
    if (!editable) {
      waiting.current = null;
      return undefined;
    }
    if (plan === stored.current) {
      return undefined;
    }
    waiting.current = plan;
    const timer = window.setTimeout(() => {
      saveRef.current(plan);
    }, SAVE_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [plan, editable]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      const pending = waiting.current;
      if (pending !== null) {
        saveRef.current(pending);
      }
    };
  }, []);

  return state;
};
