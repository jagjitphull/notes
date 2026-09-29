import { vi } from "vitest";

// jsdom doesn't implement matchMedia; useTheme.ts calls it at module load
// time (before any test's own setup runs), so this has to be a global
// setup file rather than a per-test mock.
if (!window.matchMedia) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}
