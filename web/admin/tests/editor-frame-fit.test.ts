import { describe, expect, it } from 'vitest';

import { DEVICES, fitFrame } from '../src/pages/sites/editor/frameFit';

describe('fitFrame', () => {
  it('lays a desktop page out at 1280 px and shrinks it into a narrow panel', () => {
    expect(fitFrame(1280, { width: 320, height: 600 })).toEqual({
      width: 1280,
      height: 2400,
      scale: 0.25,
      left: 0,
    });
  });

  it('never enlarges a device narrower than the panel, and centres it', () => {
    expect(fitFrame(390, { width: 800, height: 600 })).toEqual({
      width: 390,
      height: 600,
      scale: 1,
      left: 205,
    });
  });

  it('lays the page out at the panel width itself under Fit, at full scale', () => {
    expect(fitFrame('fit', { width: 900, height: 600 })).toEqual({
      width: 900,
      height: 600,
      scale: 1,
      left: 0,
    });
  });

  it('offers Fit first, as the default, then the three fixed devices', () => {
    expect(DEVICES.map((device) => device.width)).toEqual(['fit', 1280, 768, 390]);
  });
});
