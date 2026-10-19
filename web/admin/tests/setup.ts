import '@testing-library/jest-dom/vitest';
import { cleanup, configure } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

// The 1 s findBy/waitFor default failed lazy-chunk renders that take ~1.1 s
// under the load of parallel gates; a real hang still fails at 5 s.
configure({ asyncUtilTimeout: 5000 });

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// jsdom has no layout, so it ships no ResizeObserver. A box that never resizes
// is what jsdom would report anyway.
class StillResizeObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}
globalThis.ResizeObserver = StillResizeObserver;

// jsdom has no layout, so it ships no scrollIntoView either.
Element.prototype.scrollIntoView = function scrollIntoView(): void {};
