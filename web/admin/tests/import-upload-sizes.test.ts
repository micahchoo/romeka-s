/** Sizes print in decimal units, so "1 GB" is 1,000,000,000 bytes, the number an administrator types in the limit settings. */
import { describe, expect, it } from 'vitest';
import { sizeText } from '../src/pages/import/upload/text';

describe('sizeText', () => {
  it('counts a kilobyte as 1,000 bytes and a gigabyte as 1,000,000,000', () => {
    expect(sizeText(999, 'en')).toBe('999 bytes');
    expect(sizeText(1000, 'en')).toBe('1 kB');
    expect(sizeText(1_000_000, 'en')).toBe('1 MB');
    expect(sizeText(1_000_000_000, 'en')).toBe('1 GB');
    expect(sizeText(2_500_000_000, 'en')).toBe('2.5 GB');
  });

  it('prints the default limits as the bytes they are', () => {
    expect(sizeText(2 * 1024 ** 3, 'en')).toBe('2.1 GB');
    expect(sizeText(50 * 1024 ** 3, 'en')).toBe('53.7 GB');
  });
});
