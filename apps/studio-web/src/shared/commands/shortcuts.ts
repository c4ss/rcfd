// The Studio's one table of keyboard shortcuts (F-831; docs/studio-guardrails.md G20, G8). The command palette shows the
// shortcut of each command from here, the "Keyboard shortcuts" listing is this table, the palette's key handler binds
// the global entries from it, and tests/unit/commands-shortcuts.spec.ts fails when two entries collide.
//
// An entry is a chord in one scope. A scope is where the keys work: `global` anywhere in the window, the others only
// while the focus is inside that part (the case editor page, the field viewport, ...).
// Entries marked `reserved` are bound by someone else and never by F-831: the Electron default application menu (ADR
// 0801 keeps it), Chromium's own keys in text fields and for focus, and keys other Studio items own (undo and redo for
// F-829, the time panel's arrows, Home and End for F-810). Listing them is what lets the test refuse a shortcut that
// would take one of them.
//
// Two entries collide when they have the same chord in scopes that can be active together (SCOPE_OVERLAPS): the same
// scope, or `global` with any scope, or a part with the text fields inside it.
//
// Shared by the renderer and the tests: no Node or DOM types.

export type ShortcutScope =
  | 'global'
  | 'palette'
  | 'case-editor'
  | 'field-view'
  | 'time-panel'
  | 'runs-table'
  | 'confirm'
  | 'text-field';

/** Words for each scope, as the "Keyboard shortcuts" listing heads them. */
export const SCOPE_WORDS: Readonly<Record<ShortcutScope, string>> = {
  global: 'Anywhere in the window',
  palette: 'In the command palette',
  'case-editor': 'In the case editor',
  'field-view': 'In the field view (focused)',
  'time-panel': 'In the time panel (F-810)',
  'runs-table': 'In the runs table',
  confirm: 'In a run’s confirmation strip (Stop…, Cancel…)',
  'text-field': 'In any text field',
};

/** The order of the listing. */
export const SCOPES: readonly ShortcutScope[] = [
  'global',
  'palette',
  'case-editor',
  'runs-table',
  'confirm',
  'field-view',
  'time-panel',
  'text-field',
];

/**
 * The scopes whose keys can be live at the same time as a scope's own (besides itself and `global`, which is live
 * everywhere): a part that holds text fields overlaps `text-field`. Parts that hold each other's focus never coexist
 * (the palette is modal; the field view, the time panel and the runs table each take the keys only while focused).
 * The palette's search field is a combobox (React Aria's Autocomplete): its keys are the palette's own, so it does not
 * take the text-field keys it overrides (Home and End move through the list, as the combobox pattern does).
 */
export const SCOPE_OVERLAPS: Readonly<Record<ShortcutScope, readonly ShortcutScope[]>> = {
  global: [],
  palette: [],
  'case-editor': ['text-field'],
  'field-view': [],
  'time-panel': [],
  'runs-table': [],
  confirm: [],
  'text-field': ['case-editor'],
};

export function scopesOverlap(a: ShortcutScope, b: ShortcutScope): boolean {
  return a === b || a === 'global' || b === 'global' || SCOPE_OVERLAPS[a].includes(b) || SCOPE_OVERLAPS[b].includes(a);
}

export interface Shortcut {
  /** The chord, canonical (see `chord`): modifiers Ctrl, Alt, Shift in that order, then the key ("Ctrl+Shift+P"). */
  readonly keys: string;
  readonly scope: ShortcutScope;
  /** What it does, in words (the listing and the palette's hint). */
  readonly does: string;
  /** The palette command it runs (src/shared/commands/palette.ts), or null for a key of a surface that is not a command. */
  readonly command: string | null;
  /** Who binds it: the roadmap item, or the Electron menu or Chromium for reserved keys. */
  readonly owner: string;
  /** Bound by someone else; F-831 never binds it, and no other entry may take it. */
  readonly reserved?: true;
}

