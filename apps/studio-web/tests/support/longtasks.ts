// F-832 (G21): recording the renderer main thread's long tasks and long animation frames in the packaged Studio, and the
// machine's load while they are measured. The recorder is a PerformanceObserver on `longtask` and
// `long-animation-frame` installed in the page; each entry is assigned to the phase (opening the project, opening a run,
// loading a field, scrubbing, ...) whose time range holds its start.
import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export interface LongEntry {
  readonly type: 'longtask' | 'long-animation-frame';
  readonly start: number;
  readonly duration: number;
  /** long-animation-frame only: the part of the frame that blocked input. */
  readonly blocking: number | null;
  /** The scripts a long frame ran (sourceURL, function, invoker, duration), the longest first. */
  readonly scripts: readonly { invoker: string; source: string; fn: string; duration: number }[];
  readonly attribution: readonly string[];
  /** long-animation-frame only: ms from the frame's start to its rendering, and to its style and layout. */
  readonly renderAt: number | null;
  readonly styleAt: number | null;
  phase?: string;
}

/** Installs the observers; returns the entry types this window supports (both, or the test would pass vacuously). */
export async function installRecorder(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const w = window as unknown as { __rt?: { phases: { name: string; at: number }[]; entries: unknown[]; supported: string[] } };
    if (w.__rt) return w.__rt.supported;
    const rt = { phases: [] as { name: string; at: number }[], entries: [] as unknown[], supported: [] as string[] };
    w.__rt = rt;
    for (const type of ['longtask', 'long-animation-frame']) {
      if (!PerformanceObserver.supportedEntryTypes.includes(type)) continue;
      new PerformanceObserver((list) => {
        for (const e of list.getEntries()) {
          const x = e as PerformanceEntry & {
            blockingDuration?: number;
            renderStart?: number;
            styleAndLayoutStart?: number;
            scripts?: { invoker?: string; sourceURL?: string; sourceFunctionName?: string; duration: number }[];
            attribution?: { name: string; containerType?: string }[];
          };
          rt.entries.push({
            type: e.entryType,
            start: e.startTime,
            duration: e.duration,
            blocking: typeof x.blockingDuration === 'number' ? x.blockingDuration : null,
            scripts: (x.scripts ?? [])
              .map((s) => ({ invoker: s.invoker ?? '', source: s.sourceURL ?? '', fn: s.sourceFunctionName ?? '', duration: s.duration }))
              .sort((a, b) => b.duration - a.duration)
              .slice(0, 5),
            attribution: (x.attribution ?? []).map((a) => `${a.name}:${a.containerType ?? ''}`),
            renderAt: typeof x.renderStart === 'number' && x.renderStart > 0 ? x.renderStart - e.startTime : null,
            styleAt: typeof x.styleAndLayoutStart === 'number' && x.styleAndLayoutStart > 0 ? x.styleAndLayoutStart - e.startTime : null,
          });
        }
      }).observe({ type, buffered: false });
      rt.supported.push(type);
    }
    return rt.supported;
  });
}

/** Starts a phase now (the entries that start from now on belong to it). */
export const phase = (page: Page, name: string): Promise<void> =>
  page.evaluate((n) => {
    (window as unknown as { __rt: { phases: { name: string; at: number }[] } }).__rt.phases.push({ name: n, at: performance.now() });
  }, name);

/** Every entry recorded so far, after two frames (the observers deliver asynchronously), each with its phase. */
export async function entries(page: Page): Promise<LongEntry[]> {
  const { phases, list } = await page.evaluate(async () => {
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    await new Promise((r) => setTimeout(r, 100));
    const rt = (window as unknown as { __rt: { phases: { name: string; at: number }[]; entries: LongEntry[] } }).__rt;
    return { phases: rt.phases, list: rt.entries };
  });
  for (const e of list) {
    let p = 'before';
    for (const ph of phases) if (ph.at <= e.start) p = ph.name;
    e.phase = p;
  }
  return list;
}

/** The share of the machine's processor time that was busy over one second, and the build processes running. */
export async function machineLoad(): Promise<{ at: string; cpuBusy: number; builders: number | null; freeMemGB: number }> {
  const times = () => {
    let busy = 0;
    let total = 0;
    for (const c of os.cpus()) {
      const t = c.times;
      busy += t.user + t.nice + t.sys + t.irq;
      total += t.user + t.nice + t.sys + t.irq + t.idle;
    }
    return { busy, total };
  };
  const a = times();
  await new Promise((r) => setTimeout(r, 1000));
  const b = times();
  let builders: number | null = null;
  if (process.platform === 'win32') {
    try {
      builders = execFileSync('tasklist', ['/fo', 'csv', '/nh'], { encoding: 'utf8' })
        .split('\n')
        .filter((l) => /^"(cargo|rustc|link|node|electron|rcfd-studio)\.exe"/i.test(l)).length;
    } catch {
      builders = null;
    }
  }
  return { at: new Date().toISOString(), cpuBusy: (b.busy - a.busy) / Math.max(1, b.total - a.total), builders, freeMemGB: os.freemem() / 2 ** 30 };
}

/** The run id each generated box gets in the bench project (the engine lists only folders named as run ids). */
export const benchRunId = (size: string): string => `r-20261116T000000-${size === 'tiny' ? '0001' : size === '1e5' ? '1e05' : size.replace('e', 'e0')}`;

/**
 * Makes `<root>/project` (written by `f447_bench gen`) a project the Studio opens, as scripts/measure-scrub.mjs does:
 * rcfd-project.json, and each box renamed to a run id with run.json, status.json and a residuals.jsonl of one line per
 * stored step. Returns the run folder of each size.
 */
export function benchProject(root: string, sizes: readonly string[]): Record<string, string> {
  const project = path.join(root, 'project');
  const now = new Date().toISOString();
  writeFileSync(path.join(project, 'rcfd-project.json'), JSON.stringify({ schema_version: 1, name: 'F-832 bench (f447_bench boxes)', created: now, solver_version_created: 'rcfd 0.1.0' }, null, 2) + '\n');
  const out: Record<string, string> = {};
  for (const size of sizes) {
    const id = benchRunId(size);
    const dir = path.join(project, 'runs', id);
    renameSync(path.join(project, 'runs', `box-${size}-raw`), dir);
    const index = JSON.parse(readFileSync(path.join(dir, 'fields', 'index.json'), 'utf8')) as { snapshots: { step: number }[] };
    const steps = index.snapshots.map((s) => s.step);
    const last = steps[steps.length - 1];
    writeFileSync(
      path.join(dir, 'run.json'),
      JSON.stringify({ id, case: `cases/box-${size}.toml`, case_snapshot_hash: 'fnv64:0000000000000000', solver_version: 'rcfd 0.1.0', settings: { mem_mb: null, priority: 0, set: [], threads: 1 }, created: now }, null, 2) + '\n',
    );
    writeFileSync(path.join(dir, 'status.json'), JSON.stringify({ state: 'finished', step: last, time: 0, pid: null, updated: now, reason: 'step limit (f447_bench gen)', exit_code: 0 }, null, 2) + '\n');
    writeFileSync(
      path.join(dir, 'residuals.jsonl'),
      steps.map((s) => JSON.stringify({ monitors: { probe: s / 1000 }, residuals: { continuity: 1 / s, p: 2 / s, u: 3 / s }, step: s, time: 0 })).join('\n') + '\n',
    );
    out[size] = dir;
  }
  return out;
}
