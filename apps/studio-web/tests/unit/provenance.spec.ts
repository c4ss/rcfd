// The provenance inspector (F-828) without a window: the reproduce command built from a run record alone
// (src/shared/provenance.ts), with every gap said in words; the reading of a run's record, its case.snapshot and its
// case file through the engine's read_project_file (src/main/provenance/record.ts); the provenance port's checks; a
// number's facts and its unit as the engine gave it; the palette commands and the shortcut; the one renderer module
// that names window.studioProvenance; the stylesheet's tokens.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import { MessageChannel } from 'node:worker_threads';
import path from 'node:path';
import {
  COPY_MAX_CHARS,
  howItRanFacts,
  NO_LAUNCH_RECORD,
  numberFacts,
  PROVENANCE_OPS,
  provenanceText,
  reproduceCommand,
  rerunCommand,
  runFacts,
  sameHash,
  SESSION_FILE_NOT_RECORDED,
  SESSION_NO_REPRODUCE,
  sessionFacts,
  shellLine,
  unitText,
  type HashCheck,
  type NumberSource,
  type ProvenancePortMessage,
  type RunRecordFacts,
} from '../../src/shared/provenance.js';
import { commandLine } from '../../src/shared/actions/runs.js';
import { sessionSeriesValue, settingValue, snapshotFactValue } from '../../src/renderer/provenance/sources.js';
import { parseProvenanceRequest, ProvenancePortBridge, type ProvenancePortLike } from '../../src/main/provenance/bridge.js';
import { launchFacts, readRunProvenance, recordFacts } from '../../src/main/provenance/record.js';
import type { Query } from '../../src/main/run/series.js';
import { STUDIO_COMMANDS } from '../../src/shared/commands/studio.js';
import { paletteModel } from '../../src/shared/commands/index.js';
import { globalCommandFor, SHORTCUTS } from '../../src/shared/commands/shortcuts.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';
import { until } from '../support/fake-service.js';

const record = (o: Partial<RunRecordFacts> = {}): RunRecordFacts => ({
  id: 'r-20261116T100000-abcd',
  case: 'cases/cavity-re100.toml',
  caseSnapshotHash: 'fnv64:00000000000000aa',
  caseSource: '/old/place/cases/cavity-re100.toml',
  caseDir: '/old/place/cases',
  solverVersion: 'rcfd 0.1.0',
  created: '2026-11-16T10:00:00.000Z',
  settings: { mem_mb: null, priority: 0, set: [], threads: 1 },
  mesh: null,
  labels: {},
  rerunOf: null,
  ...o,
});
const same = (p: string): HashCheck => ({ path: p, hash: 'fnv64:00000000000000aa', recorded: 'fnv64:00000000000000aa', matches: true, error: null });
const engine = { name: 'rcfd', version: '0.1.0' };
const input = (o: Partial<Parameters<typeof reproduceCommand>[0]> = {}) => ({
  record: record(),
  casePath: '/p/quick/cases/cavity-re100.toml',
  folder: '/p/quick',
  caseCheck: same('cases/cavity-re100.toml'),
  snapshotCheck: same('runs/r-20261116T100000-abcd/case.snapshot'),
  engineNow: engine,
  ...o,
});

// ---- the reproduce command

test('the command is rcfd-runner start with the recorded case and every recorded setting, in order, and nothing else', () => {
  const r = reproduceCommand(input());
  expect(r.argv).toEqual(['rcfd-runner', 'start', '/p/quick/cases/cavity-re100.toml', '--project', '/p/quick', '--threads', '1', '--priority', '0']);
  expect(r.gaps).toEqual([]);
  expect(r.text).toBe('rcfd-runner start /p/quick/cases/cavity-re100.toml --project /p/quick --threads 1 --priority 0');
  const full = reproduceCommand(
    input({ record: record({ settings: { threads: 4, priority: -2, mem_mb: 512, set: ['output.store_interval=5', 'solver.max_iterations=50', 'solver.max_iterations=60'] } }) }),
  );
  expect(full.argv!.slice(5)).toEqual([
    '--threads',
    '4',
    '--priority',
    '-2',
    '--mem-mb',
    '512',
    '--set',
    'output.store_interval=5',
    '--set',
    'solver.max_iterations=50',
    '--set',
    'solver.max_iterations=60',
  ]);
  expect(full.gaps).toEqual([]);
  // the record's defaults when a key is missing, as the runner reads RunSettings (threads 1, priority 0)
  expect(reproduceCommand(input({ record: record({ settings: {} }) })).argv!.slice(5)).toEqual(['--threads', '1', '--priority', '0']);
});

