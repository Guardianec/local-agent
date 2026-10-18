import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';
window.matchMedia = vi.fn().mockImplementation(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
