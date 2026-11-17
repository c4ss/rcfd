// ADR 0738 (F-809): the engine's derived pictures, read from the GLB `rcfd post derive` writes (rcfd_post::picture's
// GlbBuilder: one buffer, f32 and u32 accessors, triangles (4), line strips (3), lines (1)). Pure: bytes in, typed arrays
// out, so the unit tests import it. The Studio draws these arrays as they are; it computes no geometry of its own.

export interface GlbPrimitive {
  readonly mode: number;
  readonly attributes: Readonly<Record<string, Float32Array | Uint32Array>>;
  readonly indices: Uint32Array | null;
  readonly extras: Readonly<Record<string, unknown>>;
}

export interface Glb {
  /** `asset.extras`: the engine's reply (kind, snapshot, bounds, length_unit, counts). */
  readonly extras: Readonly<Record<string, unknown>>;
  readonly primitives: readonly GlbPrimitive[];
}

const MAGIC = 0x46546c67;
const JSON_CHUNK = 0x4e4f534a;
const BIN_CHUNK = 0x004e4942;

interface Accessor {
  bufferView: number;
  byteOffset?: number;
  componentType: number;
  count: number;
  type: string;
}

/** Reads a GLB of the engine; throws with the reason when it is not one. */
export function readGlb(buf: ArrayBuffer): Glb {
  const dv = new DataView(buf);
  if (buf.byteLength < 20 || dv.getUint32(0, true) !== MAGIC || dv.getUint32(4, true) !== 2) throw new Error('not a glTF 2 binary');
  const jsonLen = dv.getUint32(12, true);
  if (dv.getUint32(16, true) !== JSON_CHUNK) throw new Error('the first chunk is not JSON');
  const doc = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 20, jsonLen))) as {
    asset?: { extras?: Record<string, unknown> };
    meshes?: { primitives: { mode?: number; attributes: Record<string, number>; indices?: number; extras?: Record<string, unknown> }[] }[];
    accessors?: Accessor[];
    bufferViews?: { byteOffset?: number; byteLength: number }[];
  };
  const binHead = 20 + jsonLen;
  const hasBin = buf.byteLength >= binHead + 8 && dv.getUint32(binHead + 4, true) === BIN_CHUNK;
  const binAt = binHead + 8;
  const binLen = hasBin ? dv.getUint32(binHead, true) : 0;
  const read = (i: number): Float32Array | Uint32Array => {
    const a = doc.accessors?.[i];
    const v = a ? doc.bufferViews?.[a.bufferView] : undefined;
    if (!a || !v) throw new Error(`accessor ${i} is missing`);
    const comps = a.type === 'VEC3' ? 3 : a.type === 'SCALAR' ? 1 : 0;
    if (comps === 0) throw new Error(`accessor ${i}: type ${a.type}`);
    const off = (v.byteOffset ?? 0) + (a.byteOffset ?? 0);
    const n = comps * a.count;
    if (off + 4 * n > binLen) throw new Error(`accessor ${i} is past the end of the buffer`);
    const bytes = buf.slice(binAt + off, binAt + off + 4 * n);
    if (a.componentType === 5126) return new Float32Array(bytes);
    if (a.componentType === 5125) return new Uint32Array(bytes);
    throw new Error(`accessor ${i}: component type ${a.componentType}`);
  };
  const primitives: GlbPrimitive[] = [];
  for (const m of doc.meshes ?? [])
    for (const p of m.primitives) {
      const attributes: Record<string, Float32Array | Uint32Array> = {};
      for (const [k, i] of Object.entries(p.attributes)) attributes[k] = read(i);
      const idx = p.indices === undefined ? null : read(p.indices);
      if (idx && !(idx instanceof Uint32Array)) throw new Error('indices are not u32');
      primitives.push({ mode: p.mode ?? 4, attributes, indices: idx as Uint32Array | null, extras: p.extras ?? {} });
    }
  return { extras: doc.asset?.extras ?? {}, primitives };
}