test('the shell line quotes what needs it (Windows paths, spaces, quotes) the way the run actions do', () => {
  const argv = ['rcfd-runner', 'start', 'C:\\Users\\a b\\quick\\cases\\x.toml', '--project', 'C:\\Users\\a b\\quick', '--set', "title=it's"];
  expect(shellLine(argv)).toBe(commandLine(argv));
  expect(shellLine(argv)).toBe("rcfd-runner start 'C:\\Users\\a b\\quick\\cases\\x.toml' --project 'C:\\Users\\a b\\quick' --set 'title=it'\\''s'");
});

test('every fact the command cannot carry, or that would change the result, is a gap in words', () => {
  // ADR 0734 P3: continue_from and max_steps are options of rcfd-runner start now, so no gap
  const launch = reproduceCommand(input({ record: record({ settings: { threads: 1, priority: 0, set: [], continue_from: 'r-20261116T090000-0001', max_steps: 10 } }) }));
  expect(launch.gaps).toEqual([]);
  expect(launch.argv!.slice(5)).toEqual(['--threads', '1', '--priority', '0', '--continue-from', 'r-20261116T090000-0001', '--max-steps', '10']);
  expect(reproduceCommand(input({ record: record({ settings: { max_steps: 0 } }) })).gaps[0]).toContain('settings.max_steps is 0');
  // a key the runner has no option for is still a gap
  expect(reproduceCommand(input({ record: record({ settings: { threads: 1, later_key: 3 } }) })).gaps[0]).toContain('settings.later_key is 3; rcfd-runner start has no option for it');
  // null launch settings are no gap (the runner writes none)
  expect(reproduceCommand(input({ record: record({ settings: { threads: 1, continue_from: null } }) })).gaps).toEqual([]);
  const changed = reproduceCommand(input({ caseCheck: { ...same('cases/cavity-re100.toml'), hash: 'fnv64:00000000000000bb', matches: false } }));
  expect(changed.gaps).toEqual([
    'cases/cavity-re100.toml changed since the run (hash fnv64:00000000000000bb now, fnv64:00000000000000aa when the run was made); the command runs the case as it is now.',
  ]);
  expect(reproduceCommand(input({ caseCheck: { ...same('cases/x.toml'), hash: null, matches: null, error: 'no such file' } })).gaps[0]).toContain('cannot be read now (no such file)');
  expect(reproduceCommand(input({ caseCheck: null })).gaps[0]).toContain('outside the project folder');
  expect(reproduceCommand(input({ snapshotCheck: { ...same('runs/r/case.snapshot'), hash: 'fnv64:1', matches: false } })).gaps[0]).toContain('does not match the hash in run.json');
  expect(reproduceCommand(input({ engineNow: { name: 'rcfd', version: '0.2.0' } })).gaps).toEqual([
    'The run was made by rcfd 0.1.0; the engine here is rcfd 0.2.0, which may compute other values.',
  ]);
  expect(reproduceCommand(input({ record: record({ solverVersion: 'unknown' }) })).gaps).toEqual(['run.json does not say which engine version made the run.']);
  expect(reproduceCommand(input({ record: record({ settings: { threads: 0 } }) })).gaps[0]).toContain('settings.threads is 0');
  expect(reproduceCommand(input({ record: record({ settings: { set: 'a=1' } }) })).gaps[0]).toContain('settings.set is "a=1"');
  // ADR 0734 P4, P5: a recorded mesh is checked by its hash now; only a changed, unhashed or unreadable mesh is a gap
  const mesh = record({ mesh: { path: 'meshes/a.msh', hash: 'fnv64:0000000000000002' } });
  const meshSame: HashCheck = { path: 'meshes/a.msh', hash: 'fnv64:0000000000000002', recorded: 'fnv64:0000000000000002', matches: true, error: null };
  expect(reproduceCommand(input({ record: mesh, meshCheck: meshSame })).gaps).toEqual([]);
  expect(reproduceCommand(input({ record: mesh, meshCheck: { ...meshSame, hash: 'fnv64:0000000000000003', matches: false } })).gaps).toEqual([
    'The mesh meshes/a.msh changed since the run (hash fnv64:0000000000000003 now, fnv64:0000000000000002 when the run was made).',
  ]);
  expect(reproduceCommand(input({ record: mesh, meshCheck: null })).gaps[0]).toContain('outside the project folder');
  expect(reproduceCommand(input({ record: record({ mesh: { path: 'meshes/a.msh', hash: null } }) })).gaps[0]).toContain('records no hash of it');
  // the rerun command: the run's own snapshot, so a changed case is no gap; a changed mesh or engine is
  const rerun = rerunCommand({ run: 'r-20261116T100000-abcd', folder: '/p/quick', record: mesh, snapshotCheck: same('runs/r/case.snapshot'), meshCheck: meshSame, engineNow: engine });
  expect(rerun.argv).toEqual(['rcfd-runner', 'rerun', 'r-20261116T100000-abcd', '--project', '/p/quick']);
  expect(rerun.gaps).toEqual([]);
  expect(rerunCommand({ run: 'r', folder: '/p', record: mesh, snapshotCheck: null, meshCheck: { ...meshSame, matches: false }, engineNow: { name: 'rcfd', version: '0.2.0' } }).gaps).toHaveLength(2);
  // a design study's labels do not change the result: left out, not a gap
  const labelled = reproduceCommand(input({ record: record({ labels: { study: 'designs/s1', variant: 'a=1' } }) }));
  expect(labelled.gaps).toEqual([]);
  expect(labelled.leftOut).toEqual(['study designs/s1', 'variant a=1']);
});

