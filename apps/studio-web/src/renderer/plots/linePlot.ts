// The line drawing of the residual and monitor plots (F-811) on three.js: WebGPURenderer, which falls back to WebGL2 by
// itself where WebGPU is missing (PRODUCT.md, Stack; ADR 0801 addendum 3). One orthographic camera maps the plot range
// onto the canvas; each series is one LineSegments2 of fat, optionally dashed segments whose instance buffers grow in
// place, so an increment of a live run uploads only its new segments (G21). Text (ticks, names, values) is not drawn
// here: it is HTML beside the canvas, in Archivo with tabular figures, and reaches assistive technology.
//
// What is drawn is exactly the samples: consecutive drawable samples are joined by a straight segment; a sample that
// cannot be drawn (see range.ts) breaks the line there and is counted; a drawable sample between two that cannot is a
// dot. Nothing is smoothed, thinned or clamped (G3).
//
// The GPU holds 32-bit floats. So that every finite f64 a plot draws lands where it belongs (a monitor value of 1e300, a
// residual of 1e-320, an abscissa of 1e9 s with steps of 1e-3 s), the buffers hold coordinates in a frame: (v - origin)
// / span, with the origin and span of what was drawn when the frame was set. When the data outgrows the frame a
// thousandfold, the frame is set again and every segment is written again from the f64 samples.
import { Color, InstancedInterleavedBuffer, InterleavedBufferAttribute, Line2NodeMaterial, OrthographicCamera, Scene, WebGPURenderer } from 'three/webgpu';
import { LineSegments2 } from 'three/addons/lines/webgpu/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/addons/lines/LineSegmentsGeometry.js';
import { Extent } from './range.js';
import type { SeriesData } from './series.js';
import { roleHex, type LineStyle } from './style.js';

/** The renderer's backend once initialised: WebGPU, or three.js's WebGL2 fallback. */
export type Backend = 'WebGPU' | 'WebGL2';

/** Pixels kept free around the plot range inside the canvas, so a line on the range's edge is drawn whole. */
export const PLOT_INSET = { left: 3, right: 3, top: 3, bottom: 3 } as const;

/** How far the data may outgrow (or the range shrink inside) the frame before the frame is set again. */
const REFRAME = 1e3;

interface Frame {
  readonly ox: number;
  readonly sx: number;
  readonly oy: number;
  readonly sy: number;
}

/** Origin and span of [lo, hi] for a frame: the span is never 0 and never overflows. */
function frameOf(lo: number, hi: number): [number, number] {
  const span = hi - lo;
  if (Number.isFinite(span) && span > 0) return [lo, span];
  if (span === 0) return [lo, Math.abs(lo) || 1];
  return [0, Math.max(Math.abs(lo), Math.abs(hi))];
}

interface Track {
  readonly data: SeriesData;
  readonly style: LineStyle;
  readonly generation: number;
  /** Samples counted into `extent`. */
  scanned: number;
  /** Samples turned into segments. */
  synced: number;
  /** The last sample turned into segments, in frame coordinates (NaN when it was not drawable), and whether it began
   * a line that has neither a segment nor a dot yet. */
  prevX: number;
  prevY: number;
  prevStarted: boolean;
  /** Segments: start (x, y, 0) and end (x, y, 0) in frame coordinates. */
  segs: Float32Array;
  /** Dash distance in device pixels at the start and the end of each segment (dashed lines only). */
  dist: Float32Array;
  count: number;
  /** Device-pixel scale the distances were computed with. */
  distSx: number;
  distSy: number;
  distCount: number;
  readonly extent: Extent;
  readonly mesh: LineSegments2;
  readonly material: Line2NodeMaterial;
  geometry: LineSegmentsGeometry;
  segBuffer: InstancedInterleavedBuffer;
  distBuffer: InstancedInterleavedBuffer | null;
}

export interface PlotRange {
  readonly xlo: number;
  readonly xhi: number;
  readonly ylo: number;
  readonly yhi: number;
}

