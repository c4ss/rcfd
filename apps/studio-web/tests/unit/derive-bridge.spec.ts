// ADR 0738 (F-809, E-809d): the data port's `derive` operation, in Node. Main runs the engine's `rcfd post derive` on a
// local store inside a data root and hands back the GLB it wrote, byte for byte (the file the CLI writes, which the
// project tool `derive_picture` writes too, G16); the page's arguments are checked member by member before anything
// runs, and become the command's flags only.
//
// - parseDataRequest takes `derive` in its exact shape and refuses every other one, with the reason;
// - deriveFlags: the arguments as flags; deriveEngine: the chosen engine (locate.ts), else RCFD_STUDIO_RCFD or beside RCFD_STUDIO_MCP;
// - StudioData: the GLB is the CLI's own file of the same arguments; outside the data roots `not_allowed`; the service
//   route and a Studio without an engine refuse with the reason; the engine's errors keep their codes (1003, 1202);
//   a cancel answers at once.
//
// Needs the worktree's rcfd (tests/support/rcfd.ts: RCFD_BIN); without it the engine tests are skipped and say why.
import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { parseDataRequest, parseDeriveArgs } from '../../src/main/data/bridge.js';
import { deriveEngine, deriveFlags } from '../../src/main/data/derive.js';
import { DataRoots } from '../../src/main/data/roots.js';
import { StudioData } from '../../src/main/data/studio-data.js';
import { DataFailure } from '../../src/main/data/queue.js';
import type { DataWireResult, DeriveArgs } from '../../src/shared/data.js';
import { cutOf, readGlb } from '../../src/renderer/field/glb.js';
import { STORE_LENGTH_UNIT } from '../../src/renderer/field/viewExtras.js';
import { repoRoot } from '../support/project.js';
import { appRoot, findRcfd, NO_RCFD } from '../support/rcfd.js';

const WORKER = new URL(`file:///${path.join(appRoot, 'dist', 'main', 'data', 'worker.js').replace(/\\/g, '/')}`);
const rcfd = findRcfd();
const service = { state: { address: null, instance: 0 }, call: async () => ({}) } as never;

test('parseDataRequest: derive in its exact shape; every other shape refused with the reason', () => {
  const store = { route: 'local', path: 'C:\\x\\fields' };
  const good: DeriveArgs[] = [
    { kind: 'cut_surface' },
    { kind: 'cut_surface', axis: 2, at: 0.25, keep: 'above', vectors: true },
    { kind: 'iso', field: 'p', level: -3.5 },
    { kind: 'streamlines', rake: { p0: [0, 0, 0], p1: [1, 1, 0], n: 7 }, direction: 'both', maxSteps: 2000 },
  ];
  for (const args of good) {
    const r = parseDataRequest({ op: 'derive', id: 1, store, snapshot: 3, args, tag: 't' });
    expect(r.ok, JSON.stringify(args)).toBe(true);
    if (r.ok) expect(r.request).toEqual({ op: 'derive', id: 1, store, snapshot: 3, args, tag: 't' });
  }
  const bad: [unknown, RegExp][] = [
    [{ kind: 'volume' }, /args.kind must be one of cut_surface, iso, streamlines/],
    [{ kind: 'cut_surface', field: 'p' }, /unexpected 'field'/],
    [{ kind: 'cut_surface', axis: 1 }, /both axis and at/],
    [{ kind: 'cut_surface', axis: 3, at: 0 }, /axis must be 0, 1 or 2/],
    [{ kind: 'cut_surface', axis: 0, at: Number.NaN }, /at must be a finite number/],
    [{ kind: 'cut_surface', vectors: 'yes' }, /vectors must be a boolean/],
    [{ kind: 'iso', field: 'p' }, /field and a finite level/],
    [{ kind: 'iso', field: '--out', level: 0 }, /field must be a field name/],
    [{ kind: 'streamlines' }, /rake must be/],
    [{ kind: 'streamlines', rake: { p0: [0, 0], p1: [1, 1, 1], n: 2 } }, /p0 and p1 must be three finite numbers/],
    [{ kind: 'streamlines', rake: { p0: [0, 0, 0], p1: [1, 1, 1], n: 1001 } }, /n must be an integer from 1 to 1000/],
    [{ kind: 'streamlines', rake: { p0: [0, 0, 0], p1: [1, 1, 1], n: 2 }, maxSteps: 10_001 }, /maxSteps must be an integer from 1 to 10000/],
    [{ kind: 'streamlines', rake: { p0: [0, 0, 0], p1: [1, 1, 1], n: 2 }, direction: 'up' }, /direction must be forward, backward or both/],
    ['cut_surface', /args must be a plain object/],
  ];
  for (const [args, why] of bad) {
    expect(typeof parseDeriveArgs(args) === 'string' ? (parseDeriveArgs(args) as string) : 'accepted', JSON.stringify(args)).toMatch(why);
    const r = parseDataRequest({ op: 'derive', id: 2, store, snapshot: 0, args });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.code).toBe('bad_request');
  }
  for (const r of [
    parseDataRequest({ op: 'derive', id: 3, store, args: { kind: 'cut_surface' } }),
    parseDataRequest({ op: 'derive', id: 3, store, snapshot: -1, args: { kind: 'cut_surface' } }),
    parseDataRequest({ op: 'derive', id: 3, store, snapshot: 0, args: { kind: 'cut_surface' }, extra: 1 }),
  ])
    expect(r.ok).toBe(false);
});