test('the runner’s legacy hash spelling names the same bytes', () => {
  expect(sameHash('fnv1a64:00000000000000aa', 'fnv64:00000000000000aa')).toBe(true);
  expect(sameHash('fnv64:00000000000000aa', 'fnv64:00000000000000aa')).toBe(true);
  expect(sameHash('fnv64:00000000000000aa', 'fnv64:00000000000000ab')).toBe(false);
});

// ---- reading a run's provenance through the engine

/** A fake project server: read_project_file of a few files, in pages of `page` characters, recording what was asked. */
function fakeServer(files: Record<string, { text: string; hash: string }>, page = 10, status: Record<string, unknown> | null = null) {
  const calls: { tool: string; args: Record<string, unknown> }[] = [];
  const query: Query = async (tool, args) => {
    calls.push({ tool, args });
    // ADR 0734: run_status's launch and time_unit, and hash_project_file of a mesh (the hash of the file's entry)
    if (tool === 'run_status') {
      if (!status) throw new Error('run_status: no such run');
      return status;
    }
    if (tool === 'hash_project_file') {
      const f = files[args['path'] as string];
      if (!f) throw new Error(`${String(args['path'])}: no such file or folder`);
      return { path: args['path'], kind: 'file', hash: f.hash, bytes: f.text.length, files: 1 };
    }
    if (tool !== 'read_project_file') throw new Error(`unexpected tool ${tool}`);
    const f = files[args['path'] as string];
    if (!f) throw new Error(`read ${String(args['path'])}: The system cannot find the file specified. (os error 2)`);
    const offset = (args['offset'] as number) ?? 0;
    const limit = Math.min((args['limit'] as number | undefined) ?? page, page);
    const end = Math.min(f.text.length, offset + limit);
    return { path: args['path'], hash: f.hash, size: f.text.length, offset, text: f.text.slice(offset, end), next_offset: end < f.text.length ? end : null };
  };
  return { query, calls };
}

const RUN = 'r-20261116T100000-abcd';
const RUN_JSON = JSON.stringify({
  id: RUN,
  case: 'cases/cavity-re100.toml',
  case_snapshot_hash: 'fnv64:00000000000000aa',
  solver_version: 'rcfd 0.1.0',
  settings: { mem_mb: null, priority: 0, set: ['output.store_interval=5'], threads: 1 },
  created: '2026-11-16T10:00:00.000Z',
  case_source: '/old/place/cases/cavity-re100.toml',
  case_dir: '/old/place/cases',
});
const MONITORS = JSON.stringify({ format: 'rcfd-monitors', version: 1, series: { lid_drag: { unit: 'N', x: [1], y: [2] }, res_u: { unit: '', x: [1], y: [1] } } });

