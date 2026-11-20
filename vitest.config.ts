import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: { environment: 'jsdom', include: ['desktop/src/**/*.test.tsx'], setupFiles: ['desktop/src/test/setup.ts'] },
  esbuild: { jsx: 'automatic' },
});
