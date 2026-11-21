import { defineConfig } from 'vitest/config';

export default defineConfig({
  // Windows 上大型 jsdom 列表用例串行运行，保留默认超时及全部断言。
  test: { fileParallelism: process.platform !== 'win32', environment: 'jsdom', include: ['desktop/src/**/*.test.tsx'], setupFiles: ['desktop/src/test/setup.ts'] },
  esbuild: { jsx: 'automatic' },
});
