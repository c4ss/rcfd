// Starting the packaged Studio (out/packaged, scripts/package-dir.mjs) for the service tests, with the service chosen by
// environment variables (src/main/service/config.ts), and reading its service state from the page.
import { _electron as electron, type ElectronApplication, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { ServiceState } from '../../src/shared/service.js';
import { until } from './fake-service.js';
import { appRoot } from './rcfd.js';

export interface Studio {
  readonly app: ElectronApplication;
  readonly page: Page;
}

/** Launches the packaged Studio with these service variables (and none inherited from the test's own environment). */
export async function launchStudio(serviceEnv: Record<string, string>): Promise<Studio> {
  const { executable } = JSON.parse(readFileSync(path.join(appRoot, 'out', 'packaged.json'), 'utf8')) as { executable: string };
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
  const app = await electron.launch({ executablePath: executable, args: [], env: { ...env, ...serviceEnv } });
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  return { app, page };
}

export const serviceState = (page: Page): Promise<ServiceState> => page.evaluate(() => window.studioService.state());

/** Waits until the page's service state satisfies `ok`, and returns it. */
export function untilState(page: Page, ok: (s: ServiceState) => boolean, timeoutMs = 15_000, what = 'the service state'): Promise<ServiceState> {
  return until(async () => {
    const s = await serviceState(page);
    return ok(s) ? s : null;
  }, timeoutMs, what);
}

/** In the page: collect every state change and every event batch into window globals the test reads back. */
export async function recordInPage(page: Page, session?: string): Promise<void> {
  const r = await page.evaluate(async (sess) => {
    const w = window as unknown as { __states: unknown[]; __batches: unknown[] };
    w.__states = [];
    w.__batches = [];
    window.studioService.onState((s) => w.__states.push(s));
    return window.studioService.subscribe((b) => w.__batches.push(b), sess === undefined ? undefined : { session: sess });
  }, session);
  if (!r.ok) throw new Error(`subscribe failed: ${r.error.message}`);
}

export interface PageEvent {
  seq: number;
  session: string;
  kind: string;
  data: unknown;
  instance: number;
}

export const pageEvents = (page: Page): Promise<PageEvent[]> =>
  page.evaluate(() =>
    (window as unknown as { __batches: { instance: number; events: Omit<PageEvent, 'instance'>[] }[] }).__batches.flatMap((b) =>
      b.events.map((e) => ({ ...e, instance: b.instance })),
    ),
  );

export const pageStates = (page: Page): Promise<ServiceState[]> =>
  page.evaluate(() => (window as unknown as { __states: ServiceState[] }).__states);

/**
 * Quits a Studio whose own service is left running (it holds sessions, G7), without waiting for that service.
 *
 * On Windows a child process inherits every inheritable handle of its parent. Playwright starts the Studio with pipes
 * as its standard streams, so the service the Studio starts holds those pipes open too, and Playwright's `close()`
 * (which waits for the pipes to close) would wait until the service ends. This waits for the Studio's own process to
 * end instead and returns `close()`'s promise, to be awaited after the service is stopped.
 */
export async function quitLeavingService(studio: Studio): Promise<{ closed: Promise<void> }> {
  const pid = studio.app.process().pid!;
  const closed = studio.app.close();
  await until(() => !processAlive(pid), 15_000, 'the Studio process to end');
  return { closed };
}

function processAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === 'EPERM';
  }
}
