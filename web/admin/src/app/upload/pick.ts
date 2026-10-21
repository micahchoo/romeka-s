/**
 * The files a person dropped or picked, each with its path relative to what
 * was added ("Box 1/0001.tif"), and the dot files left out.
 *
 * The entry types are the parts of `FileSystemEntry` this module reads, so the
 * browser's entries pass in as they are and a test builds a tree by hand.
 */

export interface PickedFile {
  readonly path: string;
  readonly file: File;
}

export interface Picked {
  readonly files: readonly PickedFile[];
  /** Files, or whole folders, left out because their name starts with a dot. */
  readonly skipped: number;
}

export interface DroppedFile {
  readonly isFile: true;
  readonly isDirectory: false;
  readonly name: string;
  file(success: (file: File) => void, failure?: (error: DOMException) => void): void;
}

export interface DroppedFolder {
  readonly isFile: false;
  readonly isDirectory: true;
  readonly name: string;
  createReader(): {
    readEntries(success: (entries: readonly DroppedEntry[]) => void, failure?: (error: DOMException) => void): void;
  };
}

export type DroppedEntry = DroppedFile | DroppedFolder;

const isBrowserFolder = (entry: FileSystemEntry): entry is FileSystemDirectoryEntry => entry.isDirectory;
const isBrowserFile = (entry: FileSystemEntry): entry is FileSystemFileEntry => entry.isFile;

/** A browser entry as this module reads it; `null` for an entry that is neither a file nor a folder. */
export const fromBrowserEntry = (entry: FileSystemEntry | null): DroppedEntry | null => {
  if (entry === null) {
    return null;
  }
  if (isBrowserFile(entry)) {
    return { isFile: true, isDirectory: false, name: entry.name, file: (success, failure) => { entry.file(success, failure); } };
  }
  if (isBrowserFolder(entry)) {
    return {
      isFile: false,
      isDirectory: true,
      name: entry.name,
      createReader: () => {
        const reader = entry.createReader();
        return {
          readEntries: (success, failure) => {
            reader.readEntries((list) => {
              success(list.map(fromBrowserEntry).filter((child): child is DroppedEntry => child !== null));
            }, failure);
          },
        };
      },
    };
  }
  return null;
};

const hidden = (path: string): boolean => path.split('/').some((part) => part.startsWith('.'));

export const keepVisible = (files: readonly PickedFile[]): Picked => {
  const kept = files.filter((picked) => !hidden(picked.path));
  return { files: kept, skipped: files.length - kept.length };
};

/** Files from an `<input type="file">`: a folder pick carries `webkitRelativePath`. */
export const fromInput = (files: Iterable<File>): Picked =>
  keepVisible([...files].map((file) => ({ path: file.webkitRelativePath || file.name, file })));

const readFile = (entry: DroppedFile): Promise<File> =>
  new Promise((resolve, reject) => {
    entry.file(resolve, reject);
  });

/** Every entry of a folder: `readEntries` answers in batches until one is empty. */
const readFolder = async (entry: DroppedFolder): Promise<readonly DroppedEntry[]> => {
  const reader = entry.createReader();
  const all: DroppedEntry[] = [];
  for (;;) {
    const batch = await new Promise<readonly DroppedEntry[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    if (batch.length === 0) {
      return all;
    }
    all.push(...batch);
  }
};

interface Gathered {
  readonly files: PickedFile[];
  skipped: number;
}

const collect = async (entry: DroppedEntry, prefix: string, into: Gathered): Promise<void> => {
  const path = `${prefix}${entry.name}`;
  if (entry.name.startsWith('.')) {
    // A dot folder counts as one, unread: it can hold thousands (`.git`).
    into.skipped += 1;
    return;
  }
  if (entry.isFile) {
    into.files.push({ path, file: await readFile(entry) });
    return;
  }
  for (const child of await readFolder(entry)) {
    await collect(child, `${path}/`, into);
  }
};

/**
 * Files from a drop. `entries` holds `webkitGetAsEntry()` of each item, read
 * inside the drop handler because the items die with the event; `files` is
 * the fallback for a browser that gives no entries.
 */
export const fromDrop = async (entries: readonly (DroppedEntry | null)[], files: Iterable<File>): Promise<Picked> => {
  const present = entries.filter((entry): entry is DroppedEntry => entry !== null);
  if (present.length === 0) {
    return fromInput(files);
  }
  const into: Gathered = { files: [], skipped: 0 };
  for (const entry of present) {
    await collect(entry, '', into);
  }
  return into;
};
