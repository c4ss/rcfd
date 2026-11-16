// How the project home writes times, sizes, names and run states. Display only: no unit conversion beyond bytes to
// kilobytes for a file size, and every number keeps the engine's value.
import { markWord, type MarkState } from '../ui/index.js';

const two = (n: number): string => String(n).padStart(2, '0');

/** "14:03:22", local time; the input as given when it is not a time. */
export function clock(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/** "2026-10-28 15:26", local time. */
export function dateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** Kilobytes (1000 bytes) with one decimal; the caption carries the unit. */
export const kilobytes = (bytes: number): string => (bytes / 1000).toFixed(1);

/** A number as the engine gave it, without grouping, at most four decimals. */
export const plain = (v: number): string => v.toLocaleString('en', { useGrouping: false, maximumFractionDigits: 4 });

/** The last part of a path, `/` or `\` separated. */
export function baseName(p: string): string {
  const parts = p.split(/[\\/]+/).filter((x) => x !== '');
  return parts[parts.length - 1] ?? p;
}

/** A project-relative path without its top folder (`cases/a/b.toml` -> `a/b.toml`). */
export const inFolder = (p: string, top: string): string => (p.startsWith(`${top}/`) ? p.slice(top.length + 1) : p);

/** The runner's states and their pictograms (paused and cancelled since F-806, proposed for DESIGN.md). */
const PICTOGRAM: Readonly<Record<string, MarkState>> = {
  queued: 'queued',
  running: 'running',
  paused: 'paused',
  finished: 'finished',
  failed: 'failed',
  cancelled: 'cancelled',
};

export const runPictogram = (state: string): MarkState | null => PICTOGRAM[state] ?? null;

/** The word of a run state: DESIGN.md's word where there is one, else the engine's word with a capital. */
export function runWord(state: string): string {
  const p = runPictogram(state);
  if (p) return markWord(p);
  return state.length > 0 ? state[0]!.toUpperCase() + state.slice(1) : 'Unknown';
}

/** Where a change came from, in the home's words (the engine's `source`). */
export function sourceWord(source: string): string {
  switch (source) {
    case 'external':
      return 'Outside';
    case 'run':
      return 'Run';
    case 'journal':
      return 'Workspace';
    default:
      return source;
  }
}

export const plural = (n: number, one: string, many = `${one}s`): string => `${n} ${n === 1 ? one : many}`;
