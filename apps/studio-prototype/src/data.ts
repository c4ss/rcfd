// Synthetic preview data for the design prototype. Nothing here comes from the solver; every view that shows it says
// so. Run ids and case names are the real ones in example-projects/wind-tunnel.

export type RunState = 'queued' | 'running' | 'converged' | 'finished' | 'failed';

export interface Run {
  id: string;
  caseName: string;
  state: RunState;
  iterations: number;
  wallS?: number;
  note: string;
  /** Simulated end time of the run in s, the shared time axis is 0..20 s. */
  endTime: number;
  /** Fraction of the time axis spent in each phase, in order. */
  phases: { kind: 'queued' | 'iterating' | 'converged' | 'failed'; from: number; to: number }[];
  hasField: boolean;
}

export const T_END = 20;
export const SNAPSHOTS = 100;
export const DT_SNAPSHOT = T_END / SNAPSHOTS;

export const RUNS: Run[] = [
  {
    id: 'r-20261028T152128-37c8', caseName: 'step-re400', state: 'converged', iterations: 429, wallS: 13.2,
    note: 'res_u 9.07e-7', endTime: 13.2,
    phases: [{ kind: 'iterating', from: 0, to: 12.6 }, { kind: 'converged', from: 12.6, to: 13.2 }], hasField: false,
  },
  {
    id: 'r-20261028T152128-806b', caseName: 'step-re800', state: 'finished', iterations: 612, wallS: 19.8,
    note: 'end time reached', endTime: 19.8,
    phases: [{ kind: 'iterating', from: 0, to: 19.8 }], hasField: false,
  },
  {
    id: 'r-20261028T152128-d5cb', caseName: 'cylinder-re100', state: 'running', iterations: 1840,
    note: 't = 12.40 s of 20.00 s', endTime: 20,
    phases: [{ kind: 'iterating', from: 0, to: 12.4 }], hasField: true,
  },
  {
    id: 'r-20261028T152128-db3f', caseName: 'cylinder-re200', state: 'queued', iterations: 0,
    note: 'waiting for a free slot', endTime: 20,
    phases: [{ kind: 'queued', from: 0, to: 20 }], hasField: false,
  },
  {
    id: 'r-20261029T230332-0566', caseName: 'naca0012-aoa4', state: 'failed', iterations: 211,
    note: 'diverged at it 211', endTime: 1.4,
    phases: [{ kind: 'iterating', from: 0, to: 1.25 }, { kind: 'failed', from: 1.25, to: 1.4 }], hasField: false,
  },
];

export const STATE_WORD: Record<RunState, string> = {
  queued: 'Queued', running: 'Running', converged: 'Converged', finished: 'Finished', failed: 'Failed',
};

// Deterministic noise so every render of the prototype shows the same numbers.
function hash(n: number): number {
  const s = Math.sin(n * 127.1) * 43758.5453;
  return s - Math.floor(s);
}

/** log10 residuals of the running cylinder case at a snapshot index. */
export function residualsAt(k: number): { continuity: number; u: number; p: number } {
  const x = k / SNAPSHOTS;
  const wob = (hash(k) - 0.5) * 0.18;
  return {
    continuity: -1.2 - 4.2 * (1 - Math.exp(-3.2 * x)) + wob,
    u: -1.6 - 3.9 * (1 - Math.exp(-2.6 * x)) + wob * 0.8,
    p: -1.4 - 3.6 * (1 - Math.exp(-2.9 * x)) + wob * 1.2,
  };
}

const STROUHAL = 0.165; // vortex shedding of a cylinder near Re = 100, D = 0.1 m, U = 1 m/s
const F_SHED = (STROUHAL * 1.0) / 0.1;

/** Drag coefficient monitor at time t (s); oscillates at twice the shedding frequency once shedding has started. */
export function cdAt(t: number): number {
  const ramp = 1 - Math.exp(-t / 2.5);
  return 1.18 + 0.16 * ramp + 0.012 * ramp * Math.sin(2 * Math.PI * 2 * F_SHED * t);
}

export function iterationAt(k: number): number {
  return Math.round((1840 / 62) * k);
}

export function courantAt(k: number): number {
  return 0.74 + 0.08 * Math.min(1, k / 40) + (hash(k + 9) - 0.5) * 0.02;
}

// --- synthetic velocity field around a cylinder in a channel -------------------------------------------------------
// Domain 0..W by 0..H metres (scaled), cylinder at (CX, CY) radius R. Potential flow plus a street of Lamb-Oseen
// vortices advected downstream. This is a picture-maker for the prototype, not a solution of anything.

export const DOMAIN = { W: 2.2, H: 0.8, CX: 0.45, CY: 0.4, R: 0.05 };
export const U_MAX = 1.84;