const f32 = (p: GlbPrimitive, k: string): Float32Array => {
  const a = p.attributes[k];
  if (!(a instanceof Float32Array)) throw new Error(`no f32 attribute ${k}`);
  return a;
};
const u32 = (p: GlbPrimitive, k: string): Uint32Array => {
  const a = p.attributes[k];
  if (!(a instanceof Uint32Array)) throw new Error(`no u32 attribute ${k}`);
  return a;
};

/**
 * The engine's cut of the boundary surface (`cut_surface`): the kept triangles (3 corners each, unindexed), the
 * triangle of `data.surface` each was cut from (`_TRIANGLE`, what the readout names), and the arrows as line segments
 * with their count before the cut (T3's, `data-arrows`).
 */
export interface EngineCut {
  readonly positions: Float32Array<ArrayBuffer>;
  /** One per kept triangle: the source triangle of the store's surface. */
  readonly source: Uint32Array;
  readonly arrows: Float32Array<ArrayBuffer> | null;
  readonly arrowCount: number;
}

export function cutOf(g: Glb): EngineCut {
  const tri = g.primitives.find((p) => p.mode === 4);
  const lines = g.primitives.find((p) => p.mode === 1);
  const src = tri ? u32(tri, '_TRIANGLE') : new Uint32Array(0);
  const source = new Uint32Array(src.length / 3);
  for (let i = 0; i < source.length; i++) source[i] = src[3 * i]!;
  return {
    positions: (tri ? f32(tri, 'POSITION') : new Float32Array(0)) as Float32Array<ArrayBuffer>,
    source,
    arrows: lines ? (f32(lines, 'POSITION') as Float32Array<ArrayBuffer>) : null,
    arrowCount: lines && typeof lines.extras['arrows'] === 'number' ? (lines.extras['arrows'] as number) : 0,
  };
}

/** An iso-surface, unindexed (3 corners per triangle), with the engine's colour value at each corner. */
export interface EngineIso {
  readonly positions: Float32Array<ArrayBuffer>;
  readonly values: Float32Array<ArrayBuffer>;
  readonly triangles: number;
}

export function isoOf(g: Glb): EngineIso {
  const p = g.primitives.find((q) => q.mode === 4);
  if (!p) return { positions: new Float32Array(0), values: new Float32Array(0), triangles: 0 };
  const pos = f32(p, 'POSITION');
  const val = p.attributes['_VALUE'] instanceof Float32Array ? (p.attributes['_VALUE'] as Float32Array) : null;
  const idx = p.indices ?? Uint32Array.from({ length: pos.length / 3 }, (_, i) => i);
  const positions = new Float32Array(3 * idx.length);
  const values = new Float32Array(idx.length);
  for (let i = 0; i < idx.length; i++) {
    const v = idx[i]!;
    positions[3 * i] = pos[3 * v]!;
    positions[3 * i + 1] = pos[3 * v + 1]!;
    positions[3 * i + 2] = pos[3 * v + 2]!;
    values[i] = val ? val[v]! : 0;
  }
  return { positions, values, triangles: idx.length / 3 };
}

/** Streamlines as line segments (each strip's consecutive points), and how many lines there are. */
export interface EngineLines {
  readonly segments: Float32Array<ArrayBuffer>;
  readonly lines: number;
}

export function linesOf(g: Glb): EngineLines {
  const strips = g.primitives.filter((p) => p.mode === 3);
  let n = 0;
  for (const s of strips) n += Math.max(0, f32(s, 'POSITION').length / 3 - 1);
  const segments = new Float32Array(6 * n);
  let o = 0;
  for (const s of strips) {
    const p = f32(s, 'POSITION');
    for (let i = 0; i + 1 < p.length / 3; i++) {
      segments.set(p.subarray(3 * i, 3 * i + 6), o);
      o += 6;
    }
  }
  return { segments, lines: strips.length };
}
