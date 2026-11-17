// The capability inventory and its UI coverage (F-824, docs/studio-guardrails.md G15): every capability the engine
// reports (service.info methods, settings.schema keys, the MCP tool list, the CLI help) is mapped in
// capabilities/coverage.json to a UI path or to a dated exemption naming a roadmap item; the committed inventory is the
// engine's own (regenerated here from the rcfd binaries); docs/studio-coverage.md is the generated report.
//
// The engine check needs the rcfd binary (tests/support/rcfd.ts says where it is looked for) and rcfd-runner and rcfd-mcp
// next to it; without rcfd it is skipped with that reason. Every other test reads only committed files.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildInventory,
  checkCoverage,
  commandNames,
  coverageReport,
  engineBinaries,
  helpCapabilities,
  INVENTORY_NOTE,
  inventoryText,
  readEngine,
  schemaCapabilities,
  SOURCES,
  type Capability,
  type CoverageFile,
  type Inventory,
  type RoadmapItem,
} from '../support/capabilities.js';
import { findRcfd, NO_RCFD } from '../support/rcfd.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const repo = path.resolve(root, '..', '..');
const read = (...p: string[]): string => readFileSync(path.join(...p), 'utf8').replace(/\r\n/g, '\n');
const inventoryFile = path.join(root, 'capabilities', 'inventory.json');
const inventory = (): Inventory => JSON.parse(read(inventoryFile)) as Inventory;
const coverage = (): CoverageFile => JSON.parse(read(root, 'capabilities', 'coverage.json')) as CoverageFile;
const roadmap = (): RoadmapItem[] => JSON.parse(read(repo, 'docs', 'roadmap.json')) as RoadmapItem[];
const REGENERATE = 'run `npm run capabilities` in apps/studio-web';

