// The command palette's model (F-831; G20, G16): generated from the action registries, so every registered action and
// Studio command is in it exactly once and is found by its name and by its id; each shows its CLI and Claude Code
// equivalent or says why there is none; the palette's stylesheet keeps to the design tokens.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { ACTIONS } from '../../src/shared/actions/registry.js';
import { RUN_ACTIONS } from '../../src/shared/actions/runs.js';
import { CASE_ACTIONS } from '../../src/shared/case.js';
import {
  CHOICE_KINDS,
  GROUP_ORDER,
  matchScore,
  paletteModel,
  registeredIds,
  searchPalette,
  SHORTCUTS,
  STUDIO_COMMANDS,
} from '../../src/shared/commands/index.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';

const model = paletteModel();
const ids = model.map((e) => e.id);
const renderer = path.join(appRoot, 'src', 'renderer');

test.describe('every registered action is in the palette (G20)', () => {
  test('each action of every registry, and each Studio command, is one entry of the palette model', () => {
    const registries = [
      ...ACTIONS.map((a) => a.id),
      ...RUN_ACTIONS.map((a) => a.id),
      ...CASE_ACTIONS.map((a) => a.id),
      ...STUDIO_COMMANDS.map((c) => c.id),
    ];
    expect(registries.length).toBeGreaterThan(40);
    for (const id of registries) expect(ids.filter((x) => x === id), id).toHaveLength(1);
    expect([...ids].sort()).toEqual([...registries].sort());
    expect([...registeredIds()].sort()).toEqual([...registries].sort());
  });

  test('the service actions are the registry’s, one per allow-listed method, with its title and parameters', () => {
    for (const a of ACTIONS) {
      const e = model.find((x) => x.id === a.id)!;
      expect(e.source).toBe('service');
      expect(e.title).toBe(a.title);
      expect(e.description).toBe(a.description);
      expect(e.params.map((p) => p.name)).toEqual(Object.keys(a.params));
      expect(e.params.map((p) => p.optional)).toEqual(Object.values(a.params).map((s) => s.optional === true));
      expect(e.cli).toContain(`rcfd call ${a.engine.service}`);
      expect(e.mcp).toBe(a.mcp.tool);
    }
  });

  test('the navigation commands go to Home, Runs, Case settings and the case editor; Claude Code has its two commands', () => {
    for (const id of ['go.home', 'go.runs', 'go.settings', 'go.case_editor', 'claude_code.register', 'claude_code.open', 'help.shortcuts', 'palette.open']) expect(ids).toContain(id);
  });

  test('ids and titles are unique, and the groups are the known ones in their order', () => {
    expect(new Set(ids).size).toBe(ids.length);
    const titles = model.map((e) => e.title);
    expect(new Set(titles).size).toBe(titles.length);
    const order = model.map((e) => GROUP_ORDER.indexOf(e.group));
    expect(order.every((g) => g >= 0)).toBe(true);
    expect(order).toEqual([...order].sort((a, b) => a - b));
  });
});

test.describe('the palette finds every action by name and by id', () => {
  test('its exact title and its exact id rank it first', () => {
    for (const e of model) {
      expect(searchPalette(model, e.title)[0]?.id, e.title).toBe(e.id);
      expect(searchPalette(model, e.id)[0]?.id, e.id).toBe(e.id);
      expect(searchPalette(model, e.title.toUpperCase())[0]?.id, e.title).toBe(e.id);
    }
  });

  test('a prefix of its title or its id, and the start of each word, find it', () => {
    for (const e of model) {
      const prefix = e.title.slice(0, Math.max(4, Math.ceil(e.title.length / 2)));
      expect(searchPalette(model, prefix).map((x) => x.id), prefix).toContain(e.id);
      expect(searchPalette(model, e.id.slice(0, e.id.length - 1)).map((x) => x.id), e.id).toContain(e.id);
      const initials = e.title
        .split(/\s+/)
        .filter((w) => /^[a-z]/i.test(w))
        .map((w) => w.slice(0, 2))
        .join(' ');
      expect(searchPalette(model, initials).map((x) => x.id), initials).toContain(e.id);
    }
  });

  test('a fuzzy subsequence finds it; a query that matches nothing finds nothing; nothing typed lists everything', () => {
    expect(searchPalette(model, 'rncase').map((x) => x.id)).toContain('project.run_case');
    expect(searchPalette(model, 'prjstop').map((x) => x.id)).toContain('project.stop_run');
    expect(searchPalette(model, 'zzzqqq')).toEqual([]);
    expect(searchPalette(model, '').map((e) => e.id)).toEqual(ids);
    expect(searchPalette(model, '   ').map((e) => e.id)).toEqual(ids);
  });

  test('ranking: exact over prefix over word prefixes over contained over fuzzy', () => {
    const e = model.find((x) => x.id === 'run.start')!;
    const s = (q: string) => matchScore(e, q);
    expect(s('start the run')).toBeGreaterThan(s('start'));
    expect(s('start')).toBeGreaterThan(s('th ru'));
    expect(s('th ru')).toBeGreaterThan(s('art the'));
    expect(s('art the')).toBeGreaterThan(s('sttr'));
    expect(s('sttr')).toBeGreaterThan(0);
    expect(s('nothing like it')).toBe(0);
  });
});

