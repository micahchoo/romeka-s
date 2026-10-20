/**
 * The archive folders the import spec reads, written under the run's work
 * directory before the port starts, so `ROMEKA_ARCHIVE_ROOT` points at them.
 *
 * `BHC_006` is a small copy of the BHC_006 archive's shape: artifacts with a
 * front and a back, photographs with a verso, a register whose first column is
 * each item's accession number, and a note that is no picture. `BHC_007` holds
 * more photographs than the Pair screen pairs by hand, and a register whose
 * numbers are not the file names, so every row of it is paired through the
 * pairs list.
 *
 * The images are 8 by 8 and carry EXIF (Model, Artist, DateTimeOriginal).
 * They were made with Pillow and exiftool:
 * `Image.new('RGB', (8, 8), color).save(name, exif=...)`, then
 * `exiftool "-DateTimeOriginal=1952:03:04 10:00:00"`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { crc32 } from 'node:zlib';

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

/** The register's rows: accession number, title, date. */
export const BHC_ROWS: readonly (readonly [string, string, string])[] = [
  ['BHC_006_AF_001', 'Brass lamp', '1950'],
  ['BHC_006_AF_002', 'Tin box', '1962'],
  ['BHC_006_AF_003', 'Clay pot', '1931'],
  ['BHC_006_PH_001', 'Harbour at dawn', '1952'],
  ['BHC_006_PH_002', 'Street corner', '1955'],
];

export const BHC = 'BHC_006';
export const ARTIFACTS = 'Artifacts';
export const PHOTOGRAPHS = 'Photographs';
export const REGISTER = 'register.csv';

/** The folder of photographs paired by the pairs list, and how many it holds. */
export const PAIRS = 'BHC_007';
export const PAIRS_COUNT = 31;

/**
 * Each file's bytes end with its own name, after the image ends, so no two
 * files are identical: the import asks about identical files, and this run is
 * not about them.
 */
const own = (base64: string, name: string): Buffer => Buffer.concat([Buffer.from(base64, 'base64'), Buffer.from(name)]);
const jpeg = (name: string): Buffer => own(JPEG_FRONT, name);

const register = (): Buffer =>
  Buffer.from(['Accession No,Title,Date', ...BHC_ROWS.map((row) => row.join(','))].join('\n') + '\n');

/** `R001`…: numbers that name no file, so no row pairs by itself. */
export const pairsRow = (n: number): string => `R${String(n).padStart(3, '0')}`;
export const pairsFile = (n: number): string => `photo-${String(n)}.jpg`;

const pairsRegister = (): Buffer =>
  Buffer.from(
    ['Number,Title', ...Array.from({ length: PAIRS_COUNT }, (_, at) => `${pairsRow(at + 1)},Photograph ${String(at + 1)}`)].join('\n') + '\n',
  );

/** The images by path under the archive root. */
const IMAGES: Readonly<Record<string, string>> = {
  [`${BHC}/${ARTIFACTS}/BHC_006_AF_001_front.jpg`]: JPEG_FRONT,
  [`${BHC}/${ARTIFACTS}/BHC_006_AF_001_back.jpg`]: JPEG_BACK,
  [`${BHC}/${ARTIFACTS}/BHC_006_AF_002_front.jpg`]: JPEG_FRONT,
  [`${BHC}/${ARTIFACTS}/BHC_006_AF_002_back.jpg`]: JPEG_BACK,
  [`${BHC}/${ARTIFACTS}/BHC_006_AF_003_front.jpg`]: JPEG_FRONT,
  [`${BHC}/${PHOTOGRAPHS}/BHC_006_PH_001.tif`]: TIFF,
  [`${BHC}/${PHOTOGRAPHS}/BHC_006_PH_001_verso.jpg`]: JPEG_BACK,
  [`${BHC}/${PHOTOGRAPHS}/BHC_006_PH_002.tif`]: TIFF,
  ...Object.fromEntries(Array.from({ length: PAIRS_COUNT }, (_, at) => [`${PAIRS}/${pairsFile(at + 1)}`, JPEG_FRONT])),
};

/** The files by path under the archive root. */
const FILES: Readonly<Record<string, () => Buffer>> = {
  ...Object.fromEntries(Object.entries(IMAGES).map(([path, base64]) => [path, () => own(base64, path)])),
  [`${BHC}/notes.txt`]: () => Buffer.from('Scanned in 2026.\n'),
  [`${BHC}/${REGISTER}`]: register,
  [`${PAIRS}/register.csv`]: pairsRegister,
};

/** The archive root the port is started with. */
export const archiveRoot = (workDir: string): string => join(workDir, 'archive');

/** Write the import folders under the archive root. */
export const writeArchive = (workDir: string): void => {
  for (const [path, bytes] of Object.entries(FILES)) {
    const file = join(archiveRoot(workDir), path);
    mkdirSync(join(file, '..'), { recursive: true });
    writeFileSync(file, bytes());
  }
};

/** A zip of `files`, stored without compression: enough for the unpack job. */
export const zipOf = (files: Readonly<Record<string, Buffer>>): Buffer => {
  const local: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of Object.entries(files)) {
    const path = Buffer.from(name, 'utf8');
    const crc = crc32(data);
    const head = Buffer.alloc(30);
    head.writeUInt32LE(0x04034b50, 0);
    head.writeUInt16LE(20, 4);
    head.writeUInt16LE(0x0800, 6);
    head.writeUInt32LE(crc, 14);
    head.writeUInt32LE(data.length, 18);
    head.writeUInt32LE(data.length, 22);
    head.writeUInt16LE(path.length, 26);
    local.push(head, path, data);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50, 0);
    entry.writeUInt16LE(20, 4);
    entry.writeUInt16LE(20, 6);
    entry.writeUInt16LE(0x0800, 8);
    entry.writeUInt32LE(crc, 16);
    entry.writeUInt32LE(data.length, 20);
    entry.writeUInt32LE(data.length, 24);
    entry.writeUInt16LE(path.length, 28);
    entry.writeUInt32LE(offset, 42);
    central.push(entry, path);
    offset += head.length + path.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(files).length, 8);
  end.writeUInt16LE(Object.keys(files).length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, directory, end]);
};

/** A JPEG for an upload. */
export const uploadJpeg = jpeg;
