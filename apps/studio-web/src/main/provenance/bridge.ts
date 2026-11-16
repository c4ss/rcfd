// The main-process end of the provenance port (F-828, src/shared/provenance.ts): every message the renderer's preload
// script sends is checked here first. A request has exactly the fields of its operation; a run is a run id (RUN_ID), a
// text to copy is a string of at most COPY_MAX_CHARS characters with no NUL. `run` asks the open project's read-only
// project server (record.ts); `copy` puts the text on the system clipboard (the page itself has no clipboard
// permission: every permission is denied in the session, G10).
//
// No Electron import: the port and the clipboard are passed in (the unit tests use Node's MessageChannel and a fake;
// src/main/index.ts adapts Electron's MessagePortMain and clipboard).
import { COPY_MAX_CHARS, type ProvenanceError, type ProvenancePortMessage, type ProvenanceReply, type ProvenanceRequest } from '../../shared/provenance.js';
import { RUN_ID } from '../../shared/run.js';
import type { Query } from '../run/series.js';
import { readRunProvenance } from './record.js';

export interface ProvenancePortLike {
  postMessage(message: ProvenancePortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** Where the bridge asks: the open project (its query, folder and engine), or null when none is open. */
export interface ProvenanceSource {
  project(): { query: Query; folder: string; engine: { name: string; version: string } | null } | null;
  copy(text: string): void;
  now(): string;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

export type ParsedProvenance =
  | { readonly ok: true; readonly request: ProvenanceRequest }
  | { readonly ok: false; readonly id: number | null; readonly error: ProvenanceError };

/** Checks one message from the port. Anything not exactly a ProvenanceRequest is refused, with the reason. */
export function parseProvenanceRequest(raw: unknown): ParsedProvenance {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedProvenance => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const keys = Object.keys(raw).sort().join(',');
  switch (raw['op']) {
    case 'run': {
      if (keys !== 'id,op,run') return bad('run: takes exactly a run');
      const run = raw['run'];
      if (typeof run !== 'string' || !RUN_ID.test(run)) return bad('run: the run must be a run id (letters, digits, ".", "_" and "-", at most 128)');
      return { ok: true, request: { op: 'run', id, run } };
    }
    case 'copy': {
      if (keys !== 'id,op,text') return bad('copy: takes exactly a text');
      const text = raw['text'];
      if (typeof text !== 'string' || text.length === 0 || text.length > COPY_MAX_CHARS || text.includes('\u0000')) {
        return bad(`copy: the text must be a string of 1 to ${COPY_MAX_CHARS} characters without NUL`);
      }
      return { ok: true, request: { op: 'copy', id, text } };
    }
    default:
      return bad(`unknown op '${String(raw['op']).slice(0, 40)}'`);
  }
}

/** Serves one port: requests in, replies out. Ends when the port closes. */
export class ProvenancePortBridge {
  private disposed = false;
  /** Requests refused by parseProvenanceRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: ProvenancePortLike,
    private readonly source: ProvenanceSource,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private reply(id: number, reply: ProvenanceReply): void {
    if (this.disposed) return;
    try {
      this.port.postMessage({ type: 'reply', id, reply });
    } catch {
      this.dispose();
    }
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseProvenanceRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    if (r.op === 'copy') {
      try {
        this.source.copy(r.text);
        this.reply(r.id, { ok: true, result: { chars: r.text.length } });
      } catch (e) {
        this.reply(r.id, { ok: false, error: { code: 'clipboard', message: e instanceof Error ? e.message : String(e) } });
      }
      return;
    }
    const p = this.source.project();
    if (!p) {
      this.reply(r.id, { ok: false, error: { code: 'no_project', message: 'no project is open' } });
      return;
    }
    try {
      this.reply(r.id, { ok: true, result: await readRunProvenance(p.query, r.run, p.folder, p.engine, this.source.now) });
    } catch (e) {
      this.reply(r.id, { ok: false, error: { code: 'engine', message: e instanceof Error ? e.message : String(e) } });
    }
  }
}
