// The mesh preview (F-836, ADR 0732) without Electron: the engine's `mesh.preview` (the real `rcfd serve --stdio --ui`)
// writes a mesh-only store, the Studio's store reader (src/main/data/tsstore.ts) opens it and gives the patch of each
// triangle (M5's Studio side: the same mesh hash, cell and point counts, and the same triangles as the reader's plain
// surface, whose bits F-804's data.spec.ts checks against the Rust reader), and the pure helpers of
// src/renderer/case/preview.ts. Without --ui the method is the same -32601 as any unknown method.
//
// Needs rcfd (tests/support/rcfd.ts); without it the engine tests are skipped.
import { expect, test } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { parseDataRequest } from '../../src/main/data/bridge.js';
import { Store } from '../../src/main/data/tsstore.js';
import { isReply, patchRange, previewsDir, shownSurface, thinAxis, type PreviewReply } from '../../src/renderer/case/preview.js';
import { lutIndex } from '../../src/renderer/field/colormap.js';
import { appRoot, findRcfd, NO_RCFD } from '../support/rcfd.js';

const rcfd = findRcfd();
const repo = path.resolve(appRoot, '..', '..');

/** One `rcfd serve --stdio` (with `extra` switches) for the body of `f`. */
async function withService<T>(extra: string[], f: (c: RpcConnection) => Promise<T>): Promise<T> {
  const child = spawn(rcfd!, ['serve', '--stdio', ...extra], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
  const c = new RpcConnection(child.stdout, child.stdin, {}, () => child.stdin.end());
  try {
    return await f(c);
  } finally {
    c.close();
    await new Promise((r) => (child.exitCode !== null ? r(null) : child.once('exit', r)));
  }
}

const error = async (p: Promise<unknown>): Promise<{ code: number; message: string }> => {
  try {
    await p;
  } catch (e) {
    return e as { code: number; message: string };
  }
  throw new Error('the call succeeded');
};

test.describe('the engine writes it, the Studio reads it', () => {
  test.skip(rcfd === null, NO_RCFD);
  let dir = '';
  test.beforeAll(() => {
    dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f836-'));
  });
  test.afterAll(() => rmSync(dir, { recursive: true, force: true }));

  test('a 2D cavity: the store opens, its hash is the reply’s, and each patch has two triangles per face', async () => {
    const reply = await withService(['--ui'], async (c) => {
      const { session } = (await c.request('session.create', { path: path.join(repo, 'cases', 'cavity', 'adapt_re100.toml') })) as { session: string };
      return (await c.request('mesh.preview', { session, dir: previewsDir(dir) })) as PreviewReply;
    });
    expect(isReply(reply)).toBe(true);
    expect(reply.adapt).toEqual({ pending: true, cycles: 2, solves: 0 });
    expect(path.basename(reply.path)).toBe(reply.mesh_hash);
    expect(path.dirname(reply.path)).toBe(path.join(dir, '.rcfd', 'previews'));
    expect(readdirSync(path.join(dir, '.rcfd', 'previews'))).toEqual([reply.mesh_hash]);
    const store = new Store(reply.path);
    try {
      expect([store.meshHash, store.nCells, store.nPoints, store.snapshots.length]).toEqual([reply.mesh_hash, reply.cells, reply.points, 0]);
      const plain = store.surface();
      const withPatches = store.surface(true);
      // the patches add one array; the triangles are the plain surface's
      expect(Buffer.from(withPatches.points).equals(Buffer.from(plain.points))).toBe(true);
      expect(Buffer.from(withPatches.triangles).equals(Buffer.from(plain.triangles))).toBe(true);
      expect(Buffer.from(withPatches.cells).equals(Buffer.from(plain.cells))).toBe(true);
      expect(plain.patch).toBeUndefined();
      const patch = new Uint32Array(withPatches.patch!);
      const count = reply.patches.map((_, i) => patch.filter((p) => p === i).length);
      // cavity_2d: hexahedra, every boundary face a quadrilateral cut into two triangles
      expect(count).toEqual(reply.patches.map((p) => 2 * p.faces));
      expect([...patch].every((p, t) => t === 0 || p >= patch[t - 1]!)).toBe(true);
      // every patch has triangles; a 2D case has its Empty patch
      expect(count.every((n) => n > 0)).toBe(true);
      expect(reply.patches.some((p) => p.kind === 'Empty')).toBe(true);
      expect(reply.two_d).toBe(true);
    } finally {
      store.close();
    }
  });

  test('without --ui the method is unknown, like any other name', async () => {
    const [e, other] = await withService([], async (c) => {
      const { session } = (await c.request('session.create', {})) as { session: string };
      return [await error(c.request('mesh.preview', { session, dir: previewsDir(dir) })), await error(c.request('nothing.here', {}))];
    });
    expect(e.code).toBe(-32601);
    expect(e.code).toBe(other.code);
  });
});

test.describe('the pure part', () => {
  test('previewsDir is the project’s .rcfd/previews', () => {
    expect(previewsDir('C:\\p\\quick')).toBe('C:\\p\\quick\\.rcfd\\previews');
    expect(previewsDir('/p/quick/')).toBe('/p/quick/.rcfd/previews');
  });

  test('shownSurface keeps the triangles of the patches shown, valued by their patch', () => {
    const s = {
      points: new Float32Array(9),
      triangles: new Uint32Array([0, 1, 2, 1, 2, 0, 2, 0, 1, 0, 2, 1]),
      cells: new Uint32Array([5, 6, 7, 8]),
      patch: new Uint32Array([0, 0, 1, 2]),
      meshHash: '0000000000000000',
    };
    const all = shownSurface(s, new Set());
    expect([...all.values]).toEqual([0, 0, 1, 2]);
    expect([...all.surface.triangles]).toEqual([...s.triangles]);
    const some = shownSurface(s, new Set([0]));
    expect([...some.values]).toEqual([1, 2]);
    expect([...some.surface.triangles]).toEqual([2, 0, 1, 0, 2, 1]);
    expect([...some.surface.cells]).toEqual([7, 8]);
    expect(some.surface.points).toBe(s.points);
    expect(shownSurface(s, new Set([0, 1, 2])).values.length).toBe(0);
  });

  test('thinAxis is the axis of the smallest extent', () => {
    expect(thinAxis({ min: [0, 0, 0], max: [1, 1, 1e-5] })).toBe(2);
    expect(thinAxis({ min: [0, 0, 0], max: [1, 0.1, 1] })).toBe(1);
    expect(thinAxis({ min: [0, -1, -1], max: [0.01, 1, 1] })).toBe(0);
  });

  test('patchRange gives each patch its own colour', () => {
    for (const n of [1, 2, 3, 7, 12]) {
      const r = patchRange(n);
      const k = Array.from({ length: n }, (_, i) => lutIndex(i, r));
      expect(new Set(k).size).toBe(n);
    }
  });

  test('the data port takes patchSurface in the shape of surface', () => {
    const local = { route: 'local', path: process.platform === 'win32' ? 'C:\\p\\.rcfd\\previews\\x' : '/p/.rcfd/previews/x' };
    expect(parseDataRequest({ op: 'patchSurface', id: 3, store: local, tag: 'mesh-preview-s1' })).toEqual({
      ok: true,
      request: { op: 'patchSurface', id: 3, store: local, tag: 'mesh-preview-s1' },
    });
    for (const bad of [{ op: 'patchSurface', id: 1 }, { op: 'patchSurface', id: 1, store: local, snapshot: 0 }, { op: 'patchSurface', id: 1, store: { route: 'file', path: local.path } }]) {
      expect(parseDataRequest(bad).ok, JSON.stringify(bad)).toBe(false);
    }
  });
});
