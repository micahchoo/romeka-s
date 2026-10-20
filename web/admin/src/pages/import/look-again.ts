/**
 * "Look at the files again": one rescan of the import's sources, from the
 * button or from files that arrived. A look asked while one is out runs once
 * more when it ends, so the last arrival is always counted. A look that ends
 * tells the frame (`looked`), and the previews count again.
 */
import { useRef, useState } from 'react';
import { importApi } from '../../api/import-routes';
import type { ScanResult } from '../../api/import-view';
import { toError } from '../../app/useAsync';
import { useDraft } from './draft-context';

export type Look =
  | { readonly at: 'idle' }
  | { readonly at: 'looking' }
  | { readonly at: 'done'; readonly result: ScanResult }
  | { readonly at: 'failed'; readonly error: Error };

export const useLookAgain = (): { readonly look: Look; readonly lookAgain: () => void } => {
  const { id, looked } = useDraft();
  const [look, setLook] = useState<Look>({ at: 'idle' });
  const out = useRef(false);
  const asked = useRef(false);
  const lookAgain = (): void => {
    if (out.current) {
      asked.current = true;
      return;
    }
    out.current = true;
    asked.current = false;
    setLook({ at: 'looking' });
    const settle = (next: Look): void => {
      out.current = false;
      setLook(next);
      looked();
      if (asked.current) {
        lookAgain();
      }
    };
    importApi.scan(id).then(
      (result) => {
        settle({ at: 'done', result });
      },
      (error: unknown) => {
        settle({ at: 'failed', error: toError(error) });
      },
    );
  };
  return { look, lookAgain };
};