test('a run’s provenance: run.json whole through read_project_file, the hashes of case.snapshot and of the case now, the units', async () => {
  const folder = path.join(path.sep, 'p', 'quick');
  const s = fakeServer({
    [`runs/${RUN}/run.json`]: { text: RUN_JSON, hash: 'fnv64:00000000000000r1' },
    [`runs/${RUN}/case.snapshot`]: { text: 'title = "x"', hash: 'fnv64:00000000000000aa' },
    'cases/cavity-re100.toml': { text: 'title = "x"', hash: 'fnv64:00000000000000aa' },
    [`runs/${RUN}/output/monitors.json`]: { text: MONITORS, hash: 'm' },
  });
  const p = await readRunProvenance(s.query, RUN, folder, engine, () => 'now');
  expect(p.record).toMatchObject({ id: RUN, case: 'cases/cavity-re100.toml', solverVersion: 'rcfd 0.1.0', caseDir: '/old/place/cases' });
  expect(p.recordHash).toBe('fnv64:00000000000000r1');
  expect(p.snapshotCheck).toMatchObject({ matches: true });
  expect(p.caseCheck).toMatchObject({ path: 'cases/cavity-re100.toml', matches: true });
  expect(p.units).toEqual({ lid_drag: 'N', res_u: '' });
  expect(p.reproduce.argv).toEqual(['rcfd-runner', 'start', path.join(folder, 'cases', 'cavity-re100.toml'), '--project', folder, '--threads', '1', '--priority', '0', '--set', 'output.store_interval=5']);
  expect(p.reproduce.gaps).toEqual([]);
  // ADR 0734: no launch record from this fake (run_status failed), said; the rerun command from the record alone
  expect(p.launch).toBeNull();
  expect(p.launchNote).toContain('Not read: run_status of');
  expect(p.rerun.argv).toEqual(['rcfd-runner', 'rerun', RUN, '--project', folder]);
  // only read-only tools, never a write, and the record read whole page by page
  expect(new Set(s.calls.map((c) => c.tool))).toEqual(new Set(['read_project_file', 'run_status']));
  expect(s.calls.filter((c) => c.args['path'] === `runs/${RUN}/run.json`).length).toBe(Math.ceil(RUN_JSON.length / 10));
});

test('a changed case, a missing record and a record of another run are said, never filled in', async () => {
  const changed = fakeServer({
    [`runs/${RUN}/run.json`]: { text: RUN_JSON, hash: 'r' },
    [`runs/${RUN}/case.snapshot`]: { text: 'a', hash: 'fnv64:00000000000000aa' },
    'cases/cavity-re100.toml': { text: 'b', hash: 'fnv64:00000000000000bb' },
  });
  const p = await readRunProvenance(changed.query, RUN, '/p', engine, () => 'now');
  expect(p.reproduce.gaps[0]).toContain('cases/cavity-re100.toml changed since the run');
  expect(p.unitsNote).toContain('the runner writes it when a run ends');
  const none = await readRunProvenance(fakeServer({}).query, RUN, '/p', engine, () => 'now');
  expect(none.record).toBeNull();
  expect(none.recordError).toContain('os error 2');
  expect(none.reproduce.argv).toBeNull();
  expect(none.reproduce.gaps[0]).toContain('There is no run record to build the command from');
  expect(() => recordFacts(RUN_JSON, 'r-other')).toThrow(/not the record of run r-other/);
  expect(() => recordFacts('{', RUN)).toThrow(/not JSON/);
  // the runner's older records name the case in case_source only
  const legacy = recordFacts(JSON.stringify({ id: RUN, case_source: 'C:\\x\\cases\\a.toml', case_snapshot_hash: 'fnv1a64:01', settings: {} }), RUN);
  expect(legacy.case).toBe('C:\\x\\cases\\a.toml');
  expect(legacy.solverVersion).toBe('unknown');
  const outside = await readRunProvenance(
    fakeServer({ [`runs/${RUN}/run.json`]: { text: JSON.stringify({ ...JSON.parse(RUN_JSON), case: 'C:\\elsewhere\\a.toml' }), hash: 'r' } }).query,
    RUN,
    'C:\\p',
    engine,
    () => 'now',
  );
  expect(outside.caseCheck).toBeNull();
  expect(outside.reproduce.argv![2]).toBe('C:\\elsewhere\\a.toml');
  expect(outside.reproduce.gaps.some((g) => g.includes('outside the project folder'))).toBe(true);
});

// ---- the port

