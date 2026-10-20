/**
 * One preview read for a list, made again whenever the saved answers change
 * (the import's revision) or what the caller asks changes. The last answer
 * stays on screen while the next loads, so a list never blinks.
 */
import { useEffect, useState } from 'react';
import { importApi } from '../../../api/import-routes';
import type { PreviewOptions } from '../../../api/import-queries';
import type { PreviewWith, Section } from '../../../api/import-sections';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';

export type Read<T> =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'ready'; readonly data: T; readonly busy: boolean };

export const usePreview = <S extends Section>(
  include: readonly S[],
  options: PreviewOptions,
): Read<PreviewWith<S>> => {
  const { id, detail } = useDraft();
  const [read, setRead] = useState<Read<PreviewWith<S>>>({ status: 'loading' });
  const asked = JSON.stringify([include, options]);
  useEffect(() => {
    let current = true;
    setRead((last) => (last.status === 'ready' ? { ...last, busy: true } : last));
    importApi.preview(id, include, options).then(
      (data) => {
        if (current) {
          setRead({ status: 'ready', data, busy: false });
        }
      },
      (error: unknown) => {
        if (current) {
          setRead({ status: 'error', error: toError(error) });
        }
      },
    );
    return () => {
      current = false;
    };
    // `asked` holds `include` and `options`, which are new objects on every render.
  }, [id, detail.revision, asked]);
  return read;
};
