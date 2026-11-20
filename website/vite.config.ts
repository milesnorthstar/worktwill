import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import { pages } from './site.config.mjs';

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  server: { port: 4321, strictPort: true },
  preview: { port: 4321, strictPort: true },
  build: {
    outDir: 'dist', emptyOutDir: true, target: 'es2022',
    rollupOptions: {
      input: Object.fromEntries(['', 'en/'].flatMap((locale) => Object.keys(pages).map((page) => [
        `${locale}${page}`, fileURLToPath(new URL(`./${locale}${page}.html`, import.meta.url)),
      ]))),
    },
  },
});
