// Opening a case file into the editor (F-812): the engine's hash of the file, a service session made from it, and the
// binding of the two in the main process (src/shared/case.ts).
import type { CaseBinding } from '../../shared/case.js';
import { dispatch } from '../actions/index.js';
import { caseDoor } from './door.js';

export interface OpenFailure {
  readonly message: string;
  /** The engine's located problems, when it refused the file as a case. */
  readonly diagnostics: readonly { readonly path: string; readonly line: number | null; readonly message: string }[];
  readonly code: string | number;
}

export type Opened = { readonly ok: true; readonly session: string; readonly binding: CaseBinding } | { readonly ok: false; readonly failure: OpenFailure };

const diags = (data: unknown): OpenFailure['diagnostics'] => {
  const d = (data as { diagnostics?: unknown } | null | undefined)?.diagnostics;
  return Array.isArray(d)
    ? d.map((x: { path?: unknown; line?: unknown; message?: unknown }) => ({
        path: typeof x?.path === 'string' ? x.path : '',
        line: typeof x?.line === 'number' ? x.line : null,
        message: String(x?.message ?? ''),
      }))
    : [];
};

/**
 * Opens a case file: its hash from the engine's project server (main), a service session made from the file
 * (`session.create {path}`, an action of the registry), then the binding (main checks the session's journal names the
 * file and the file did not change in between; when it did, the new session is closed and the open starts again).
 */
export async function openCase(path: string, attempts = 3): Promise<Opened> {
  let last: OpenFailure = { message: `${path} kept changing while it was being opened; open it again.`, diagnostics: [], code: 'changed' };
  for (let i = 0; i < attempts; i++) {
    const p = await caseDoor.prepare(path);
    if (!p.ok) return { ok: false, failure: { message: p.error.message, diagnostics: [], code: p.error.code } };
    // opening a file is not an edit: it stays out of the undo history (F-829); Close the session is the way back
    const c = await dispatch('session.create', { path: p.result.file }, { undo: false });
    if (!c.ok) return { ok: false, failure: { message: c.error.message, diagnostics: diags(c.error.data), code: c.error.code } };
    const session = (c.result as { session?: unknown }).session;
    if (typeof session !== 'string') return { ok: false, failure: { message: 'session.create answered without a session', diagnostics: [], code: 'protocol' } };
    const a = await caseDoor.attach(session, p.result.path, p.result.hash);
    if (a.ok) return { ok: true, session, binding: a.result };
    await dispatch('session.close', { session });
    last = { message: a.error.message, diagnostics: [], code: a.error.code };
    if (a.error.code !== 'changed') break;
  }
  return { ok: false, failure: last };
}

/**
 * ADR 0737: opens one case file outside a project: the main process's open dialog, a service session made from the
 * file (`session.create {path}`), and the binding, whose hash is the one the service's journal recorded when it read the
 * file. Null when the dialog was cancelled.
 */
export async function openCaseFile(): Promise<Opened | null> {
  const f = await caseDoor.pickFile();
  if (!f.ok) return { ok: false, failure: { message: f.error.message, diagnostics: [], code: f.error.code } };
  if (f.result === null) return null;
  return openPickedFile(f.result.file);
}

/** Opens a file the dialog gave again (loading it as it is now, after a conflict or an outside change). */
export async function openPickedFile(file: string): Promise<Opened> {
  const c = await dispatch('session.create', { path: file }, { undo: false });
  if (!c.ok) return { ok: false, failure: { message: c.error.message, diagnostics: diags(c.error.data), code: c.error.code } };
  const session = (c.result as { session?: unknown }).session;
  if (typeof session !== 'string') return { ok: false, failure: { message: 'session.create answered without a session', diagnostics: [], code: 'protocol' } };
  const a = await caseDoor.attachFile(session, file);
  if (a.ok) return { ok: true, session, binding: a.result };
  await dispatch('session.close', { session });
  return { ok: false, failure: { message: a.error.message, diagnostics: [], code: a.error.code } };
}