test.describe('reading what the engine prints', () => {
  test('command names: words up to the first argument, alternatives with | in a word or between bracket-free parts', () => {
    expect(commandNames('validate <case>')).toEqual(['validate']);
    expect(commandNames('cases [list] [--dir <cases-dir>]')).toEqual(['cases']);
    expect(commandNames('data list|verify|provenance [--json]')).toEqual(['data list', 'data verify', 'data provenance']);
    expect(commandNames('pause <id> | resume <id> | cancel <id>')).toEqual(['pause', 'resume', 'cancel']);
    expect(commandNames('workflow show [--template <name> | --workflow <file>]')).toEqual(['workflow show']);
  });

  test('help: commands of the COMMANDS block, each flag under the command whose block names it', () => {
    const help = [
      'tool - a test binary',
      '',
      'USAGE:',
      '    tool <command> [--project <dir>]',
      '',
      'COMMANDS:',
      '    run <case> [options]       run a case',
      '        --out <dir>              output directory',
      '        --set <path=json>        override; see also --quiet',
      '    check <case> [--json]',
      '                               validate; --strict refuses warnings',
      '    data list|verify [--data <dir>]',
      '',
      'OUTPUT OPTIONS of run and check (at most one): where the output',
      'goes. A continuation of the title gives --also to them too.',
      '    --log-file <file>          appended to <file>',
      '',
      'OPTIONS:',
      '    -h, --help       print this help',
    ].join('\r\n');
    const ids = helpCapabilities('tool', help).map((c) => `${c.kind} ${c.id}${c.parent ? ` < ${c.parent}` : ''}`);
    expect(ids.sort()).toEqual(
      [
        'cli_command cli:tool run',
        'cli_command cli:tool check',
        'cli_command cli:tool data list',
        'cli_command cli:tool data verify',
        'cli_option cli:tool --project < cli:tool',
        'cli_option cli:tool --help < cli:tool',
        'cli_option cli:tool run --out < cli:tool run',
        'cli_option cli:tool run --set < cli:tool run',
        'cli_option cli:tool run --quiet < cli:tool run',
        'cli_option cli:tool check --json < cli:tool check',
        'cli_option cli:tool check --strict < cli:tool check',
        'cli_option cli:tool data list --data < cli:tool data list',
        'cli_option cli:tool data verify --data < cli:tool data verify',
        'cli_option cli:tool run --also < cli:tool run',
        'cli_option cli:tool check --also < cli:tool check',
        'cli_option cli:tool run --log-file < cli:tool run',
        'cli_option cli:tool check --log-file < cli:tool check',
      ].sort(),
    );
    // a binary without a COMMANDS block is one command, and its flags are its options
    const server = helpCapabilities('srv', 'srv: a server\n\nOPTIONS:\n    --root <dir>    confine\n');
    expect(server.map((c) => c.id)).toEqual(['cli:srv', 'cli:srv --root']);
  });

  test('settings.schema: leaf keys through properties, named maps (*) and object items ([]), with type and enumeration', () => {
    const caps = schemaCapabilities({
      case: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          solver: { type: 'object', properties: { algorithm: { type: 'string', enum: ['simple', 'piso'] }, dt: { type: ['number', 'null'] } } },
          boundaries: { type: 'object', additionalProperties: { type: 'object', properties: { velocity: { type: 'string' } } } },
          probes: { type: 'array', items: { type: 'object', properties: { at: { type: 'array' } } } },
          fields: { type: 'array', items: { type: 'string', enum: ['p', 'U'] } },
          free: { type: 'object', additionalProperties: true },
        },
      },
      monitors: { type: 'object', properties: { version: { type: 'integer' } } },
    });
    expect(caps).toEqual([
      { id: 'case:title', kind: 'case_key', source: 'settings.schema', type: 'string' },
      { id: 'case:solver.algorithm', kind: 'case_key', source: 'settings.schema', type: 'string', enum: ['simple', 'piso'] },
      { id: 'case:solver.dt', kind: 'case_key', source: 'settings.schema', type: ['number', 'null'] },
      { id: 'case:boundaries.*.velocity', kind: 'case_key', source: 'settings.schema', type: 'string' },
      { id: 'case:probes.[].at', kind: 'case_key', source: 'settings.schema', type: 'array' },
      { id: 'case:fields', kind: 'case_key', source: 'settings.schema', type: 'array', enum: ['p', 'U'] },
      { id: 'case:free', kind: 'case_key', source: 'settings.schema', type: 'object' },
      { id: 'monitors:version', kind: 'monitor_format_key', source: 'settings.schema', type: 'integer' },
    ]);
  });
});