test('the provenance port takes exactly a run id, or a text to copy of at most COPY_MAX_CHARS', () => {
  expect(PROVENANCE_OPS).toEqual(['run', 'copy']);
  expect(parseProvenanceRequest({ id: 1, op: 'run', run: RUN })).toEqual({ ok: true, request: { id: 1, op: 'run', run: RUN } });
  expect(parseProvenanceRequest({ id: 2, op: 'copy', text: 'rcfd-runner start' })).toEqual({ ok: true, request: { id: 2, op: 'copy', text: 'rcfd-runner start' } });
  const refused = [
    null,
    [],
    { op: 'run', run: RUN },
    { id: 1, op: 'run', run: '../x' },
    { id: 1, op: 'run', run: RUN, path: 'runs/x' },
    { id: 1, op: 'copy', text: '' },
    { id: 1, op: 'copy', text: 'x'.repeat(COPY_MAX_CHARS + 1) },
    { id: 1, op: 'copy', text: 'a\u0000b' },
    { id: 1, op: 'copy', text: 3 },
    { id: 1, op: 'read_project_file', path: 'run.json' },
  ];
  for (const r of refused) expect(parseProvenanceRequest(r).ok, JSON.stringify(r)).toBe(false);
});

test('the bridge answers on its port, copies through the clipboard it is given, and says when no project is open', async () => {
  const ch = new MessageChannel();
  const adapt = (p: typeof ch.port1): ProvenancePortLike => ({
    postMessage: (m) => p.postMessage(m),
    onMessage: (l) => p.on('message', l),
    onClose: (l) => p.on('close', l),
    close: () => p.close(),
  });
  let open = true;
  const copied: string[] = [];
  const s = fakeServer({ [`runs/${RUN}/run.json`]: { text: RUN_JSON, hash: 'r' } });
  const bridge = new ProvenancePortBridge(adapt(ch.port1), {
    project: () => (open ? { query: s.query, folder: '/p', engine } : null),
    copy: (t) => copied.push(t),
    now: () => 'now',
  });
  const got: ProvenancePortMessage[] = [];
  ch.port2.on('message', (m: ProvenancePortMessage) => got.push(m));
  ch.port2.postMessage({ id: 1, op: 'run', run: RUN });
  ch.port2.postMessage({ id: 2, op: 'copy', text: 'hello' });
  ch.port2.postMessage({ id: 3, op: 'run', run: '../x' });
  await until(() => (got.length >= 3 ? true : null), 5_000, 'three replies');
  const byId = new Map(got.map((m) => [m.id, m.reply]));
  expect(byId.get(1)).toMatchObject({ ok: true, result: { run: RUN, record: { id: RUN } } });
  expect(byId.get(2)).toEqual({ ok: true, result: { chars: 5 } });
  expect(copied).toEqual(['hello']);
  expect(byId.get(3)).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  expect(bridge.refused).toBe(1);
  open = false;
  ch.port2.postMessage({ id: 4, op: 'run', run: RUN });
  await until(() => (got.length >= 4 ? true : null), 5_000, 'the fourth reply');
  expect(got[3]!.reply).toMatchObject({ ok: false, error: { code: 'no_project' } });
  bridge.dispose();
  ch.port2.close();
});

// ---- a number's facts

const source = (o: Partial<NumberSource> = {}): NumberSource => ({
  quantity: 'lid_drag',
  shown: '-0.008166',
  exact: '-0.008166479886055894',
  unit: { kind: 'monitors', name: 'lid_drag' },
  run: RUN,
  snapshot: { index: 23, of: 24, step: 121, time: null },
  step: 121,
  file: `runs/${RUN}/residuals.jsonl`,
  readBy: 'read_residuals',
  ...o,
});

