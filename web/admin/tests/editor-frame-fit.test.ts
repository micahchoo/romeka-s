import { describe, expect, it } from 'vitest';

import { fitFrame } from '../src/pages/sites/editor/frameFit';

describe('fitFrame', () => {
  it('lays a desktop page out at 1280 px and shrinks it into a narrow panel', () => {
    expect(fitFrame(1280, { width: 320, height: 600 })).toEqual({
      width: 1280,
      height: 2400,
      scale: 0.25,
    });
  });

  it('never enlarges a device narrower than the panel', () => {
    expect(fitFrame(390, { width: 800, height: 600 })).toEqual({
      width: 390,
      height: 600,
      scale: 1,
    });
  });
});