test.describe('each entry says how to do the same elsewhere (G16, G19)', () => {
  test('every entry has a description, and a CLI or Claude Code equivalent or a note that says why there is none', () => {
    for (const e of model) {
      expect(e.description.length, e.id).toBeGreaterThan(10);
      expect(e.cli !== null || e.mcp !== null || (e.note ?? '').length > 0, e.id).toBe(true);
    }
  });

  test('the run actions name their rcfd-runner command and the project server tool', () => {
    for (const a of RUN_ACTIONS) {
      const e = model.find((x) => x.id === a.id)!;
      expect(e.cli).toMatch(/^rcfd-runner /);
      if (a.engine.kind === 'mcp') expect(e.mcp).toBe(a.engine.tool);
      expect(e.confirms).toBe(a.confirm !== undefined);
    }
  });

  test('a session, a case, a run and a folder are chosen from a list, never typed', () => {
    for (const e of model) {
      for (const p of e.params) {
        if (p.name === 'session' && e.source === 'service') expect(p.kind).toBe('session');
        if ((p.name === 'case' || p.name === 'run') && e.source === 'run') expect(p.kind).toBe(p.name);
      }
    }
    expect(model.find((e) => e.id === 'case.open')!.params.map((p) => p.kind)).toEqual(['case']);
    expect(model.find((e) => e.id === 'project.open_recent')!.params.map((p) => p.kind)).toEqual(['recent']);
    expect([...CHOICE_KINDS].sort()).toEqual(['case', 'recent', 'run', 'session']);
  });

  test('the palette shows each command’s shortcut from the one table', () => {
    for (const s of SHORTCUTS) {
      if (s.command === null || s.reserved) continue;
      const e = model.find((x) => x.id === s.command);
      expect(e, `${s.keys} runs ${s.command}, which the palette must hold`).toBeTruthy();
      expect(e!.shortcuts).toContainEqual({ keys: s.keys, scope: s.scope });
      if (s.scope === 'global') expect([e!.shortcut, ...SHORTCUTS.filter((x) => x.command === s.command && x.scope === 'global').map((x) => x.keys)]).toContain(s.keys);
    }
    expect(model.find((e) => e.id === 'palette.open')!.shortcut).toBe('Ctrl+K');
    expect(model.find((e) => e.id === 'case.save')!.shortcuts).toEqual([{ keys: 'Ctrl+S', scope: 'case-editor' }]);
  });
});

test.describe('the renderer runs every Studio command', () => {
  test('each Studio command has a branch in src/renderer/commands (the palette or run.ts)', () => {
    const runTs = readFileSync(path.join(renderer, 'commands', 'run.ts'), 'utf8');
    const palette = readFileSync(path.join(renderer, 'commands', 'CommandPalette.tsx'), 'utf8');
    for (const c of STUDIO_COMMANDS) {
      const inRun = runTs.includes(`case '${c.id}':`);
      const inPalette = palette.includes(`e.id === '${c.id}'`);
      expect(inRun || inPalette, c.id).toBe(true);
    }
  });

  test('the commands a surface runs are registered by that surface (useCommandHandlers)', () => {
    const home = readFileSync(path.join(renderer, 'project', 'ProjectHome.tsx'), 'utf8');
    for (const id of ['go.home', 'go.runs', 'home.show_path', 'claude_code.show']) expect(home, id).toContain(`'${id}':`);
    const editor = readFileSync(path.join(renderer, 'case', 'CaseEditorPage.tsx'), 'utf8');
    for (const id of ['case.save', 'case.check_file', 'case.duplicate', 'case.close']) expect(editor, id).toContain(`'${id}':`);
  });
});

test('the palette stylesheet uses only defined tokens, no colour literal and square corners', () => {
  const css = readFileSync(path.join(renderer, 'commands', 'palette.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(20);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect([...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim())).toEqual(['var(--radius-none)', 'var(--radius-none)']);
  expect(css).not.toMatch(/font-family:(?!\s*var\(--font-family\))/);
});
