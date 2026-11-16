// Builds the renderer only (src/renderer -> dist/renderer). The main and preload processes are compiled by tsc
// (tsconfig.node.json). base './' because Electron loads the page from the file system.
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
  },
});
