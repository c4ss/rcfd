// Playwright runs two projects (npm test builds and packages the app first):
//   unit      tests/unit: pure checks of src/main/security.ts and the built index.html, no Electron
//   electron  tests/e2e: the packaged shell (out/packaged, scripts/package-dir.mjs) driven by _electron.launch
// One worker: each e2e file starts its own Electron, and the timings of a shared machine stay out of the way.
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests',
  outputDir: 'test-results',
  workers: 1,
  retries: 0,
  forbidOnly: true,
  reporter: [['list']],
  timeout: 60_000,
  projects: [
    { name: 'unit', testDir: 'tests/unit' },
    { name: 'electron', testDir: 'tests/e2e' },
  ],
});
