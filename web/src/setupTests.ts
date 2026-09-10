import '@testing-library/jest-dom/vitest';

import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// Vitest runs without `globals`, so Testing Library's auto-cleanup isn't registered.
// Unmount rendered trees between tests so repeated renders don't accumulate in the DOM.
afterEach(() => {
  cleanup();
});
