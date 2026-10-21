/**
 * A generated PNG for a spec that needs a picture with known pixels.
 *
 * The seeding run writes one 8 by 8 file; a spec that measures where a marker
 * lands needs a picture large enough to see and to scale. A blue sea with a red
 * band down the middle, so a zoom or a region over the band shows.
 */
import { crc32, deflateSync } from 'node:zlib';

export const png = (width: number, height: number): Buffer => {
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + width * 3);
    for (let x = 0; x < width; x += 1) {
      const band = Math.abs(y - height / 2) < height / 12;
      row.set(band ? [200, 40, 40] : [30, 90, 160 + Math.round((x / width) * 80)], 1 + x * 3);
    }
    rows.push(row);
  }
  const chunk = (kind: string, body: Buffer): Buffer => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(body.length);
    const typed = Buffer.concat([Buffer.from(kind, 'ascii'), body]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc32(typed));
    return Buffer.concat([length, typed, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
};