export function velocity(x: number, y: number, t: number): [number, number] {
  const { CX, CY, R } = DOMAIN;
  const dx = x - CX, dy = y - CY;
  const r2 = dx * dx + dy * dy;
  if (r2 < R * R) return [0, 0];
  // potential flow around a cylinder, free stream 1 m/s
  const R2 = R * R;
  let u = 1 - (R2 * (dx * dx - dy * dy)) / (r2 * r2);
  let v = (-2 * R2 * dx * dy) / (r2 * r2);
  const ramp = 1 - Math.exp(-t / 2.5);
  const spacing = 0.24;
  const conv = 0.82;
  const phase = (conv * t) % spacing;
  for (let k = 0; k < 10; k++) {
    const xv = CX + 0.12 + phase + k * spacing * 0.5;
    if (xv < CX + 0.08) continue;
    const sign = k % 2 === 0 ? 1 : -1;
    const yv = CY + sign * 0.045 * Math.min(1, (xv - CX) / 0.3);
    const ex = x - xv, ey = y - yv;
    const rr = ex * ex + ey * ey + 1e-6;
    const core = 0.0016 + 0.0006 * k;
    const gamma = 0.12 * sign * ramp * Math.exp(-0.08 * k);
    const f = (gamma / (2 * Math.PI * rr)) * (1 - Math.exp(-rr / core));
    u += -f * ey;
    v += f * ex;
  }
  // wake deficit behind the body
  if (dx > 0) {
    const w = 0.05 + 0.08 * dx;
    u -= 0.55 * Math.exp(-(dy * dy) / (w * w)) * Math.exp(-dx / 1.6);
  }
  // channel walls slow the flow near y = 0 and y = H
  const wall = Math.min(y, DOMAIN.H - y);
  const damp = (1 - Math.exp(-wall / 0.03)) * 1.08;
  return [u * damp, v * damp];
}

export function velocityMagnitude(x: number, y: number, t: number): number {
  const [u, v] = velocity(x, y, t);
  return Math.min(U_MAX, Math.hypot(u, v));
}

// --- per-run time series for the bottom panel (synthetic) -----------------------------------------------------------
// Snapshots are every DT_SNAPSHOT on the shared run-time axis. Each run has its own residual history and, where the
// case defines one, a report monitor. A queued run has none: the panel says so instead of showing another run's data.

export interface Monitor { name: string; label: string; unit: string; at: (t: number) => number; lo: number; hi: number }

export interface RunSeries {
  /** Last snapshot index with data (running: grows live). */
  last: (available: number) => number;
  residuals: ((k: number) => { continuity: number; u: number; p: number }) | null;
  monitor: Monitor | null;
  iterationAt: (k: number) => number;
}

const decay = (k: number, n: number, floor: number, seed: number) => {
  const x = k / Math.max(1, n);
  const wob = (hash(k + seed) - 0.5) * 0.16;
  return {
    continuity: -1.3 + (floor - 0.4 + 1.3) * (1 - Math.exp(-3 * x)) + wob,
    u: -1.5 + (floor + 1.5) * (1 - Math.exp(-2.7 * x)) + wob * 0.8,
    p: -1.4 + (floor + 1.4) * (1 - Math.exp(-2.9 * x)) + wob * 1.1,
  };
};

const P_INLET: Monitor = { name: 'p_inlet', label: 'p_inlet', unit: 'Pa', lo: -0.20, hi: -0.10,
  at: (t) => -0.147 - 0.05 * Math.exp(-t / 2) };

export const SERIES: Record<string, RunSeries> = {
  'r-20261028T152128-37c8': {
    last: () => Math.round(13.2 / DT_SNAPSHOT),
    residuals: (k) => decay(k, 66, -6.05, 3),
    monitor: P_INLET,
    iterationAt: (k) => Math.round((429 / 66) * k),
  },
  'r-20261028T152128-806b': {
    last: () => Math.round(19.8 / DT_SNAPSHOT),
    residuals: (k) => decay(k, 99, -4.6, 7),
    monitor: { ...P_INLET, at: (t) => -0.162 - 0.04 * Math.exp(-t / 3) + 0.004 * Math.sin(t * 1.7) },
    iterationAt: (k) => Math.round((612 / 99) * k),
  },
  'r-20261028T152128-d5cb': {
    last: (available) => available,
    residuals: (k) => residualsAt(k),
    monitor: { name: 'cd', label: 'Cd', unit: '', lo: 1.30, hi: 1.37, at: cdAt },
    iterationAt: (k) => iterationAt(k),
  },
  'r-20261028T152128-db3f': {
    last: () => 0,
    residuals: null,
    monitor: null,
    iterationAt: () => 0,
  },
  'r-20261029T230332-0566': {
    last: () => 7,
    // residuals fall, then grow from iteration 190 (snapshot 6) until the run is stopped at 211 (snapshot 7)
    residuals: (k) => {
      const base = decay(k, 30, -4.2, 11);
      if (k < 6) return base;
      const g = (k - 5.4) * 2.6;
      return { continuity: base.continuity + g, u: base.u + g * 1.2, p: base.p + g * 1.1 };
    },
    monitor: { name: 'cl', label: 'Cl', unit: '', lo: 0.30, hi: 0.60, at: (t) => (t < 1.15 ? 0.44 + 0.01 * Math.sin(t * 9) : 0.44 + (t - 1.15) * 0.9) },
    iterationAt: (k) => Math.min(211, Math.round(30 * k + (k >= 6 ? 11 : 0))),
  },
};