export class LinePlot {
  private renderer: WebGPURenderer | null = null;
  private readonly scene = new Scene();
  private readonly camera = new OrthographicCamera(0, 1, 1, 0, -1, 1);
  private readonly tracks = new Map<string, Track>();
  private width = 1;
  private height = 1;
  private dpr = 1;
  private range: PlotRange = { xlo: 0, xhi: 1, ylo: 0, yhi: 1 };
  private frame: Frame | null = null;
  private frameId = 0;
  private disposed = false;
  /** Times the frame was set (for the dev route and the tests). */
  reframes = 0;
  /** Time of the last draw (ms, from the frame request to the end of render()), for the dev route. */
  lastDrawMs = 0;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly log: boolean,
  ) {}

  /** Starts the renderer. Rejects with the reason when neither WebGPU nor WebGL2 is available. */
  async init(): Promise<Backend> {
    const r = new WebGPURenderer({ canvas: this.canvas, antialias: true, alpha: false });
    r.setClearColor(new Color(roleHex('ground')), 1);
    await r.init();
    if (this.disposed) {
      r.dispose();
      throw new Error('disposed');
    }
    this.renderer = r;
    this.resize(this.width, this.height, this.dpr);
    return (r.backend as { isWebGPUBackend?: boolean }).isWebGPUBackend ? 'WebGPU' : 'WebGL2';
  }

  /** The canvas size in CSS pixels and the device pixel ratio. */
  resize(width: number, height: number, dpr: number): void {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.dpr = dpr;
    if (this.renderer) {
      this.renderer.setPixelRatio(dpr);
      this.renderer.setSize(this.width, this.height, false);
    }
    for (const t of this.tracks.values()) this.styleMaterial(t.material, t.style);
    this.updateCamera();
  }

  /**
   * Brings the drawing up to date with `series` (name -> data and style): new series get a line, removed ones lose
   * theirs, replaced ones (a new generation) are rebuilt, grown ones get their new segments. Returns the extent of
   * what is drawn now, all series together.
   */
  sync(series: ReadonlyMap<string, { data: SeriesData; style: LineStyle }>): Extent {
    for (const [name, t] of this.tracks) {
      const s = series.get(name);
      if (!s || s.data !== t.data || s.style !== t.style || s.data.generation !== t.generation || s.data.length < t.scanned) this.drop(name);
    }
    const all = new Extent();
    for (const [name, { data, style }] of series) {
      const t = this.tracks.get(name) ?? this.track(name, data, style);
      for (; t.scanned < data.length; t.scanned++) t.extent.add(data.xAt(t.scanned), data.yAt(t.scanned), this.log);
      all.merge(t.extent);
    }
    if (!all.empty && this.needsFrame(all)) {
      const [ox, sx] = frameOf(all.xlo, all.xhi);
      const [oy, sy] = frameOf(all.ylo, all.yhi);
      this.frame = { ox, sx, oy, sy };
      this.reframes++;
      for (const t of this.tracks.values()) this.clearSegments(t);
    }
    if (this.frame) for (const t of this.tracks.values()) this.extend(t, this.frame);
    this.updateCamera();
    return all;
  }

  /** The extent of one series as drawn (after `sync`). */
  extentOf(name: string): Extent | null {
    return this.tracks.get(name)?.extent ?? null;
  }

  /** The plot range in plot space (log10 for the y of a log plot); it holds every drawn value (range.ts). */
  setRange(range: PlotRange): void {
    this.range = range;
    if (this.frame && this.rangeOutgrows(this.frame)) {
      // a range far wider than the frame (a value far from the others): set the frame from the range itself
      const [ox, sx] = frameOf(range.xlo, range.xhi);
      const [oy, sy] = frameOf(range.ylo, range.yhi);
      this.frame = { ox, sx, oy, sy };
      this.reframes++;
      for (const t of this.tracks.values()) {
        this.clearSegments(t);
        this.extend(t, this.frame);
      }
    }
    this.updateCamera();
  }

  /** Draws at the next animation frame (several requests in one frame draw once). */
  requestDraw(): void {
    if (this.frameId || this.disposed) return;
    const t0 = performance.now();
    this.frameId = requestAnimationFrame(() => {
      this.frameId = 0;
      if (!this.renderer || this.disposed) return;
      this.updateDistances();
      const t1 = performance.now();
      this.renderer.render(this.scene, this.camera);
      this.lastDrawMs = performance.now() - t0;
      // for the dev route and the measurement script: request to drawn, and the render call alone
      this.canvas.dataset['drawMs'] = this.lastDrawMs.toFixed(2);
      this.canvas.dataset['renderMs'] = (performance.now() - t1).toFixed(2);
      this.canvas.dataset['segments'] = String([...this.tracks.values()].reduce((a, t) => a + t.count, 0));
    });
  }

  dispose(): void {
    this.disposed = true;
    if (this.frameId) cancelAnimationFrame(this.frameId);
    for (const name of [...this.tracks.keys()]) this.drop(name);
    this.renderer?.dispose();
    this.renderer = null;
  }

  // ---- internals

  /** No frame yet, or the data reaches a thousand frame spans away from the frame, or spans a thousandth of it. */
  private needsFrame(e: Extent): boolean {
    const f = this.frame;
    if (!f) return true;
    const nx0 = (e.xlo - f.ox) / f.sx;
    const nx1 = (e.xhi - f.ox) / f.sx;
    const ny0 = (e.ylo - f.oy) / f.sy;
    const ny1 = (e.yhi - f.oy) / f.sy;
    const far = Math.max(Math.abs(nx0), Math.abs(nx1), Math.abs(ny0), Math.abs(ny1));
    if (!(far <= REFRAME)) return true;
    const thin = (a: number, b: number, lo: number, hi: number) => hi > lo && b - a < 1 / REFRAME;
    return thin(nx0, nx1, e.xlo, e.xhi) || thin(ny0, ny1, e.ylo, e.yhi);
  }

  private rangeOutgrows(f: Frame): boolean {
    const r = this.range;
    const n = [(r.xlo - f.ox) / f.sx, (r.xhi - f.ox) / f.sx, (r.ylo - f.oy) / f.sy, (r.yhi - f.oy) / f.sy];
    return !n.every((v) => Math.abs(v) <= REFRAME);
  }

  private updateCamera(): void {
    const f = this.frame ?? { ox: 0, sx: 1, oy: 0, sy: 1 };
    const { xlo, xhi, ylo, yhi } = this.range;
    const x0 = (xlo - f.ox) / f.sx;
    const x1 = (xhi - f.ox) / f.sx;
    const y0 = (ylo - f.oy) / f.sy;
    const y1 = (yhi - f.oy) / f.sy;
    const iw = Math.max(1, this.width - PLOT_INSET.left - PLOT_INSET.right);
    const ih = Math.max(1, this.height - PLOT_INSET.top - PLOT_INSET.bottom);
    const ux = (x1 - x0) / iw;
    const uy = (y1 - y0) / ih;
    this.camera.left = x0 - PLOT_INSET.left * ux;
    this.camera.right = x1 + PLOT_INSET.right * ux;
    this.camera.bottom = y0 - PLOT_INSET.bottom * uy;
    this.camera.top = y1 + PLOT_INSET.top * uy;
    this.camera.updateProjectionMatrix();
  }

  /** Device pixels per frame unit, x and y. */
  private scale(): [number, number] {
    const f = this.frame ?? { ox: 0, sx: 1, oy: 0, sy: 1 };
    const { xlo, xhi, ylo, yhi } = this.range;
    const iw = Math.max(1, this.width - PLOT_INSET.left - PLOT_INSET.right) * this.dpr;
    const ih = Math.max(1, this.height - PLOT_INSET.top - PLOT_INSET.bottom) * this.dpr;
    return [iw / ((xhi - f.ox) / f.sx - (xlo - f.ox) / f.sx), ih / ((yhi - f.oy) / f.sy - (ylo - f.oy) / f.sy)];
  }

  private styleMaterial(m: Line2NodeMaterial, style: LineStyle): void {
    m.linewidth = style.width * this.dpr;
    if (style.dash.length === 2) {
      m.dashSize = style.dash[0] * this.dpr;
      m.gapSize = style.dash[1] * this.dpr;
    }
  }

  private track(name: string, data: SeriesData, style: LineStyle): Track {
    const dashed = style.dash.length === 2;
    const material = new Line2NodeMaterial({ color: new Color(roleHex(style.role)), worldUnits: false, dashed } as ConstructorParameters<typeof Line2NodeMaterial>[0]);
    this.styleMaterial(material, style);
    const geometry = new LineSegmentsGeometry();
    const t: Track = {
      data,
      style,
      generation: data.generation,
      scanned: 0,
      synced: 0,
      prevX: Number.NaN,
      prevY: Number.NaN,
      prevStarted: false,
      segs: new Float32Array(0),
      dist: new Float32Array(0),
      count: 0,
      distSx: Number.NaN,
      distSy: Number.NaN,
      distCount: 0,
      extent: new Extent(),
      material,
      geometry,
      mesh: new LineSegments2(geometry, material),
      segBuffer: new InstancedInterleavedBuffer(new Float32Array(0), 6, 1),
      distBuffer: null,
    };
    t.mesh.frustumCulled = false;
    this.grow(t, 256);
    this.scene.add(t.mesh);
    this.tracks.set(name, t);
    return t;
  }

  private drop(name: string): void {
    const t = this.tracks.get(name);
    if (!t) return;
    this.scene.remove(t.mesh);
    t.geometry.dispose();
    t.material.dispose();
    this.tracks.delete(name);
  }

  private clearSegments(t: Track): void {
    t.count = 0;
    t.synced = 0;
    t.distCount = 0;
    t.prevX = Number.NaN;
    t.prevY = Number.NaN;
    t.prevStarted = false;
  }

  /** Gives `t` room for `cap` segments, in a new geometry (a new GPU buffer) holding the segments so far. */
  private grow(t: Track, cap: number): void {
    const segs = new Float32Array(cap * 6);
    segs.set(t.segs.subarray(0, t.count * 6));
    t.segs = segs;
    const geometry = new LineSegmentsGeometry();
    t.segBuffer = new InstancedInterleavedBuffer(segs, 6, 1);
    geometry.setAttribute('instanceStart', new InterleavedBufferAttribute(t.segBuffer, 3, 0));
    geometry.setAttribute('instanceEnd', new InterleavedBufferAttribute(t.segBuffer, 3, 3));
    if (t.style.dash.length === 2) {
      const dist = new Float32Array(cap * 2);
      dist.set(t.dist.subarray(0, t.distCount * 2));
      t.dist = dist;
      t.distBuffer = new InstancedInterleavedBuffer(dist, 2, 1);
      geometry.setAttribute('instanceDistanceStart', new InterleavedBufferAttribute(t.distBuffer, 1, 0));
      geometry.setAttribute('instanceDistanceEnd', new InterleavedBufferAttribute(t.distBuffer, 1, 1));
    }
    geometry.instanceCount = t.count;
    const old = t.geometry;
    t.geometry = geometry;
    t.mesh.geometry = geometry;
    if (old !== geometry) old.dispose();
  }

  /** Turns the samples of `t.data` after `t.synced` into segments, in frame coordinates. */
  private extend(t: Track, f: Frame): void {
    const d = t.data;
    if (d.length === t.synced) return;
    const from = t.count;
    for (let i = t.synced; i < d.length; i++) {
      const x = d.xAt(i);
      const y = d.yAt(i);
      const py = this.log ? (y > 0 ? Math.log10(y) : Number.NaN) : y;
      if (!Number.isFinite(x) || !Number.isFinite(py)) {
        // the line breaks; a sample that began a line and was not joined is drawn as a dot
        if (t.prevStarted) this.dot(t, t.prevX, t.prevY);
        t.prevX = Number.NaN;
        t.prevY = Number.NaN;
        t.prevStarted = false;
        continue;
      }
      const fx = (x - f.ox) / f.sx;
      const fy = (py - f.oy) / f.sy;
      if (Number.isNaN(t.prevX)) t.prevStarted = true;
      else {
        this.segment(t, t.prevX, t.prevY, fx, fy);
        t.prevStarted = false;
      }
      t.prevX = fx;
      t.prevY = fy;
    }
    // a last sample that began a line and has no successor yet is a dot (it stays when a successor joins it)
    if (t.prevStarted) {
      this.dot(t, t.prevX, t.prevY);
      t.prevStarted = false;
    }
    t.synced = d.length;
    t.geometry.instanceCount = t.count;
    if (t.count > from) {
      t.segBuffer.addUpdateRange(from * 6, (t.count - from) * 6);
      t.segBuffer.needsUpdate = true;
    }
  }

  private segment(t: Track, x0: number, y0: number, x1: number, y1: number): void {
    if (t.count + 1 > t.segs.length / 6) this.grow(t, Math.max(256, (t.segs.length / 6) * 2));
    const o = t.count * 6;
    t.segs[o] = x0;
    t.segs[o + 1] = y0;
    t.segs[o + 2] = 0;
    t.segs[o + 3] = x1;
    t.segs[o + 4] = y1;
    t.segs[o + 5] = 0;
    t.count++;
  }

  /** A sample with no drawable neighbour: a segment of a millionth of the frame, which the round caps draw as a dot. */
  private dot(t: Track, x: number, y: number): void {
    this.segment(t, x, y, x + 1e-6, y);
  }

  /** Dash distances in device pixels: all again when the scale changed, else only for new segments. */
  private updateDistances(): void {
    const [sx, sy] = this.scale();
    for (const t of this.tracks.values()) {
      if (!t.distBuffer) continue;
      const rescaled = t.distSx !== sx || t.distSy !== sy;
      const start = rescaled ? 0 : t.distCount;
      if (start === t.count) continue;
      const period = (t.style.dash[0]! + t.style.dash[1]!) * this.dpr;
      let acc = start === 0 ? 0 : t.dist[start * 2 - 1]!;
      for (let k = start; k < t.count; k++) {
        const o = k * 6;
        const len = Math.hypot((t.segs[o + 3]! - t.segs[o]!) * sx, (t.segs[o + 4]! - t.segs[o + 1]!) * sy);
        // the pattern runs along the line in device pixels; a segment under half a pixel (a dot) starts a dash, so it
        // never falls into a gap
        if (len < 0.5) acc = Math.ceil(acc / period) * period;
        t.dist[k * 2] = acc;
        acc += len;
        t.dist[k * 2 + 1] = acc;
      }
      t.distSx = sx;
      t.distSy = sy;
      t.distCount = t.count;
      t.distBuffer.addUpdateRange(start * 2, (t.count - start) * 2);
      t.distBuffer.needsUpdate = true;
    }
  }
}