const ELECTRON_MENU = 'Electron default menu';
const CHROMIUM = 'Chromium';
const WINDOWS = 'Windows';

/** Every shortcut of the Studio, the reserved keys included. Ctrl is Cmd on macOS. */
export const SHORTCUTS: readonly Shortcut[] = [
  // ---- F-831: the command palette and the frequent actions (global)
  { keys: 'Ctrl+K', scope: 'global', does: 'Open the command palette', command: 'palette.open', owner: 'F-831' },
  { keys: 'Ctrl+Shift+P', scope: 'global', does: 'Open the command palette (alias of Ctrl+K)', command: 'palette.open', owner: 'F-831' },
  { keys: 'Ctrl+/', scope: 'global', does: 'Show the keyboard shortcuts', command: 'help.shortcuts', owner: 'F-831' },
  { keys: 'Ctrl+1', scope: 'global', does: 'Go to Home, the index of the project folder', command: 'go.home', owner: 'F-831' },
  { keys: 'Ctrl+2', scope: 'global', does: 'Go to Runs', command: 'go.runs', owner: 'F-831' },
  { keys: 'Ctrl+,', scope: 'global', does: 'Open the Studio’s settings (the theme)', command: 'go.studio_settings', owner: 'F-818' },
  { keys: 'Ctrl+O', scope: 'global', does: 'Open a project folder', command: 'project.open_folder', owner: 'F-831' },
  { keys: 'Ctrl+N', scope: 'global', does: 'New case (from a template or a shipped example)', command: 'case.new', owner: 'F-831' },
  { keys: 'F5', scope: 'global', does: 'Run a case of the project (asks which)', command: 'project.run_case', owner: 'F-831' },
  { keys: 'Ctrl+.', scope: 'global', does: 'Stop a project run (asks which, then confirms)', command: 'project.stop_run', owner: 'F-831' },

  // ---- F-828: provenance of the focused or pointed number (global)
  { keys: 'Ctrl+I', scope: 'global', does: 'Show the provenance of the focused or pointed number (else of the selected run)', command: 'provenance.show', owner: 'F-828' },

  // ---- the command palette (F-831)
  { keys: 'ArrowDown', scope: 'palette', does: 'Next command or choice', command: null, owner: 'F-831' },
  { keys: 'ArrowUp', scope: 'palette', does: 'Previous command or choice', command: null, owner: 'F-831' },
  { keys: 'Home', scope: 'palette', does: 'First command or choice', command: null, owner: 'F-831' },
  { keys: 'End', scope: 'palette', does: 'Last command or choice', command: null, owner: 'F-831' },
  { keys: 'Enter', scope: 'palette', does: 'Run the command, take the choice, or send the form', command: null, owner: 'F-831' },
  { keys: 'Escape', scope: 'palette', does: 'Close the palette; the focus goes back where it was', command: null, owner: 'F-831' },

  // ---- keys of the surfaces (each item that bound it)
  { keys: 'Ctrl+S', scope: 'case-editor', does: 'Save the case', command: 'case.save', owner: 'F-812' },
  { keys: 'Escape', scope: 'runs-table', does: 'Clear the selected run', command: null, owner: 'F-806' },
  { keys: 'Escape', scope: 'confirm', does: 'Keep things as they are; the focus goes back to the control', command: null, owner: 'F-807' },
  { keys: 'ArrowLeft', scope: 'field-view', does: 'Orbit left', command: null, owner: 'F-808' },
  { keys: 'ArrowRight', scope: 'field-view', does: 'Orbit right', command: null, owner: 'F-808' },
  { keys: 'ArrowUp', scope: 'field-view', does: 'Orbit up', command: null, owner: 'F-808' },
  { keys: 'ArrowDown', scope: 'field-view', does: 'Orbit down', command: null, owner: 'F-808' },
  { keys: '+', scope: 'field-view', does: 'Zoom in', command: null, owner: 'F-808' },
  { keys: '=', scope: 'field-view', does: 'Zoom in', command: null, owner: 'F-808' },
  { keys: '-', scope: 'field-view', does: 'Zoom out', command: null, owner: 'F-808' },
  { keys: 'Home', scope: 'field-view', does: 'Reset the view', command: null, owner: 'F-808' },
  { keys: '0', scope: 'field-view', does: 'Reset the view', command: null, owner: 'F-808' },

  // ---- reserved for other Studio items
  { keys: 'Ctrl+Z', scope: 'global', does: 'Undo (F-829; until then the Edit menu’s Undo in text fields)', command: null, owner: 'F-829', reserved: true },
  { keys: 'Ctrl+Shift+Z', scope: 'global', does: 'Redo (F-829)', command: null, owner: 'F-829', reserved: true },
  { keys: 'Ctrl+Y', scope: 'global', does: 'Redo (F-829; until then the Edit menu’s Redo in text fields)', command: null, owner: 'F-829', reserved: true },
  { keys: 'ArrowLeft', scope: 'time-panel', does: 'Scrub back (F-810)', command: null, owner: 'F-810', reserved: true },
  { keys: 'ArrowRight', scope: 'time-panel', does: 'Scrub forward (F-810)', command: null, owner: 'F-810', reserved: true },
  { keys: 'ArrowUp', scope: 'time-panel', does: 'Kept for the time panel (F-810); on a run band’s name, the previous name (F-816)', command: null, owner: 'F-810', reserved: true },
  { keys: 'ArrowDown', scope: 'time-panel', does: 'Kept for the time panel (F-810); on a run band’s name, the next name (F-816)', command: null, owner: 'F-810', reserved: true },
  { keys: 'Home', scope: 'time-panel', does: 'Scrub to the start (F-810); on a run band’s name, the first name (F-816)', command: null, owner: 'F-810', reserved: true },
  { keys: 'End', scope: 'time-panel', does: 'Scrub to the end (F-810); on a run band’s name, the last name (F-816)', command: null, owner: 'F-810', reserved: true },

  // ---- reserved: the Electron default application menu (tests/e2e/commands.spec.ts reads the running menu)
  { keys: 'Ctrl+X', scope: 'global', does: 'Cut', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+C', scope: 'global', does: 'Copy', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+V', scope: 'global', does: 'Paste', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+Shift+V', scope: 'global', does: 'Paste and match style', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+A', scope: 'global', does: 'Select all', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+R', scope: 'global', does: 'Reload the window (runs go on, G7)', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+Shift+R', scope: 'global', does: 'Force reload the window', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+Shift+I', scope: 'global', does: 'Developer tools', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+0', scope: 'global', does: 'Actual size', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl++', scope: 'global', does: 'Zoom in', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+=', scope: 'global', does: 'Zoom in', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+Shift+=', scope: 'global', does: 'Zoom in', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+-', scope: 'global', does: 'Zoom out', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'F11', scope: 'global', does: 'Full screen', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+M', scope: 'global', does: 'Minimize the window', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+W', scope: 'global', does: 'Close the window (runs go on, G7)', command: null, owner: ELECTRON_MENU, reserved: true },
  { keys: 'Ctrl+Q', scope: 'global', does: 'Quit (Linux and macOS)', command: null, owner: ELECTRON_MENU, reserved: true },

  // ---- reserved: Chromium and Windows
  { keys: 'Tab', scope: 'global', does: 'Next control', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+Tab', scope: 'global', does: 'Previous control', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+F10', scope: 'global', does: 'Context menu', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Alt', scope: 'global', does: 'The menu bar', command: null, owner: WINDOWS, reserved: true },
  { keys: 'F10', scope: 'global', does: 'The menu bar', command: null, owner: WINDOWS, reserved: true },
  { keys: 'Alt+Space', scope: 'global', does: 'The window menu', command: null, owner: WINDOWS, reserved: true },
  { keys: 'Alt+F4', scope: 'global', does: 'Close the window (runs go on, G7)', command: null, owner: WINDOWS, reserved: true },
  { keys: 'ArrowLeft', scope: 'text-field', does: 'Move the caret', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'ArrowRight', scope: 'text-field', does: 'Move the caret', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Home', scope: 'text-field', does: 'Caret to the start of the line', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'End', scope: 'text-field', does: 'Caret to the end of the line', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+ArrowLeft', scope: 'text-field', does: 'Select one character back', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+ArrowRight', scope: 'text-field', does: 'Select one character on', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+Home', scope: 'text-field', does: 'Select to the start of the line', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Shift+End', scope: 'text-field', does: 'Select to the end of the line', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+ArrowLeft', scope: 'text-field', does: 'Caret one word back', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+ArrowRight', scope: 'text-field', does: 'Caret one word on', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+Shift+ArrowLeft', scope: 'text-field', does: 'Select one word back', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+Shift+ArrowRight', scope: 'text-field', does: 'Select one word on', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+Home', scope: 'text-field', does: 'Caret to the start of the text', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+End', scope: 'text-field', does: 'Caret to the end of the text', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Backspace', scope: 'text-field', does: 'Delete back', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Delete', scope: 'text-field', does: 'Delete on', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+Backspace', scope: 'text-field', does: 'Delete one word back', command: null, owner: CHROMIUM, reserved: true },
  { keys: 'Ctrl+Delete', scope: 'text-field', does: 'Delete one word on', command: null, owner: CHROMIUM, reserved: true },
];

// ---- chords

const MODIFIERS = ['Ctrl', 'Alt', 'Shift'] as const;

/** Key names as a chord spells them (KeyboardEvent.key names, letters upper case). */
const KEY_ALIASES: Readonly<Record<string, string>> = {
  Esc: 'Escape',
  Return: 'Enter',
  Up: 'ArrowUp',
  Down: 'ArrowDown',
  Left: 'ArrowLeft',
  Right: 'ArrowRight',
  Del: 'Delete',
  Plus: '+',
  Space: 'Space',
  ' ': 'Space',
  Spacebar: 'Space',
};

/**
 * The canonical form of a chord: modifiers in the order Ctrl, Alt, Shift (Cmd, Meta, Control, CommandOrControl and
 * CmdOrCtrl are Ctrl), then the key, a letter upper case. Throws on a chord with no key or two keys.
 */
export function chord(text: string): string {
  // split on '+' but keep a '+' key ("Ctrl++", "+")
  const parts: string[] = [];
  let cur = '';
  for (let i = 0; i < text.length; i++) {
    const c = text[i]!;
    if (c === '+' && cur !== '') {
      parts.push(cur);
      cur = '';
    } else cur += c;
  }
  if (cur !== '') parts.push(cur);
  const mods = new Set<string>();
  let key: string | null = null;
  for (const raw of parts) {
    const p = raw.trim() === '' ? raw : raw.trim();
    const m = /^(ctrl|control|cmd|command|meta|super|commandorcontrol|cmdorctrl)$/i.test(p)
      ? 'Ctrl'
      : /^(alt|option|altgr)$/i.test(p)
        ? 'Alt'
        : /^shift$/i.test(p)
          ? 'Shift'
          : null;
    if (m) {
      mods.add(m);
      continue;
    }
    if (key !== null) throw new Error(`chord '${text}' names two keys (${key}, ${p})`);
    key = keyName(p);
  }
  if (key === null) {
    // a lone modifier is a key of its own ("Alt" opens the menu bar)
    if (mods.size === 1) return [...mods][0]!;
    throw new Error(`chord '${text}' names no key`);
  }
  return [...MODIFIERS.filter((m) => mods.has(m)), key].join('+');
}

function keyName(k: string): string {
  const alias = KEY_ALIASES[k] ?? KEY_ALIASES[k.charAt(0).toUpperCase() + k.slice(1).toLowerCase()];
  if (alias) return alias;
  if (k.length === 1) return k.toUpperCase();
  if (/^f\d{1,2}$/i.test(k)) return k.toUpperCase();
  return k.charAt(0).toUpperCase() + k.slice(1);
}

/** What a keyboard event needs to have for `eventChord` (a DOM KeyboardEvent has it). */
export interface KeyLike {
  readonly key: string;
  readonly code?: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
}

/**
 * The chord of a keyboard event, or null for a modifier pressed alone. Ctrl and Cmd are one modifier. A printable key is
 * read from `code` where the layout gives a letter or digit another name (Shift+1 is "!", a Cyrillic layout's K is "Л"),
 * so Ctrl+1 and Ctrl+K are the same chord on every layout; Shift on a symbol key stays in the key ("?" is Shift+/ on a
 * US layout, so Ctrl+/ is written with "/").
 */
export function eventChord(e: KeyLike): string | null {
  if (['Control', 'Meta', 'Alt', 'Shift', 'AltGraph', 'OS'].includes(e.key)) return null;
  let key = e.key;
  const code = e.code ?? '';
  const letter = /^Key([A-Z])$/.exec(code);
  const digit = /^Digit(\d)$/.exec(code);
  if (letter && !/^[a-z]$/i.test(key)) key = letter[1]!;
  else if (digit && (e.ctrlKey || e.metaKey || e.altKey)) key = digit[1]!;
  // a printable symbol carries its Shift already ("?", "+"); a name ("Enter", "F5") or a letter does not
  const symbol = key.length === 1 && !/^[a-z0-9]$/i.test(key);
  const mods = [e.ctrlKey || e.metaKey ? 'Ctrl' : null, e.altKey ? 'Alt' : null, e.shiftKey && !symbol ? 'Shift' : null].filter(Boolean);
  return chord([...mods, key === '+' ? 'Plus' : key].join('+'));
}

/** A chord as the listing and the palette show it ("Ctrl+Shift+P", "↑"). */
export function chordLabel(keys: string): string {
  return keys
    .replace(/ArrowUp/g, '↑')
    .replace(/ArrowDown/g, '↓')
    .replace(/ArrowLeft/g, '←')
    .replace(/ArrowRight/g, '→');
}

/** An Electron accelerator ("CommandOrControl+Shift+I", "CmdOrCtrl+Plus") as a chord of this table. */
export function fromAccelerator(accelerator: string): string {
  return chord(accelerator);
}

// ---- lookups

/** The shortcuts that run a palette command (the reserved ones run none). */
export function shortcutsOf(command: string): readonly Shortcut[] {
  return SHORTCUTS.filter((s) => s.command === command && !s.reserved);
}

/** The global shortcut that runs a command, when one does (the palette shows it beside the command). */
export function globalShortcutOf(command: string): Shortcut | null {
  return SHORTCUTS.find((s) => s.command === command && s.scope === 'global' && !s.reserved) ?? null;
}

/** The global command bound to an event's chord, or null. */
export function globalCommandFor(e: KeyLike): string | null {
  const c = eventChord(e);
  if (c === null) return null;
  return SHORTCUTS.find((s) => s.scope === 'global' && !s.reserved && s.command !== null && s.keys === c)?.command ?? null;
}

/** Every pair of entries that collide (the test keeps it empty), as words. */
export function shortcutConflicts(table: readonly Shortcut[] = SHORTCUTS): string[] {
  const out: string[] = [];
  for (let i = 0; i < table.length; i++) {
    for (let j = i + 1; j < table.length; j++) {
      const a = table[i]!;
      const b = table[j]!;
      if (a.keys !== b.keys || !scopesOverlap(a.scope, b.scope)) continue;
      out.push(`${a.keys}: '${a.does}' (${a.scope}, ${a.owner}) and '${b.does}' (${b.scope}, ${b.owner})`);
    }
  }
  return out;
}
