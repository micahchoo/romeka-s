/**
 * What a drop or a pick adds (`app/upload/pick.ts`): paths relative to what
 * was added, folders read in batches, and dot files left out and counted.
 */
import { describe, expect, it } from 'vitest';
import { fromDrop, fromInput } from '../src/app/upload/pick';
import type { DroppedEntry, DroppedFile, DroppedFolder } from '../src/app/upload/pick';

const dropped = (name: string): DroppedFile => ({
  isFile: true,
  isDirectory: false,
  name,
  file: (success) => {
    success(new File(['x'], name));
  },
});

/** A folder whose reader answers two entries per batch, then an empty batch, as browsers do for large folders. */
const folder = (name: string, children: readonly DroppedEntry[]): DroppedFolder => ({
  isFile: false,
  isDirectory: true,
  name,
  createReader: () => {
    let at = 0;
    return {
      readEntries: (success) => {
        success(children.slice(at, at + 2));
        at += 2;
      },
    };
  },
});

const withRelativePath = (name: string, relative: string): File => {
  const picked = new File(['x'], name);
  Object.defineProperty(picked, 'webkitRelativePath', { value: relative });
  return picked;
};

describe('a drop', () => {
  it('keeps every path relative to what was dropped and reads a folder past its first batch', async () => {
    const tree = folder('Box 1', [
      dropped('0001.tif'),
      dropped('0002.tif'),
      dropped('.DS_Store'),
      folder('.git', [dropped('HEAD')]),
      folder('Scans', [dropped('0003.tif')]),
    ]);
    const picked = await fromDrop([tree, dropped('list.csv'), null], []);
    expect(picked.files.map((file) => file.path)).toEqual(['Box 1/0001.tif', 'Box 1/0002.tif', 'Box 1/Scans/0003.tif', 'list.csv']);
    expect(picked.skipped).toBe(2);
  });

  it('falls back to the dropped files when the browser gives no entries', async () => {
    const picked = await fromDrop([null], [new File(['x'], 'a.zip'), new File(['x'], '.hidden')]);
    expect(picked.files.map((file) => file.path)).toEqual(['a.zip']);
    expect(picked.skipped).toBe(1);
  });
});

describe('a pick', () => {
  it('uses the folder-relative path of a folder pick and leaves out files under a dot folder', () => {
    const picked = fromInput([
      withRelativePath('0001.tif', 'Box 1/0001.tif'),
      withRelativePath('config', 'Box 1/.cache/config'),
      new File(['x'], 'single.jpg'),
    ]);
    expect(picked.files.map((file) => file.path)).toEqual(['Box 1/0001.tif', 'single.jpg']);
    expect(picked.skipped).toBe(1);
  });
});