test.describe('the coverage check', () => {
  const caps: Capability[] = [
    { id: 'service:run.start', kind: 'service_method', source: 'service.info' },
    { id: 'service:run.stop', kind: 'service_method', source: 'service.info' },
    { id: 'case:mesh.nx', kind: 'case_key', source: 'settings.schema' },
    { id: 'case:mesh.ny', kind: 'case_key', source: 'settings.schema' },
    { id: 'cli:rcfd run', kind: 'cli_command', source: 'help', usage: 'run <case>' },
    { id: 'cli:rcfd run --out', kind: 'cli_option', source: 'help', parent: 'cli:rcfd run' },
  ];
  const inv = buildInventory({ rcfd: 'test' }, caps);
  const items: RoadmapItem[] = [{ id: 'F-807', title: 'Electron Studio: run control' }];
  const ex = { reason: 'not built yet', date: '2026-11-16', item: 'F-807' };
  const file = (rules: CoverageFile['rules']): CoverageFile => ({ format: 'rcfd-studio-coverage', version: 1, note: '', rules });

  test('a full map passes; an id rule wins over a prefix', () => {
    const r = checkCoverage(
      inv,
      file([
        { id: 'service:run.start', ui: 'Run bar > Run' },
        { id: 'service:run.stop', exemption: ex },
        { prefix: 'case:mesh.', exemption: ex },
        { id: 'case:mesh.ny', ui: 'Case editor > Mesh > ny' },
        { id: 'cli:rcfd run', exemption: ex },
        { prefix: 'cli:rcfd run --', exemption: ex },
      ]),
      items,
    );
    expect(r.unmapped).toEqual([]);
    expect(r.problems).toEqual([]);
    expect(r.resolved.get('case:mesh.ny')?.ui).toBe('Case editor > Mesh > ny');
    expect(r.resolved.get('case:mesh.nx')?.prefix).toBe('case:mesh.');
  });

  test('fails on an unmapped capability, a malformed or undated exemption, an unknown item, a whole-kind prefix and a rule that maps nothing', () => {
    const r = checkCoverage(
      inv,
      file([
        { id: 'service:run.start', exemption: { ...ex, date: '16.11.2026' } },
        { id: 'service:run.stop', exemption: { ...ex, item: 'F-999' } },
        { prefix: 'case:', exemption: ex },
        { id: 'cli:rcfd run', exemption: { ...ex, reason: ' ' } },
        { id: 'cli:rcfd gone', exemption: ex },
        { id: 'cli:rcfd run --out', ui: 'x', exemption: ex },
      ]),
      items,
    );
    expect(r.unmapped).toEqual([]);
    expect(r.problems).toEqual([
      "service:run.start: the exemption's date is not a YYYY-MM-DD date",
      "service:run.stop: the exemption's item F-999 is not in docs/roadmap.json",
      'case:: a prefix names one case section (case:<section>.) or one command\'s options (cli:<command> --)',
      'cli:rcfd run: the exemption has no reason',
      'cli:rcfd run --out: a rule has exactly one of ui and exemption',
      'cli:rcfd gone: maps no capability of the inventory (removed from the engine? then remove the rule)',
    ]);
    const none = checkCoverage(inv, file([]), items);
    expect(none.unmapped).toEqual(inv.capabilities.map((c) => c.id));
  });
});

test.describe('the committed inventory, map and report', () => {
  test('the inventory file is in its canonical form (order, counts, one capability per line)', () => {
    const inv = inventory();
    expect(inv.format).toBe('rcfd-studio-capabilities');
    expect(inv.note).toBe(INVENTORY_NOTE);
    expect(inv.sources).toEqual(SOURCES);
    expect(read(inventoryFile), REGENERATE).toBe(inventoryText(buildInventory(inv.engine as Record<string, string>, inv.capabilities)));
  });

  test('every capability has a UI path or a recorded, dated exemption that names a roadmap item (G15)', () => {
    const res = checkCoverage(inventory(), coverage(), roadmap());
    expect(res.unmapped, 'capabilities with no rule in capabilities/coverage.json: map each to ui or to an exemption {reason, date, item}').toEqual([]);
    expect(res.problems, 'rules of capabilities/coverage.json').toEqual([]);
  });

  test('docs/studio-coverage.md is the report generated from the inventory and the map', () => {
    expect(read(repo, 'docs', 'studio-coverage.md'), `${REGENERATE} -- --report-only`).toBe(coverageReport(inventory(), coverage(), roadmap()));
  });
});

test.describe('the inventory against the engine', () => {
  const rcfd = findRcfd();
  test.skip(rcfd === null, NO_RCFD);

  test('the committed inventory is what the engine reports now (service.info, settings.schema, MCP tools/list, CLI help)', () => {
    const bins = engineBinaries(rcfd!);
    if ('missing' in bins) {
      throw new Error(`rcfd is ${rcfd}, but not next to it: ${bins.missing.join(', ')}; cargo build --release -p rcfd-runner -p rcfd-mcp`);
    }
    const engine = readEngine(bins);
    const committed = inventory();
    const ids = (i: Inventory) => new Set(i.capabilities.map((c) => c.id));
    const now = ids(engine);
    const before = ids(committed);
    expect([...now].filter((id) => !before.has(id)), `new in the engine (${rcfd}); ${REGENERATE}, then map them`).toEqual([]);
    expect([...before].filter((id) => !now.has(id)), `gone from the engine (${rcfd}); ${REGENERATE}`).toEqual([]);
    expect(read(inventoryFile), `the engine's facts changed (${rcfd}); ${REGENERATE}`).toBe(inventoryText(engine));
  });
});
