// The capability inventory (F-824): every capability the engine reports (service.info methods, settings.schema keys, the
// MCP tool list, the CLI help); the committed inventory is the engine's own (regenerated here from the rcfd binaries);
// docs/studio-coverage.md is the report generated from it. ADR 0808 (F-981) retired the G15 map of UI paths
// (capabilities/coverage.json) and its tests, on the maintainer's answer of 2026-11-26 in ADR 0803.
//
// The engine check needs the rcfd binary (tests/support/rcfd.ts says where it is looked for) and rcfd-runner and rcfd-mcp
// next to it; without rcfd it is skipped with that reason. Every other test reads only committed files.
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildInventory,
  commandNames,
  engineBinaries,
  helpCapabilities,
  INVENTORY_NOTE,
  inventoryReport,
  inventoryText,
  KIND_TITLES,
  KINDS,
  readEngine,
  schemaCapabilities,
  SOURCES,
  type Inventory,
} from '../support/capabilities.js';
import { findRcfd, NO_RCFD } from '../support/rcfd.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const repo = path.resolve(root, '..', '..');
const read = (...p: string[]): string => readFileSync(path.join(...p), 'utf8').replace(/\r\n/g, '\n');
const inventoryFile = path.join(root, 'capabilities', 'inventory.json');
const inventory = (): Inventory => JSON.parse(read(inventoryFile)) as Inventory;
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

test.describe('the committed inventory and report', () => {
  test('the inventory file is in its canonical form (order, counts, one capability per line)', () => {
    const inv = inventory();
    expect(inv.format).toBe('rcfd-studio-capabilities');
    expect(inv.note).toBe(INVENTORY_NOTE);
    expect(inv.sources).toEqual(SOURCES);
    expect(read(inventoryFile), REGENERATE).toBe(inventoryText(buildInventory(inv.engine as Record<string, string>, inv.capabilities)));
  });

  test('docs/studio-coverage.md is the report generated from the inventory', () => {
    expect(read(repo, 'docs', 'studio-coverage.md'), `${REGENERATE} -- --report-only`).toBe(inventoryReport(inventory()));
  });

  // N1 (ADR 0808, F-981): G15 is retired (ADR 0803); the map of UI paths and its check are gone, the inventory stays.
  test('the G15 map is retired: no capabilities/coverage.json, and the generator, the support code and the report name no UI path or exemption', () => {
    expect(existsSync(path.join(root, 'capabilities', 'coverage.json'))).toBe(false);
    const generator = read(root, 'scripts', 'write-capabilities.mjs');
    expect(generator).not.toContain('coverage.json');
    expect(generator).not.toMatch(/checkCoverage|coverageReport/);
    const support = read(root, 'tests', 'support', 'capabilities.ts');
    expect(support).not.toMatch(/checkCoverage|coverageReport|CoverageRule|exemption/i);
    const report = read(repo, 'docs', 'studio-coverage.md');
    expect(report).not.toMatch(/UI path|exemption/i);
  });

  // N2 (ADR 0808, F-981): the report still lists every capability the engine reports.
  test('the report counts every capability of the inventory by kind, and names every service method, MCP tool, CLI command and case section', () => {
    const inv = inventory();
    const report = read(repo, 'docs', 'studio-coverage.md');
    let total = 0;
    for (const k of KINDS) {
      const list = inv.capabilities.filter((c) => c.kind === k);
      total += list.length;
      expect(report, k).toContain(`| ${KIND_TITLES[k]} | ${list.length} |`);
      expect(report, k).toContain(`## ${KIND_TITLES[k]} (${list.length})`);
    }
    expect(total).toBe(inv.capabilities.length);
    expect(report).toContain(`| **All** | **${inv.capabilities.length}** |`);
    const name = (id: string) => '`' + id.slice(id.indexOf(':') + 1) + '`';
    for (const c of inv.capabilities) {
      if (c.kind === 'service_method' || c.kind === 'mcp_tool' || c.kind === 'cli_command' || c.kind === 'monitor_format_key') {
        expect(report, c.id).toContain(name(c.id));
      }
      if (c.kind === 'case_key') expect(report, c.id).toContain('`' + c.id.slice(5).split('.')[0]! + '` (');
      if (c.kind === 'cli_option') expect(report, c.id).toContain('`' + c.parent!.slice(4) + '` (');
    }
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
