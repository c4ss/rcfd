// F-819b: a cell field's values on the boundary surface only, one per triangle of `data.surface` (its owner cell's
// value), as the field viewport draws, ranges and reads them out. Gathered where the whole field is (the store reader's
// worker, src/main/data/worker.ts; or the field worker of the renderer for a field that came whole), so the renderer
// gets 320 000 values instead of 4 000 000 at the 4e6 box. Shared by the main process and the renderer: no Node or DOM
// types.
//
// The values are copied as 32-bit words, so each is its cell's stored `f32` bit for bit (a NaN keeps its payload).

/** `out[t]` is `values[cells[t]]`, bit for bit; a cell index outside `values` is refused. */
export function gatherSurface(cells: Uint32Array, values: Float32Array): Float32Array<ArrayBuffer> {
  const out = new Float32Array(cells.length);
  const from = new Uint32Array(values.buffer, values.byteOffset, values.length);
  const to = new Uint32Array(out.buffer);
  const n = from.length;
  for (let t = 0; t < cells.length; t++) {
    const c = cells[t]!;
    if (c >= n) throw new RangeError(`triangle ${t} names cell ${c}, but the field has ${n} values`);
    to[t] = from[c]!;
  }
  return out;
}
