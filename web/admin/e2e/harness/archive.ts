/**
 * The archive folder the import specs read, written under the run's work
 * directory before the port starts, so `ROMEKA_ARCHIVE_ROOT` points at it.
 *
 * Two top folders make two item sets. `LT_001` is a front and back pair, the
 * photographs are TIFF masters with no part in their names, `readme.txt`
 * matches no item, and the spreadsheet has a title line above its header,
 * semicolons, one cell in Windows-1252 and one empty title.
 *
 * The images are 8 by 8 and carry EXIF (Model, Artist, DateTimeOriginal).
 * They were made with Pillow and exiftool:
 * `Image.new('RGB', (8, 8), color).save(name, exif=...)`, then
 * `exiftool "-DateTimeOriginal=1952:03:04 10:00:00"`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

/** The folder under the archive root that the specs import. */
export const ARCHIVE_FOLDER = 'family-archive';
export const LETTERS = 'Letters';
export const PHOTOS = 'Photos';
export const CSV_FILE = 'catalogue.csv';

const JPEG_FRONT =
  '/9j/4AAQSkZJRgABAQAAAQABAAD/4QCiRXhpZgAATU0AKgAAAAgAAwEQAAIAAAALAAAAMgE7AAIAAAARAAAAPodpAAQAAAAB' +
  'AAAAUAAAAABFMkUgQ2FtZXJhAABFMkUgUGhvdG9ncmFwaGVyAAAABJAAAAcAAAAEMDIzMpADAAIAAAAUAAAAhpEBAAcAAAAE' +
  'AQIDAKABAAMAAAAB//8AAAAAAAAxOTUyOjAzOjA0IDEwOjAwOjAwAP/bAEMABgQFBgUEBgYFBgcHBggKEAoKCQkKFA4PDBAX' +
  'FBgYFxQWFhodJR8aGyMcFhYgLCAjJicpKikZHy0wLSgwJSgpKP/bAEMBBwcHCggKEwoKEygaFhooKCgoKCgoKCgoKCgoKCgo' +
  'KCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKP/AABEIAAgACAMBIgACEQEDEQH/xAAfAAABBQEBAQEBAQAAAAAA' +
  'AAAAAQIDBAUGBwgJCgv/xAC1EAACAQMDAgQDBQUEBAAAAX0BAgMABBEFEiExQQYTUWEHInEUMoGRoQgjQrHBFVLR8CQzYnKC' +
  'CQoWFxgZGiUmJygpKjQ1Njc4OTpDREVGR0hJSlNUVVZXWFlaY2RlZmdoaWpzdHV2d3h5eoOEhYaHiImKkpOUlZaXmJmaoqOk' +
  'paanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4eLj5OXm5+jp6vHy8/T19vf4+fr/xAAfAQADAQEBAQEBAQEBAAAA' +
  'AAAAAQIDBAUGBwgJCgv/xAC1EQACAQIEBAMEBwUEBAABAncAAQIDEQQFITEGEkFRB2FxEyIygQgUQpGhscEJIzNS8BVictEK' +
  'FiQ04SXxFxgZGiYnKCkqNTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqCg4SFhoeIiYqSk5SVlpeYmZqi' +
  'o6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2dri4+Tl5ufo6ery8/T19vf4+fr/2gAMAwEAAhEDEQA/AOaooor5' +
  'g/Uj/9k=';
