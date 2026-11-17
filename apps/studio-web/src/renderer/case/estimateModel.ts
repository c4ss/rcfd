// ADR 0737 (F-837): the case editor's estimate read from a `case.facts` reply (estimate.tsx shows it). Pure, no JSX:
// the unit tests read it. Every number is the engine's; nothing here computes a time step or a step count.
export const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);
export const str = (v: unknown): string | null => (typeof v === 'string' ? v : null);
export const obj = (v: unknown): Record<string, unknown> => (v !== null && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : {});

/** A number as the engine's facts are read: four significant digits, no exponent below 1e-4. */
export function short(x: number): string {
  if (x === 0) return '0';
  const a = Math.abs(x);
  if (a >= 1e-4 && a < 1e6) return String(Number(x.toPrecision(4)));
  return x.toExponential(2);
}

export interface Estimate {
  readonly cells: number | null;
  readonly hMin: number | null;
  readonly u0: number | null;
  /** The engine's first time step, steps and Courant estimate; null for a steady case (`refused` says why). */
  readonly dt0: number | null;
  readonly steps: number | null;
  readonly courant: number | null;
  readonly refused: string | null;
  readonly warning: string | null;
  readonly memoryMb: number | null;
  /** `runner_defaults` (no project: the runner's default budgets) or `project`. */
  readonly budgets: string | null;
}

/** The estimate of a `case.facts` reply (pure: the unit tests read it). */
export function estimateOf(reply: unknown): Estimate {
  const r = obj(reply);
  const rt = obj(r['run_time']);
  const m = obj(r['memory']);
  return {
    cells: num(r['cells']),
    hMin: num(r['h_min']),
    u0: num(r['u0']),
    dt0: num(rt['dt0']),
    steps: num(rt['steps_estimate']),
    courant: num(rt['courant_estimate']),
    refused: str(rt['refused']),
    warning: str(rt['warning']),
    memoryMb: num(m['mb']),
    budgets: str(m['budgets']),
  };
}

/** The line's values, each a number with its unit (G22 data), in the order they are read. */
export function estimateParts(e: Estimate): string[] {
  const parts: string[] = [];
  if (e.cells !== null) parts.push(`${e.cells} cells`);
  if (e.hMin !== null) parts.push(`h ${short(e.hMin)} m`);
  if (e.dt0 !== null) parts.push(`dt ${short(e.dt0)} s`);
  if (e.steps !== null) parts.push(`${e.steps} steps`);
  if (e.courant !== null) parts.push(`Courant ${short(e.courant)}`);
  if (e.memoryMb !== null) parts.push(`${e.memoryMb} MB`);
  return parts;
}

/** The line's full words, for its tooltip and readers. */
export function estimateWords(e: Estimate): string {
  const w: string[] = ['Estimate before you run, from the engine:'];
  if (e.cells !== null) w.push(`${e.cells} cells, the thinnest ${e.hMin === null ? 'unknown' : `${short(e.hMin)} m`};`);
  if (e.u0 !== null) w.push(`speed at the start ${short(e.u0)} m/s;`);
  if (e.dt0 !== null) w.push(`time step ${short(e.dt0)} s, about ${e.steps ?? '?'} steps, Courant ${e.courant === null ? '?' : short(e.courant)};`);
  else if (e.refused) w.push(`no time step: ${e.refused};`);
  if (e.memoryMb !== null) w.push(`memory about ${e.memoryMb} MB, ${e.budgets === 'project' ? "with the project's runner budgets" : "with the runner's default budgets (no project)"}.`);
  return w.join(' ');
}
