// The one shortcut table (F-831; G20, G8): no two shortcuts collide within a scope (or across scopes that are live
// together), none takes a key the Electron menu, Chromium, Windows or another Studio item (F-829 undo and redo, F-810
// the time panel) holds, the keys the surfaces bind are the ones the table lists, and chords are read the same way on
// every keyboard layout. The running Electron menu is checked against the table by tests/e2e/commands.spec.ts.
import { FLY_KEYS } from '../../src/renderer/field/camera.js';
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import {
  chord,
  eventChord,
  fromAccelerator,
  globalCommandFor,
  paletteModel,
  scopesOverlap,
  shortcutConflicts,
  SHORTCUTS,
  SCOPES,
  SCOPE_WORDS,
  type KeyLike,
  type Shortcut,
} from '../../src/shared/commands/index.js';
import { appRoot } from '../support/rcfd.js';

const renderer = path.join(appRoot, 'src', 'renderer');
const ev = (key: string, mods: Partial<KeyLike> = {}, code?: string): KeyLike => ({
  key,
  ...(code !== undefined ? { code } : {}),
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

test.describe('no two shortcuts collide', () => {
  test('the table has no conflict within a scope or across scopes live together', () => {
    expect(shortcutConflicts()).toEqual([]);
  });

  test('no shortcut of F-831 takes a reserved key, in any scope', () => {
    const reserved = new Set(SHORTCUTS.filter((s) => s.reserved).map((s) => s.keys));
    const own = SHORTCUTS.filter((s) => s.owner === 'F-831' && !s.reserved);
    expect(own.length).toBeGreaterThan(10);
    for (const s of own.filter((x) => x.scope === 'global')) expect(reserved.has(s.keys), s.keys).toBe(false);
    for (const s of own) {
      const clash = SHORTCUTS.filter((r) => r.reserved && r.keys === s.keys && scopesOverlap(r.scope, s.scope));
      expect(clash, `${s.keys} (${s.scope})`).toEqual([]);
    }
  });

  test('a global shortcut has a modifier or is a function key, so typing never runs a command', () => {
    for (const s of SHORTCUTS.filter((x) => x.scope === 'global' && !x.reserved)) {
      expect(/^(Ctrl|Alt)\+/.test(s.keys) || /^F\d+$/.test(s.keys), s.keys).toBe(true);
    }
  });

  test('F-829’s undo and redo and F-810’s time panel keys are reserved', () => {
    const kept = (keys: string, scope: Shortcut['scope'], owner: string) =>
      expect(SHORTCUTS.find((s) => s.keys === keys && s.scope === scope && s.reserved && s.owner === owner), `${keys} ${scope}`).toBeTruthy();
    for (const k of ['Ctrl+Z', 'Ctrl+Shift+Z', 'Ctrl+Y']) kept(k, 'global', 'F-829');
    for (const k of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End']) kept(k, 'time-panel', 'F-810');
  });

  test('the Chromium and Electron defaults the Studio keeps are reserved (reload, developer tools, zoom, clipboard, focus)', () => {
    const reserved = new Set(SHORTCUTS.filter((s) => s.reserved && s.scope === 'global').map((s) => s.keys));
    for (const k of ['Ctrl+R', 'Ctrl+Shift+R', 'Ctrl+Shift+I', 'Ctrl+0', 'Ctrl+-', 'Ctrl+=', 'Ctrl++', 'Ctrl+C', 'Ctrl+V', 'Ctrl+X', 'Ctrl+A', 'Ctrl+W', 'F11', 'Tab', 'Shift+Tab', 'Alt', 'F10', 'Alt+F4']) {
      expect(reserved.has(k), k).toBe(true);
    }
  });

  test('the check catches each kind of collision, and allows scopes that are never live together', () => {
    const base: Shortcut = { keys: 'Ctrl+K', scope: 'global', does: 'Open the command palette', command: 'palette.open', owner: 'F-831' };
    const add = (s: Partial<Shortcut>) => shortcutConflicts([base, ...SHORTCUTS.filter((x) => x.keys !== 'Ctrl+K'), { ...base, ...s, does: 'the new one' }]);
    expect(add({ keys: 'Ctrl+K' })).toHaveLength(1); // the same chord twice in one scope
    expect(add({ keys: 'Ctrl+K', scope: 'case-editor' })).toHaveLength(1); // a surface's key under a global one
    expect(add({ keys: 'Ctrl+R' })).toHaveLength(1); // the Electron menu's reload
    expect(add({ keys: 'Ctrl+Z' })).toHaveLength(1); // F-829's undo
    expect(add({ keys: 'Home', scope: 'time-panel' })).toHaveLength(1); // F-810's scrub to the start
    expect(add({ keys: 'Ctrl+ArrowLeft', scope: 'claude-message' })).toHaveLength(1); // a text field's word move
    expect(add({ keys: 'Escape', scope: 'palette' })).toHaveLength(1); // the palette's own Escape
    expect(add({ keys: 'Enter', scope: 'field-view' })).toEqual([]); // field view and palette are never live together
    expect(add({ keys: 'ArrowLeft', scope: 'runs-table' })).toEqual([]); // nor the runs table and the time panel
  });

  test('every scope has words, and the listing shows every scope', () => {
    for (const s of SHORTCUTS) expect(SCOPES).toContain(s.scope);
    for (const s of SCOPES) expect(SCOPE_WORDS[s].length).toBeGreaterThan(5);
  });
});

test.describe('the table is the one the window uses', () => {
  test('each chord is written canonically', () => {
    for (const s of SHORTCUTS) expect(chord(s.keys), s.keys).toBe(s.keys);
  });

  test('a command bound to a key exists in the palette, and each F-831 global key runs one', () => {
    const ids = new Set(paletteModel().map((e) => e.id));
    for (const s of SHORTCUTS) if (s.command !== null) expect(ids.has(s.command), `${s.keys} -> ${s.command}`).toBe(true);
    for (const s of SHORTCUTS.filter((x) => x.owner === 'F-831' && x.scope === 'global')) expect(s.command, s.keys).not.toBeNull();
  });

  test('the palette host binds exactly the global shortcuts of the table', () => {
    const toEvent = (keys: string): KeyLike => {
      const parts = keys === '+' ? ['+'] : keys.endsWith('++') ? [...keys.slice(0, -2).split('+'), '+'] : keys.split('+');
      const key = parts[parts.length - 1]!;
      const has = (m: string) => parts.slice(0, -1).includes(m);
      const code = /^[A-Z]$/.test(key) ? `Key${key}` : /^\d$/.test(key) ? `Digit${key}` : undefined;
      return ev(key.length === 1 ? key.toLowerCase() : key, { ctrlKey: has('Ctrl'), altKey: has('Alt'), shiftKey: has('Shift') }, code);
    };
    for (const s of SHORTCUTS) {
      const bound = globalCommandFor(toEvent(s.keys));
      if (s.scope === 'global' && !s.reserved && s.command) expect(bound, s.keys).toBe(s.command);
      else if (s.scope === 'global') expect(bound, s.keys).toBeNull();
    }
  });

  test('the case editor saves on Ctrl+S, the Claude field sends on Enter, the field view has the keys the table lists', () => {
    const editor = readFileSync(path.join(renderer, 'case', 'CaseEditorPage.tsx'), 'utf8');
    expect(editor).toMatch(/\(e\.ctrlKey \|\| e\.metaKey\) && !e\.altKey && !e\.shiftKey && e\.key\.toLowerCase\(\) === 's'/);
    const claude = readFileSync(path.join(renderer, 'claude', 'ClaudePanel.tsx'), 'utf8');
    expect(claude).toContain("e.key === 'Enter' && !e.shiftKey");
    expect(claude).toContain("e.key === 'Escape' && running");
    const canvas = readFileSync(path.join(renderer, 'field', 'FieldCanvas.tsx'), 'utf8');
    const block = canvas.slice(canvas.indexOf('key(key) {'), canvas.indexOf('default:', canvas.indexOf('key(key) {')));
    const cases = [...block.matchAll(/case '([^']+)':/g)].map((m) => chord(m[1]!));
    // ADR 0801 Addendum 22: and the fly keys, live while the right button is held (FLY_KEYS of field/camera.ts)
    const fly = [...FLY_KEYS].map((code) => chord(code.replace(/^Key/, '')));
    const listed = SHORTCUTS.filter((s) => s.scope === 'field-view').map((s) => s.keys).sort();
    expect([...cases, ...fly].sort()).toEqual(listed);
  });
});

test.describe('chords', () => {
  test('modifiers are put in order and spelt one way', () => {
    expect(chord('shift+ctrl+p')).toBe('Ctrl+Shift+P');
    expect(chord('Cmd+K')).toBe('Ctrl+K');
    expect(chord('Control+Alt+Delete')).toBe('Ctrl+Alt+Delete');
    expect(chord('Ctrl++')).toBe('Ctrl++');
    expect(chord('esc')).toBe('Escape');
    expect(chord('f5')).toBe('F5');
    expect(chord('Alt')).toBe('Alt');
    expect(() => chord('Ctrl+A+B')).toThrow(/two keys/);
    expect(() => chord('Ctrl+Shift')).toThrow(/no key/);
  });

  test('Electron accelerators read as chords', () => {
    expect(fromAccelerator('CommandOrControl+Shift+I')).toBe('Ctrl+Shift+I');
    expect(fromAccelerator('CmdOrCtrl+Plus')).toBe('Ctrl++');
    expect(fromAccelerator('CommandOrControl+-')).toBe('Ctrl+-');
    expect(fromAccelerator('Shift+CommandOrControl+Z')).toBe('Ctrl+Shift+Z');
    expect(fromAccelerator('Control+Y')).toBe('Ctrl+Y');
    expect(fromAccelerator('F11')).toBe('F11');
  });

  test('keyboard events: letters by their code on any layout, Ctrl and Cmd alike, a shifted symbol as its symbol', () => {
    expect(eventChord(ev('k', { ctrlKey: true }, 'KeyK'))).toBe('Ctrl+K');
    expect(eventChord(ev('k', { metaKey: true }, 'KeyK'))).toBe('Ctrl+K');
    expect(eventChord(ev('л', { ctrlKey: true }, 'KeyK'))).toBe('Ctrl+K');
    expect(eventChord(ev('P', { ctrlKey: true, shiftKey: true }, 'KeyP'))).toBe('Ctrl+Shift+P');
    expect(eventChord(ev('/', { ctrlKey: true }, 'Slash'))).toBe('Ctrl+/');
    expect(eventChord(ev('.', { ctrlKey: true }, 'Period'))).toBe('Ctrl+.');
    expect(eventChord(ev('&', { ctrlKey: true }, 'Digit1'))).toBe('Ctrl+1');
    expect(eventChord(ev('F5', {}, 'F5'))).toBe('F5');
    expect(eventChord(ev('+', { shiftKey: true }, 'Equal'))).toBe('+');
    expect(eventChord(ev('Enter', { shiftKey: true }, 'Enter'))).toBe('Shift+Enter');
    expect(eventChord(ev(' ', { altKey: true }, 'Space'))).toBe('Alt+Space');
    expect(eventChord(ev('Control', { ctrlKey: true }, 'ControlLeft'))).toBeNull();
    // AltGr (Ctrl+Alt on Windows) typing a character is no shortcut of the table
    expect(globalCommandFor(ev('@', { ctrlKey: true, altKey: true }, 'KeyQ'))).toBeNull();
    // a letter typed without a modifier runs nothing
    expect(globalCommandFor(ev('k', {}, 'KeyK'))).toBeNull();
  });
});