test('deriveFlags and deriveEngine', () => {
  expect(deriveFlags(4, { kind: 'cut_surface', axis: 1, at: 0.5, keep: 'above', vectors: true })).toEqual([
    '--kind', 'cut_surface', '--snapshot', '4', '--axis', 'y', '--at', '0.5', '--keep', 'above', '--vectors',
  ]);
  expect(deriveFlags(0, { kind: 'iso', field: 'p', level: Math.fround(0.1) })).toEqual(['--kind', 'iso', '--snapshot', '0', '--field', 'p', '--level', String(Math.fround(0.1))]);
  expect(deriveFlags(1, { kind: 'streamlines', rake: { p0: [0, 0.5, 1], p1: [1, 0.5, 1], n: 3 }, direction: 'both', maxSteps: 9 })).toEqual([
    '--kind', 'streamlines', '--snapshot', '1', '--rake', '0,0.5,1,1,0.5,1,3', '--direction', 'both', '--max-steps', '9',
  ]);
  const exe = process.platform === 'win32' ? 'rcfd.exe' : 'rcfd';
  const files = new Set([path.join('E:', 'r', exe), path.join('E:', 'b', exe), path.join('E:', 'm', exe)]);
  const isFile = (p: string) => files.has(p);
  const vars = { from: 'variables' } as const;
  expect(deriveEngine({ RCFD_STUDIO_RCFD: path.join('E:', 'r', exe) }, isFile, vars)).toEqual({ ok: true, rcfd: path.join('E:', 'r', exe) });
  expect(deriveEngine({ RCFD_STUDIO_RCFD: path.join('E:', 'q', exe) }, isFile, vars).ok).toBe(false);
  expect(deriveEngine({ RCFD_STUDIO_MCP: path.join('E:', 'm', 'rcfd-mcp.exe') }, isFile, vars)).toEqual({ ok: true, rcfd: path.join('E:', 'm', exe) });
  // The engine the Studio chose (ADR 0801 Addendum 21): a complete folder gives its rcfd, an incomplete one its reason.
  expect(deriveEngine({}, isFile, { from: 'bundled', dir: path.join('E:', 'b'), missing: [] })).toEqual({ ok: true, rcfd: path.join('E:', 'b', exe) });
  const gap = deriveEngine({}, isFile, { from: 'checkout', dir: path.join('E:', 't'), missing: [{ name: 'rcfd', pkg: 'rcfd-cli' }] });
  expect(gap.ok).toBe(false);
  if (!gap.ok) expect(gap.reason).toMatch(/cargo build/);
  const none = deriveEngine({}, isFile, { from: 'no-engine' });
  expect(none.ok).toBe(false);
  if (!none.ok) expect(none.reason).toMatch(/--no-engine/);
});