const JPEG_BACK =
  '/9j/4AAQSkZJRgABAQAAAQABAAD/4QCiRXhpZgAATU0AKgAAAAgAAwEQAAIAAAALAAAAMgE7AAIAAAARAAAAPodpAAQAAAAB' +
  'AAAAUAAAAABFMkUgQ2FtZXJhAABFMkUgUGhvdG9ncmFwaGVyAAAABJAAAAcAAAAEMDIzMpADAAIAAAAUAAAAhpEBAAcAAAAE' +
  'AQIDAKABAAMAAAAB//8AAAAAAAAxOTUyOjAzOjA0IDEwOjAwOjAwAP/bAEMABgQFBgUEBgYFBgcHBggKEAoKCQkKFA4PDBAX' +
  'FBgYFxQWFhodJR8aGyMcFhYgLCAjJicpKikZHy0wLSgwJSgpKP/bAEMBBwcHCggKEwoKEygaFhooKCgoKCgoKCgoKCgoKCgo' +
  'KCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKP/AABEIAAgACAMBIgACEQEDEQH/xAAfAAABBQEBAQEBAQAAAAAA' +
  'AAAAAQIDBAUGBwgJCgv/xAC1EAACAQMDAgQDBQUEBAAAAX0BAgMABBEFEiExQQYTUWEHInEUMoGRoQgjQrHBFVLR8CQzYnKC' +
  'CQoWFxgZGiUmJygpKjQ1Njc4OTpDREVGR0hJSlNUVVZXWFlaY2RlZmdoaWpzdHV2d3h5eoOEhYaHiImKkpOUlZaXmJmaoqOk' +
  'paanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4eLj5OXm5+jp6vHy8/T19vf4+fr/xAAfAQADAQEBAQEBAQEBAAAA' +
  'AAAAAQIDBAUGBwgJCgv/xAC1EQACAQIEBAMEBwUEBAABAncAAQIDEQQFITEGEkFRB2FxEyIygQgUQpGhscEJIzNS8BVictEK' +
  'FiQ04SXxFxgZGiYnKCkqNTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqCg4SFhoeIiYqSk5SVlpeYmZqi' +
  'o6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2dri4+Tl5ufo6ery8/T19vf4+fr/2gAMAwEAAhEDEQA/AMCiiivv' +
  'T4k//9k=';
const TIFF =
  'SUkqAAgAAAANAAABBAABAAAACAAAAAEBBAABAAAACAAAAAIBAwADAAAAqgAAAAMBAwABAAAAAQAAAAYBAwABAAAAAgAAABAB' +
  'AgALAAAAsAAAABEBBAABAAAAGAEAABUBAwABAAAAAwAAABYBBAABAAAACAAAABcBBAABAAAAwAAAABwBAwABAAAAAQAAADsB' +
  'AgARAAAAvAAAAGmHBAABAAAAzgAAAAAAAAAIAAgACABFMkUgQ2FtZXJhAABFMkUgUGhvdG9ncmFwaGVyAAAEAACQBwAEAAAA' +
  'MDIzMgOQAgAUAAAABAEAAAGRBwAEAAAAAQIDAAGgAwABAAAA//8AAAAAAAAxOTUyOjAzOjA0IDEwOjAwOjAwADygUDygUDyg' +
  'UDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDyg' +
  'UDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDyg' +
  'UDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUDygUA==';

/** The title each spreadsheet row gives; `PH_102` has none. */
export const CSV_TITLES: Readonly<Record<string, string>> = {
  LT_001: 'Letter from Anna',
  LT_002: 'Letter from Ben',
  PH_101: 'Harbour at dawn',
};

/** `Café` with `é` as the single Windows-1252 byte 0xE9, which is not UTF-8. */
const csvBytes = (): Buffer =>
  Buffer.concat([
    Buffer.from(
      'Family archive catalogue, exported 2026-10-01\n' +
        'identifier;title;notes\n' +
        `LT_001;${CSV_TITLES.LT_001 ?? ''};First letter\n` +
        `LT_002;${CSV_TITLES.LT_002 ?? ''};Caf`,
    ),
    Buffer.from([0xe9]),
    Buffer.from(` visit\nPH_101;${CSV_TITLES.PH_101 ?? ''};Boats\nPH_102;;Street corner\n`),
  ]);

/** The files by path inside the import folder. */
const FILES: Readonly<Record<string, () => Buffer>> = {
  [`${LETTERS}/LT_001_back.jpg`]: () => Buffer.from(JPEG_BACK, 'base64'),
  [`${LETTERS}/LT_001_front.jpg`]: () => Buffer.from(JPEG_FRONT, 'base64'),
  [`${LETTERS}/LT_002_front.jpg`]: () => Buffer.from(JPEG_FRONT, 'base64'),
  [`${LETTERS}/readme.txt`]: () => Buffer.from('Scanned in 2026.\n'),
  [`${PHOTOS}/PH_101.tif`]: () => Buffer.from(TIFF, 'base64'),
  [`${PHOTOS}/PH_102.tif`]: () => Buffer.from(TIFF, 'base64'),
  [CSV_FILE]: csvBytes,
};

/** The archive root the port is started with. */
export const archiveRoot = (workDir: string): string => join(workDir, 'archive');

/** Write the import folder under the archive root. */
export const writeArchive = (workDir: string): void => {
  const folder = join(archiveRoot(workDir), ARCHIVE_FOLDER);
  for (const [path, bytes] of Object.entries(FILES)) {
    const file = join(folder, path);
    mkdirSync(join(file, '..'), { recursive: true });
    writeFileSync(file, bytes());
  }
};
