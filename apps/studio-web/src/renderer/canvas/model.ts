// The pure part of Claude's live canvas (F-979, ADR 0807 D3; Canvas.tsx draws it): which run a panel of a case follows,
// which change of the feed a panel shows, and who made it. No React, no DOM: tests/unit/canvas-f979.spec.ts imports it.
// The canvas chooses among what the engine gave (the run list, the feed's entries); it computes no value (G16, G23).
import type { CanvasPanel, ChangeEntry, RunEntry } from '../../shared/project.js';

const norm = (p: string): string => p.replace(/\\/g, '/');

/** The newest listed run of `kase` (by `created`, then by id), or null when the case has none. */
export function followedRun(runs: readonly RunEntry[], kase: string): RunEntry | null {
  const want = norm(kase);
  let best: RunEntry | null = null;
  for (const r of runs) {
    if (norm(r.case) !== want) continue;
    if (!best || r.created > best.created || (r.created === best.created && r.id > best.id)) best = r;
  }
  return best;
}

/** The run a monitor or plot panel shows: its own run, or the newest run of its case. */
export function panelRun(panel: CanvasPanel, runs: readonly RunEntry[]): string | null {
  if (typeof panel.run === 'string') return panel.run;
  if (typeof panel.case === 'string') return followedRun(runs, panel.case)?.id ?? null;
  return null;
}

/** Whether entry `e` is a change of what panel `p` shows (`run`: the run the panel shows now). */
export function concerns(p: CanvasPanel, e: ChangeEntry, run: string | null): boolean {
  const path = e.path === null ? null : norm(e.path);
  switch (p.kind) {
    case 'setting':
      return path !== null && path === norm(p.case ?? '') && (!e.changes || e.changes.some((c) => c.path === p.setting));
    case 'mesh':
      if (typeof p.case !== 'string') return false;
      if (path !== null && path.startsWith('meshes/')) return true;
      return path === norm(p.case) && (!e.changes || e.changes.some((c) => c.path.startsWith('mesh.')));
    case 'monitor':
    case 'plot':
      return run !== null && e.run === run;
    default:
      return false;
  }
}

/** The newest change of the feed (newest first, as the view keeps them) that panel `p` shows, or null. */
export function changeOf(p: CanvasPanel, changes: readonly ChangeEntry[], run: string | null = null): ChangeEntry | null {
  return changes.find((e) => concerns(p, e, run)) ?? null;
}

/** Who made a change: the journal's actor client and pid, `runner` for a run's own event, else null. */
export function actorOf(e: ChangeEntry): { readonly client: string; readonly pid: number | null } | null {
  if (e.actor) return e.actor;
  if (e.source === 'run') return { client: 'runner', pid: null };
  return null;
}

/** The old and new value a change gave a setting, as the journal wrote them (null when the entry names none). */
export function settingChange(e: ChangeEntry, setting: string): { readonly old: unknown; readonly new: unknown } | null {
  const c = e.changes?.find((x) => x.path === setting);
  return c ? { old: c.old, new: c.new } : null;
}