test('a number’s facts: run, snapshot, file, the engine that made it and the unit exactly as the engine wrote it', async () => {
  const s = fakeServer({ [`runs/${RUN}/run.json`]: { text: RUN_JSON, hash: 'r' }, [`runs/${RUN}/output/monitors.json`]: { text: MONITORS, hash: 'm' } });
  const run = await readRunProvenance(s.query, RUN, '/p', engine, () => 'now');
  const facts = new Map(numberFacts(source(), run));
  expect(facts.get('Value')).toBe('-0.008166 (-0.008166479886055894)');
  expect(facts.get('Unit')).toBe(`N, runs/${RUN}/output/monitors.json`);
  expect(facts.get('Run')).toBe(RUN);
  expect(facts.get('Snapshot')).toBe('24 of 24, step 121');
  expect(facts.get('File')).toBe(`runs/${RUN}/residuals.jsonl`);
  expect(facts.get('Engine')).toBe('rcfd 0.1.0');
  // the engine's empty unit stays empty; a series without a unit says why; nothing is converted or guessed
  expect(unitText(source({ unit: { kind: 'monitors', name: 'res_u' } }), run)).toBe(`none, empty in runs/${RUN}/output/monitors.json`);
  expect(unitText(source({ unit: { kind: 'monitors', name: 'q_hot' } }), run)).toBe(`none: runs/${RUN}/output/monitors.json has no series q_hot`);
  expect(unitText(source({ unit: { kind: 'engine', unit: 'Pa', from: 'store.rts' } }), null)).toBe('Pa, store.rts');
  expect(unitText(source({ unit: { kind: 'none', why: 'a count' } }), null)).toBe('none: a count');
  expect(new Map(numberFacts(source(), null)).get('Engine')).toBe('reading…');
  expect(new Map(numberFacts(source({ snapshot: { none: 'none: a residual sample' }, step: 40 }), null)).get('Step')).toBe('40');
  const text = provenanceText('Provenance of lid_drag', numberFacts(source(), run), run);
  expect(text).toContain('Engine: rcfd 0.1.0');
  expect(text).toContain(`Record: runs/${RUN}/run.json (r)`);
  expect(text).toContain('Reproduce: rcfd-runner start');
  expect(new Map(runFacts(run)).get('Settings')).toBe('{"mem_mb":null,"priority":0,"set":["output.store_interval=5"],"threads":1}');
});

// ---- commands, shortcut, door and stylesheet

test('Show provenance and Copy reproduce command are commands of the palette; Ctrl+I shows provenance anywhere', () => {
  const ids = new Set(paletteModel().map((e) => e.id));
  for (const id of ['provenance.show', 'provenance.copy_command']) {
    expect(ids.has(id), id).toBe(true);
    expect(STUDIO_COMMANDS.find((c) => c.id === id)!.cli).toContain('rcfd-runner start');
  }
  expect(SHORTCUTS.find((s) => s.owner === 'F-828')).toMatchObject({ keys: 'Ctrl+I', scope: 'global', command: 'provenance.show' });
  expect(globalCommandFor({ key: 'i', code: 'KeyI', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false })).toBe('provenance.show');
});

const renderer = path.join(appRoot, 'src', 'renderer');
function sources(dir: string): { file: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((d) => {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) return sources(p);
    if (!/\.(ts|tsx|js|jsx|mts|cts)$/.test(d.name) || d.name.endsWith('.d.ts')) return [];
    return [{ file: path.relative(renderer, p).split(path.sep).join('/'), text: readFileSync(p, 'utf8') }];
  });
}
const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');

test('only provenance/door.ts of the renderer names window.studioProvenance', () => {
  expect(sources(renderer).filter((f) => /\bstudioProvenance\b/.test(code(f.text))).map((f) => f.file)).toEqual(['provenance/door.ts']);
});

test('the numbers of the time panel, the workspace, Runs and the field readout are wrapped in a provenance trigger', () => {
  const read = (f: string) => code(readFileSync(path.join(renderer, f), 'utf8'));
  const count = (t: string) => (t.match(/<Provenance\b/g) ?? []).length;
  // residual values and monitor readout (one helper), snapshot counter
  expect(count(read('workspace/TimePanel.tsx'))).toBeGreaterThanOrEqual(2);
  expect(read('workspace/TimePanel.tsx')).toMatch(/className="tp-resid-value">\s*<ProvenanceOr/);
  // the number grid (4) and the run id of the list
  expect(count(read('workspace/RunWorkspace.tsx'))).toBe(5);
  // the table's result, the selected run's id, steps, time, solver time, process, exit code, report values
  expect(count(read('runs/RunsView.tsx'))).toBe(8);
  // the readout; the caption's step and time, and the learner caption's round (F-813)
  expect(count(read('field/FieldViewport.tsx'))).toBe(4);
  // F-828: the numbers of a service session (no run behind them): a setting, the plots' latest values, the iteration
  expect(count(read('settings/KeyRow.tsx'))).toBe(1);
  expect(read('settings/KeyRow.tsx')).toContain("kind: 'session'");
  expect(count(read('plots/ResidualPlot.tsx'))).toBe(1);
  expect(count(read('plots/MonitorPlot.tsx'))).toBe(1);
  expect(count(read('dev/PlotsDev.tsx'))).toBe(1);
  // the first path's result: rounds, solver time, the report values (F-813)
  expect(count(read('first/FirstPath.tsx'))).toBe(3);
});

