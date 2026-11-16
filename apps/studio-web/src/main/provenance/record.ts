// A run's provenance, read through the engine (F-828): runs/<id>/run.json as the runner wrote it, the engine's hash of
// runs/<id>/case.snapshot and of the project's case file now, and the units of runs/<id>/output/monitors.json, all with
// `read_project_file` of the open project's read-only project server (crates/rcfd-workspace, the tool Claude Code uses).
// The reproduce command is built from the record alone (src/shared/provenance.ts reproduceCommand); the Studio reads no
// file itself and writes none (G2, G12).
//
// No Electron import: the unit tests drive it with a fake query.
import path from 'node:path';
import {
  RECORD_LABELS,
  RECORD_MAX_BYTES,
  reproduceCommand,
  sameHash,
  type HashCheck,
  type Reproduce,
  type RunProvenance,
  type RunRecordFacts,
} from '../../shared/provenance.js';
import { readUnits, type Query } from '../run/series.js';

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const str = (v: unknown): string | null => (typeof v === 'string' ? v : null);
const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** A whole project file and the engine's hash of it, page by page (read_project_file pages at 32 000 bytes). */
export async function readWhole(query: Query, file: string, maxBytes = RECORD_MAX_BYTES): Promise<{ text: string; hash: string }> {
  let text = '';
  let hash = '';
  let offset: number | null = 0;
  for (let page = 0; offset !== null; page++) {
    const r = obj(await query('read_project_file', { path: file, offset }));
    const h = str(r['hash']) ?? '';
    if (page === 0) {
      hash = h;
      const size = typeof r['size'] === 'number' ? r['size'] : 0;
      if (size > maxBytes) throw new Error(`${file} is ${size} bytes, more than the Studio reads for provenance (${maxBytes})`);
    } else if (h !== hash) {
      throw new Error(`${file} changed while it was read`);
    }
    text += str(r['text']) ?? '';
    offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
  }
  return { text, hash };
}

/** The engine's hash of a project file (its first page; the hash is of the whole file). */
async function hashOf(query: Query, file: string): Promise<string> {
  const h = str(obj(await query('read_project_file', { path: file, offset: 0, limit: 1 }))['hash']);
  if (h === null) throw new Error(`the engine gave no hash for ${file}`);
  return h;
}

async function check(query: Query, file: string, recorded: string): Promise<HashCheck> {
  try {
    const hash = await hashOf(query, file);
    return { path: file, hash, recorded, matches: sameHash(hash, recorded), error: null };
  } catch (e) {
    return { path: file, hash: null, recorded, matches: null, error: message(e) };
  }
}

/** run.json's fields, verbatim; throws when it is not a run record. */
export function recordFacts(text: string, run: string): RunRecordFacts {
  let j: Obj;
  try {
    j = obj(JSON.parse(text));
  } catch {
    throw new Error(`runs/${run}/run.json is not JSON`);
  }
  // `case`, or `case_source` in records of the runner before it wrote `case` (crates/rcfd-project RunRecordWire)
  const kase = str(j['case']) ?? str(j['case_source']);
  const hash = str(j['case_snapshot_hash']);
  if (str(j['id']) !== run || kase === null || hash === null) throw new Error(`runs/${run}/run.json is not the record of run ${run}`);
  const labels: Record<string, string> = {};
  for (const k of RECORD_LABELS) {
    const v = str(j[k]);
    if (v !== null) labels[k] = v;
  }
  const mesh = str(j['mesh']);
  return {
    id: run,
    case: kase,
    caseSnapshotHash: hash,
    caseSource: str(j['case_source']),
    caseDir: str(j['case_dir']),
    solverVersion: str(j['solver_version']) ?? 'unknown',
    created: str(j['created']) ?? '',
    settings: obj(j['settings']),
    mesh: mesh === null ? null : { path: mesh, hash: str(j['mesh_hash']) },
    labels,
  };
}

/** Whether a recorded case path is inside the project (project-relative), as rcfd-runner's case_in_project writes it. */
const isRelative = (p: string): boolean => !path.isAbsolute(p) && !path.win32.isAbsolute(p) && !p.split(/[\\/]/).includes('..');

const NO_COMMAND: Reproduce = { argv: null, text: null, gaps: [], leftOut: [] };

/** Everything F-828 shows of a run of the open project. */
export async function readRunProvenance(
  query: Query,
  run: string,
  folder: string,
  engineNow: { name: string; version: string } | null,
  now: () => string,
): Promise<RunProvenance> {
  const recordPath = `runs/${run}/run.json`;
  let record: RunRecordFacts | null = null;
  let recordHash: string | null = null;
  let recordError: string | null = null;
  try {
    const whole = await readWhole(query, recordPath);
    recordHash = whole.hash;
    record = recordFacts(whole.text, run);
  } catch (e) {
    recordError = message(e);
  }
  const { units, note } = await readUnits(query, run);
  const base = { run, folder, recordPath, recordHash, record, recordError, engineNow, units: Object.fromEntries(units), unitsNote: note, readAt: now() };
  if (!record) {
    return { ...base, snapshotCheck: null, caseCheck: null, reproduce: { ...NO_COMMAND, gaps: [`There is no run record to build the command from: ${recordError}.`] } };
  }
  const snapshotCheck = await check(query, `runs/${run}/case.snapshot`, record.caseSnapshotHash);
  const inside = isRelative(record.case);
  const caseCheck = inside ? await check(query, record.case.split('\\').join('/'), record.caseSnapshotHash) : null;
  // the case the command names: the record's case in this project folder (rcfd-runner start reads a relative case
  // against its working folder, so the command names it whole), or the absolute path the record holds
  const casePath = inside ? path.join(folder, ...record.case.split(/[\\/]/)) : record.case;
  const reproduce = reproduceCommand({ record, casePath, folder, caseCheck, snapshotCheck, engineNow });
  return { ...base, snapshotCheck, caseCheck, reproduce };
}
