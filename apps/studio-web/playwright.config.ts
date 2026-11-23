// Playwright runs two projects (npm test builds and packages the app first):
//   unit      tests/unit: pure checks of src/main/security.ts and the built index.html, no Electron
//   electron  tests/e2e: the packaged shell (out/packaged, scripts/package-dir.mjs) driven by _electron.launch
// One worker: each e2e file starts its own Electron, and the timings of a shared machine stay out of the way.
//
// F-892 (ADR 0789): launchStudio notes each Studio it launches in the run's registry file, named here for the workers,
// and the global teardown ends, and prints, a Studio or an `rcfd serve` the run left behind (tests/support/leftovers.ts).
import { defineConfig } from '@playwright/test';
import { tmpdir } from 'node:os';
import path from 'node:path';

// Headless by default: the Studio keeps its window hidden while tests drive it (src/main/index.ts, RCFD_TEST_HEADLESS).
// Set RCFD_TEST_HEADLESS=0 to watch the windows.
process.env.RCFD_TEST_HEADLESS ??= '1';
// the same name as REGISTRY_VARIABLE of tests/support/leftovers.ts (the workers inherit it from the runner)
process.env.RCFD_TEST_LAUNCH_REGISTRY ??= path.join(tmpdir(), `rcfd-studio-test-run-${process.pid}-${Date.now()}.jsonl`);

export default defineConfig({
  testDir: 'tests',
  outputDir: 'test-results',
  workers: 1,
  retries: 0,
  forbidOnly: true,
  reporter: [['list']],
  timeout: 60_000,
  globalTeardown: './tests/support/global-teardown.ts',
  projects: [
    { name: 'unit', testDir: 'tests/unit' },
    { name: 'electron', testDir: 'tests/e2e' },
  ],
});
