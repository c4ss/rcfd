// Comparing two runs (F-815): the rules of ADR 0801, addendum F-815, registered before this code (G3), as pure functions
// shared by the main process's reader (the difference, C2 and C3) and the run workspace (C1 to C4). No Node or DOM
// types, no I/O.
//
// - C1, the same time: a run's time is its iteration when it is steady and its simulated time otherwise; the compared
//   run's snapshot at a stop is the one at the same iteration, or at exactly the same stored time. Never interpolated,
//   never the nearest.
// - C2, the same mesh: equal cell counts and equal recorded mesh content hashes (rcfd_post::remap's identity rule).
// - C3, the difference: B - A in f64 of the stored values, refused where rcfd_post::compare::difference refuses or a
//   conversion would be needed (G2).
// - C4, one colour range: the union of the two views' F-808 ranges, exactly.

/** What C1 compares a run's snapshots by. */
export type TimeKind = 'iteration' | 'time';

export interface Snap {
  readonly step: number;
  readonly time: number;
}

/**
 * C1: a run's time. `steady` is the runner's word (null when not known); a store whose every snapshot time is 0 is
 * steady (a steady solver writes 0).
 */
export function timeKind(snapshots: readonly Snap[], steady: boolean | null): TimeKind {
  if (steady === true) return 'iteration';
  if (steady === false) return 'time';
  return snapshots.every((s) => s.time === 0) ? 'iteration' : 'time';
}

export type Counterpart =
  | { readonly kind: 'match'; readonly snapshot: number; readonly step: number; readonly time: number }
  /** No snapshot of the compared run at that iteration or time. */
  | { readonly kind: 'none'; readonly by: TimeKind; readonly step: number; readonly time: number }
  /** One run is steady and the other transient: there is no common time. */
  | { readonly kind: 'kinds'; readonly a: TimeKind; readonly b: TimeKind };

/**
 * C1: the compared run's snapshot at the selected run's snapshot `at` (A's step and time): the same iteration, or
 * exactly the same stored time (the last snapshot that carries it).
 */
export function counterpart(at: Snap, aKind: TimeKind, b: readonly Snap[], bKind: TimeKind): Counterpart {
  if (aKind !== bKind) return { kind: 'kinds', a: aKind, b: bKind };
  let found = -1;
  for (let i = 0; i < b.length; i++) {
    const s = b[i]!;
    if (aKind === 'iteration' ? s.step === at.step : s.time === at.time) {
      found = i;
      if (aKind === 'iteration') break;
    }
  }
  if (found < 0) return { kind: 'none', by: aKind, step: at.step, time: at.time };
  return { kind: 'match', snapshot: found, step: b[found]!.step, time: b[found]!.time };
}

/** The text of a time in C1's terms: "it 45" or "t = 1.25 s". */
export function timeText(by: TimeKind, step: number, time: number): string {
  return by === 'iteration' ? `it ${step}` : `t = ${time} s`;
}

/** The words of a counterpart that is not a match (an exception: shown, G22). */
export function counterpartWords(c: Exclude<Counterpart, { kind: 'match' }>): string {
  if (c.kind === 'none') return `No snapshot at ${timeText(c.by, c.step, c.time)}`;
  return c.a === 'iteration' ? 'Steady against transient: no common time' : 'Transient against steady: no common time';
}

export interface MeshFacts {
  readonly nCells: number;
  readonly nPoints: number;
  /** The recorded content hash, 16 hexadecimal digits. */
  readonly meshHash: string;
}

export type Verdict = { readonly ok: true } | { readonly ok: false; readonly reason: string };

/** C2: the meshes match when the cell counts and the recorded content hashes are equal (and so the point counts). */
export function meshMatch(a: MeshFacts, b: MeshFacts): Verdict {
  if (a.nCells !== b.nCells) return { ok: false, reason: `the meshes differ: ${a.nCells} and ${b.nCells} cells` };
  if (a.meshHash !== b.meshHash) return { ok: false, reason: `the meshes differ: mesh ${a.meshHash} and ${b.meshHash}, both of ${a.nCells} cells` };
  if (a.nPoints !== b.nPoints) return { ok: false, reason: `the meshes differ: ${a.nPoints} and ${b.nPoints} points` };
  return { ok: true };
}

export interface FieldFacts {
  readonly name: string;
  readonly unit: string;
  readonly location: 'cell' | 'point';
  readonly n: number;
}

/** C3: whether one field of A and of B can be subtracted on a mesh of `nCells` cells. */
export function fieldsMatch(field: string, a: FieldFacts | undefined, b: FieldFacts | undefined, nCells: number): Verdict {
  if (!a) return { ok: false, reason: `the baseline run has no field ${field} at that snapshot` };
  if (!b) return { ok: false, reason: `the compared run has no field ${field} at that snapshot` };
  if (a.location !== 'cell' || b.location !== 'cell') return { ok: false, reason: `${field} is a point field; the difference is of cell fields` };
  if (a.n !== nCells || b.n !== nCells) return { ok: false, reason: `${field} has ${a.n} and ${b.n} values for ${nCells} cells` };
  if (a.unit !== b.unit) return { ok: false, reason: `${field} is in ${a.unit || 'no unit'} and in ${b.unit || 'no unit'}; the Studio converts no unit` };
  return { ok: true };
}

/**
 * C3: `b[c] - a[c]` in f64, every cell; null and the first cell that is not finite when there is one (rcfd_post's
 * `difference` refuses it the same way).
 */
export function difference(a: Float64Array, b: Float64Array): { values: Float64Array<ArrayBuffer> } | { values: null; reason: string } {
  if (a.length !== b.length) return { values: null, reason: `the fields have ${a.length} and ${b.length} values` };
  for (let i = 0; i < a.length; i++) {
    if (!Number.isFinite(a[i]!)) return { values: null, reason: `the baseline value of cell ${i} is not finite (${a[i]})` };
    if (!Number.isFinite(b[i]!)) return { values: null, reason: `the compared value of cell ${i} is not finite (${b[i]})` };
  }
  const d = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) d[i] = b[i]! - a[i]!;
  return { values: d };
}

export interface Span {
  readonly lo: number;
  readonly hi: number;
}

/** C4: the one range of two views: exactly the union of their ranges (either may be missing). */
export function unionSpan(a: Span | null, b: Span | null): Span | null {
  if (!a) return b;
  if (!b) return a;
  return { lo: Math.min(a.lo, b.lo), hi: Math.max(a.hi, b.hi) };
}
