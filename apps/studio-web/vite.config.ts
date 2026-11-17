// Builds the renderer only (src/renderer -> dist/renderer). The main and preload processes are compiled by tsc
// (tsconfig.node.json). base './' so the page's URLs stay relative; the main
// process serves dist/renderer at app://studio/ (src/main/security.ts).
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  root: 'src/renderer',
  base: './',
  plugins: [react()],
  build: {
    outDir: '../../dist/renderer',
    emptyOutDir: true,
    target: 'es2023',
    // Every asset a file of its own (no data: URLs), so the Content Security Policy can stay at 'self'.
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
  },
});