test('a session number’s facts: the service’s own, and what it does not record said as not recorded', () => {
  const src = settingValue({
    session: 's3',
    path: 'boundaries.lid.velocity',
    shown: '[1, 0, 0]',
    value: [1, 0, 0],
    pattern: 'boundaries.*.velocity',
    unit: { unit: 'm s^-1', kind: 'x-rcfd-si-dimension' },
    set: true,
  });
  const rows = new Map(sessionFacts(src, { name: 'rcfd', version: '0.1.0' }, { entries: 2, caseHash: '00ff' }));
  expect(rows.get('Unit')).toBe('m s^-1, settings.schema, x-rcfd-si-dimension of boundaries.*.velocity');
  expect(rows.get('Session')).toBe('s3');
  expect(rows.get('Case file')).toBe(SESSION_FILE_NOT_RECORDED);
  expect(rows.get('Journal')).toBe('2 edits since the session was made, case hash 00ff (journal.get)');
  expect(rows.get('Engine')).toBe('rcfd 0.1.0 (service.info)');
  expect(rows.get('Iteration')).toMatch(/^none: /);
  const none = new Map(sessionFacts(settingValue({ ...{ session: 's3', path: 'solver.alpha_u', shown: '0.9', value: 0.9, pattern: 'solver.alpha_u', set: false }, unit: null }), null, { error: 'no session' }));
  expect(none.get('Unit')).toBe('none: the case schema names no unit for solver.alpha_u');
  expect(none.get('Journal')).toBe('not read: no session');
  expect(none.get('Engine')).toBe('not known: no service answered service.info');
  const series = sessionSeriesValue({ session: 's1', name: 'res_u', shown: '1.2e-3', value: 0.0012, x: 40, xLabel: 'iteration', unit: '', residual: true });
  expect(new Map(sessionFacts(series, 'reading', null)).get('Unit')).toBe('none, empty in monitors.get (the series’ unit)');
  expect(new Map(sessionFacts(series, 'reading', null)).get('Iteration')).toBe('40');
  expect(SESSION_NO_REPRODUCE).toMatch(/^Not recorded: /);
  // a snapshot's time: ADR 0734 P2, the unit run_status names (store.rts keeps its bytes; G2: no unit is guessed)
  const t = snapshotFactValue({ run: 'r', quantity: 'time', shown: '0', value: 0, snapshot: 3, of: 24, step: 20, time: 0 });
  expect(t.unit).toEqual({ kind: 'time' });
});

// ---- ADR 0734: how a run ran (its launch record, build and mesh) and its time unit

const LAUNCH = {
  launch_seq: 1,
  argv: ['C:\\bin\\rcfd.exe', 'run', 'C:\\p\\runs\\r\\case.snapshot', '--case-dir', 'C:\\p\\cases', '--threads', '1'],
  cwd: 'C:\\p\\runs\\r',
  solver: { path: 'C:\\bin\\rcfd.exe', version: 'rcfd 0.1.0', build: { exe_hash: 'fnv64:3af0108be1275f3d', git_rev: null } },
  started: '2026-11-17T15:17:46.798804600Z',
};

