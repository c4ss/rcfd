// What every e2e test passes to Playwright's _electron.launch besides the executable and the environment.
//
// chromiumSandbox: on Linux Playwright adds --no-sandbox to Electron's arguments unless this is true, so without it the
// Linux runs (CI's ubuntu job, the Docker run of docker/linux-test) tested the Studio with Chromium's OS sandbox off,
// against G10. On Windows and macOS Playwright never adds it, so this changes nothing there.
//
// RCFD_TEST_ELECTRON_ARGS (space-separated, default none): Chromium switches for the test environment, not the app, such
// as --ignore-gpu-blocklist to let a GPU-less Linux container draw WebGL 2 with Mesa's software rasteriser
// (docker/linux-test/run.mjs --gpu-blocklist=ignore; ADR 0801 Addendum 17).
export function launchArgs(args: readonly string[] = []): { chromiumSandbox: true; args: string[] } {
  const extra = (process.env['RCFD_TEST_ELECTRON_ARGS'] ?? '').split(' ').filter(Boolean);
  return { chromiumSandbox: true, args: [...extra, ...args] };
}