test('StudioData derive: the CLI’s file, the data roots, the refusals and the engine’s codes', async () => {
  test.skip(rcfd === null, NO_RCFD);
  test.setTimeout(120_000);
  const root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-adr0738-bridge-'));
  try {
    const runs = path.join(repoRoot, 'example-projects', 'quick-start', 'runs');
    const store = path.join(root, 'qs', 'fields');
    cpSync(path.join(runs, readdirSync(runs).sort()[0]!, 'fields'), store, { recursive: true });
    const data = new StudioData(service, DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: root }), WORKER, { ok: true, rcfd: rcfd! });
    const ask = (args: DeriveArgs, p = store, route: 'local' | 'service' = 'local') =>
      data.run({ op: 'derive', id: 1, store: { route, path: p }, snapshot: 0, args }, 60_000);
    const args: DeriveArgs = { kind: 'cut_surface', axis: 0, at: 0.5, keep: 'below', vectors: true };
    const r = (await ask(args).promise) as Extract<DataWireResult, { kind: 'derive' }>;
    expect(r.kind).toBe('derive');
    const summary = JSON.parse(r.summary);
    // the engine's unit of the stored mesh is the one the Studio shows before a reply names it (ADR 0738 P5)
    expect(summary.length_unit).toBe(STORE_LENGTH_UNIT);
    expect(summary.snapshot.index).toBe(0);
    // the same file the CLI writes of the same arguments
    const out = path.join(root, 'cli.glb');
    execFileSync(rcfd!, ['post', 'derive', store, '--kind', 'cut_surface', '--snapshot', '0', '--axis', 'x', '--at', '0.5', '--keep', 'below', '--vectors', '--out', out]);
    expect(Buffer.from(r.glb).equals(readFileSync(out))).toBe(true);
    expect(cutOf(readGlb(r.glb)).arrowCount).toBeGreaterThan(0);
    // outside the data roots
    const outside = path.join(repoRoot, 'example-projects', 'quick-start', 'runs');
    await expect(ask(args, outside).promise).rejects.toMatchObject({ code: 'not_allowed' });
    // the service route
    await expect(ask(args, 'fields', 'service').promise).rejects.toMatchObject({ code: 'bad_request', message: /service route has no derived pictures/ });
    // no engine
    const none = new StudioData(service, DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: root }), WORKER);
    await expect(none.run({ op: 'derive', id: 1, store: { route: 'local', path: store }, snapshot: 0, args }, 1000).promise).rejects.toMatchObject({ code: 'not_configured' });
    // the engine's own codes: a level outside the field's range (1003, with the range), no such field (1202)
    const e1 = await ask({ kind: 'iso', field: 'p', level: 1e12 }).promise.catch((e: unknown) => e);
    expect(e1).toBeInstanceOf(DataFailure);
    expect((e1 as DataFailure).code).toBe(1003);
    expect(((e1 as DataFailure).data as { range: unknown }).range).toBeInstanceOf(Array);
    const e2 = await ask({ kind: 'iso', field: 'nope', level: 0 }).promise.catch((e: unknown) => e);
    expect((e2 as DataFailure).code).toBe(1202);
    // a cancel answers at once
    const j = ask({ kind: 'streamlines', rake: { p0: [0.1, 0.1, 0.0005], p1: [0.9, 0.9, 0.0005], n: 50 }, direction: 'both', maxSteps: 10_000 });
    j.cancel();
    await expect(j.promise).rejects.toMatchObject({ code: 'cancelled' });
    await data.close();
    await none.close();
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