test('how a run ran: the launch record, the build, the mesh checked by its hash now, the rerun command, the time unit', async () => {
  const withMesh = JSON.stringify({ ...JSON.parse(RUN_JSON), mesh: 'meshes/a.msh', mesh_hash: 'fnv64:0000000000000002', rerun_of: 'r-20261116T090000-0001' });
  const files = (meshHash: string) => ({
    [`runs/${RUN}/run.json`]: { text: withMesh, hash: 'r' },
    [`runs/${RUN}/case.snapshot`]: { text: 'a', hash: 'fnv64:00000000000000aa' },
    'cases/cavity-re100.toml': { text: 'a', hash: 'fnv64:00000000000000aa' },
    'meshes/a.msh': { text: 'm', hash: meshHash },
  });
  const s = fakeServer(files('fnv64:0000000000000002'), 10, { run: RUN, launch: LAUNCH, time_unit: 's' });
  const p = await readRunProvenance(s.query, RUN, 'C:\\p', engine, () => 'now');
  expect(p.launch).toEqual(launchFacts(LAUNCH));
  expect(p.launch?.exeHash).toBe('fnv64:3af0108be1275f3d');
  expect(p.timeUnit).toBe('s');
  expect(p.meshCheck).toMatchObject({ path: 'meshes/a.msh', matches: true });
  expect(p.record?.rerunOf).toBe('r-20261116T090000-0001');
  expect(p.reproduce.gaps).toEqual([]);
  const f = new Map(howItRanFacts(p));
  expect(f.get('Command')).toBe(shellLine(LAUNCH.argv));
  expect(f.get('Build')).toBe('fnv64:3af0108be1275f3d, no git revision (the build set none)');
  expect(f.get('Solver')).toBe('rcfd 0.1.0, C:\\bin\\rcfd.exe');
  expect(f.get('Mesh')).toBe('meshes/a.msh, fnv64:0000000000000002; unchanged');
  expect(f.get('Rerun of')).toBe('r-20261116T090000-0001');
  expect(f.get('Rerun')).toBe(`rcfd-runner rerun ${RUN} --project 'C:\\p'`);
  expect(new Set(s.calls.map((c) => c.tool))).toEqual(new Set(['read_project_file', 'run_status', 'hash_project_file']));
  // the time unit of a run's and a snapshot's time comes from run_status
  const t = snapshotFactValue({ run: RUN, quantity: 'time', shown: '0.5', value: 0.5, snapshot: 3, of: 24, step: 20, time: 0.5 });
  expect(unitText(t, p)).toBe('s, time_unit of run_status (simulated time; 0 for a steady run)');
  // a changed mesh is said, with its hash now
  const changed = await readRunProvenance(fakeServer(files('fnv64:0000000000000009'), 10, { run: RUN, launch: LAUNCH, time_unit: 's' }).query, RUN, 'C:\\p', engine, () => 'now');
  expect(new Map(howItRanFacts(changed)).get('Mesh')).toBe('meshes/a.msh, fnv64:0000000000000002; changed: fnv64:0000000000000009 now');
  expect(changed.reproduce.gaps).toEqual(['The mesh meshes/a.msh changed since the run (hash fnv64:0000000000000009 now, fnv64:0000000000000002 when the run was made).']);
  expect(changed.rerun.gaps).toEqual(changed.reproduce.gaps);
  // a run without a launch record (not launched, or launched before ADR 0734): said, never filled in
  const old = await readRunProvenance(fakeServer(files('x'), 10, { run: RUN, launch: null, time_unit: 's' }).query, RUN, 'C:\\p', engine, () => 'now');
  expect(old.launch).toBeNull();
  expect(new Map(howItRanFacts(old)).get('Command')).toBe(NO_LAUNCH_RECORD);
  // an engine before ADR 0734: no launch member and no time unit
  const before = await readRunProvenance(fakeServer(files('x'), 10, { run: RUN }).query, RUN, 'C:\\p', engine, () => 'now');
  expect(before.launchNote).toContain('an engine before ADR 0734');
  expect(unitText(t, before)).toBe('none: run_status gives no time_unit (an engine before ADR 0734)');
  expect(launchFacts({ argv: [] })).toBeNull();
  expect(launchFacts('x')).toBeNull();
});

test('provenance.css uses only defined tokens and no colour literals; corners square, no shadow of its own', () => {
  const css = readFileSync(path.join(renderer, 'provenance', 'provenance.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(0);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css).not.toMatch(/border-radius|box-shadow/);
  // the trigger at rest is the text it wraps: no border, fill or padding of its own (G22)
  expect(css).toMatch(/\.pv-num\s*\{\s*all: unset;/);
});

test('the README names the comparison of the reproduce test before its code (registered exclusions)', () => {
  const readme = readFileSync(path.join(appRoot, 'README.md'), 'utf8');
  const section = readme.slice(readme.indexOf('## Provenance and reproduce from the CLI (F-828)'));
  expect(section.length).toBeGreaterThan(100);
  for (const f of ['fields/store.rts', 'residuals.jsonl', 'output/monitors.json', 'metrics.json', 'log.txt', 'events.jsonl']) expect(section).toContain(f);
});
