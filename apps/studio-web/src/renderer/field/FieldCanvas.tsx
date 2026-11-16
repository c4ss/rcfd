// The field canvas (F-808; DESIGN.md Field view; guardrails G3, G9, G21): three.js WebGPURenderer, made and initialised
// asynchronously inside react-three-fiber's Canvas, with three's own WebGL 2 back end as the fallback (named on screen by
// FieldViewport). The surface is coloured on the GPU from the raw scalar: a TSL material maps each corner's stored value
// to its entry of the 256-entry viridis table (./colormap.ts, rcfd-render's map) in the vertex stage and reads the entry
// from a 256 x 1 texture with nearest sampling, unlit, so a pixel is exactly a table colour (no tone mapping, no colour
// space conversion, no MSAA). Everything per frame mutates three.js objects outside React (PRODUCT.md, Stack).
//
// The hover readout picks on the CPU against the same triangles (three's Raycaster on the drawn geometry): the triangle's
// owner cell and its stored value. The mesh grid is computed in a worker (./grid.worker.ts) the first time it is shown.
import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, type Ref } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import * as THREE from 'three/webgpu';
import { attribute, floor, texture, uniform, varying, vec2 } from 'three/tsl';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { SurfaceData } from '../../shared/data.js';
import { palette } from '../design/tokens.js';
import { LUT_SIZE, lutBytes, type ColourRange } from './colormap.js';
import { bounds, cornerPositions, cornerScalars, gridEdges } from './surface.js';
import type { PreparedSurface } from './prepare.js';

export type Backend = { readonly kind: 'webgpu' } | { readonly kind: 'webgl2'; readonly reason: string };

export interface Pick {
  /** Triangle of the drawn surface. */
  readonly triangle: number;
  /** Its owner cell in the stored mesh. */
  readonly cell: number;
  /** The stored value of that cell (the f32 the page received). */
  readonly value: number;
}

export interface BenchResult {
  readonly triangles: number;
  readonly uncapped: boolean;
  readonly frameMs: readonly number[];
  /** Uncapped only: from the draw call to the GPU's end of that frame's work (what a frame costs, without pacing). */
  readonly workMs: readonly number[];
  readonly emptyFrameMs: readonly number[];
}

export interface FieldCanvasApi {
  /** Back to the fitted view. */
  reset(): void;
  /** The cell under a point of the page (client coordinates), or null off the surface. */
  pickAt(clientX: number, clientY: number): Pick | null;
  /** The cell at the centre of the view (the keyboard's readout). */
  pickCentre(): Pick | null;
  /** Orbit, zoom and reset from the keyboard; true when the key was used. */
  key(key: string): boolean;
  /** P8 of ADR 0350: orbit one turn per 600 frames, `warm` frames, then `frames` timed frame intervals. */
  bench(options: { warm: number; frames: number; uncapped: boolean }): Promise<BenchResult>;
}

export interface FieldCanvasProps {
  /** Null while there is nothing to draw (the canvas, its renderer and device stay). */
  readonly surface: SurfaceData | null;
  /** One value per cell (a cell field). */
  readonly values: Float32Array | null;
  readonly range: ColourRange | null;
  readonly grid: boolean;
  readonly forceWebGL: boolean;
  readonly onBackend: (b: Backend) => void;
  /** The view was moved by the user (true) or reset (false). */
  readonly onMoved: (moved: boolean) => void;
  /** The first picture of this surface, values and range: drawn, finished on the GPU, and one animation frame later. */
  readonly onDrawn?: () => void;
  readonly onGridState?: (state: 'off' | 'computing' | 'on') => void;
  /**
   * F-832 (G21): the surface's arrays made off the main thread (./prepare.ts); used when made from `surface`, else the
   * canvas makes them itself (the P8 bench page).
   */
  readonly prepared?: PreparedSurface | null;
  /** F-832: the corner scalars of `values` made off the main thread (3 per triangle); else the canvas makes them. */
  readonly scalars?: Float32Array | null;
}

/** A colour of tokens.ts written to the canvas unchanged (the output does no colour space conversion). */
function raw(hex: string): THREE.Color {
  const n = Number.parseInt(hex.slice(1), 16);
  return new THREE.Color().setRGB(((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, THREE.LinearSRGBColorSpace);
}

const nextFrame = () => new Promise<number>((done) => requestAnimationFrame(done));

/** The renderer's GPU queue on the WebGPU back end (three does not type it). */
function gpuQueue(r: THREE.WebGPURenderer): { onSubmittedWorkDone(): Promise<void> } | null {
  const b = r.backend as unknown as { isWebGPUBackend?: boolean; device?: { queue: { onSubmittedWorkDone(): Promise<void> } } };
  return b.isWebGPUBackend && b.device ? b.device.queue : null;
}

/** Resolves when the GPU has finished the work submitted so far: the queue on WebGPU, a fence polled per frame on WebGL 2. */
async function finished(r: THREE.WebGPURenderer): Promise<void> {
  const q = gpuQueue(r);
  if (q) return q.onSubmittedWorkDone();
  const gl = (r.backend as unknown as { gl?: WebGL2RenderingContext }).gl;
  if (!gl) return;
  const sync = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0);
  if (!sync) return;
  gl.flush();
  try {
    for (let i = 0; i < 600; i++) {
      const s = gl.clientWaitSync(sync, 0, 0);
      if (s === gl.ALREADY_SIGNALED || s === gl.CONDITION_SATISFIED || s === gl.WAIT_FAILED) return;
      await nextFrame();
    }
  } finally {
    gl.deleteSync(sync);
  }
}

/** The table as a 256 x 1 texture: raw bytes, nearest sampling, no colour space, no mipmaps. */
function lutTexture(): THREE.DataTexture {
  const tex = new THREE.DataTexture(lutBytes(), LUT_SIZE, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.colorSpace = THREE.NoColorSpace;
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.flipY = false;
  tex.needsUpdate = true;
  return tex;
}

interface Material {
  readonly material: THREE.MeshBasicNodeMaterial;
  readonly lo: { value: number };
  readonly inv: { value: number };
}

/**
 * The TSL material: in the vertex stage k = floor(clamp((scalar - lo) * inv, 0, 1) * 255 + 0.5), passed flat (every
 * corner of a triangle has its cell's value), then the texel at k. `lutIndex` of ./colormap.ts is the same arithmetic.
 */
function fieldMaterial(lut: THREE.DataTexture): Material {
  const lo = uniform(0);
  const inv = uniform(1);
  const k = floor(attribute('scalar', 'float').sub(lo).mul(inv).clamp(0, 1).mul(LUT_SIZE - 1).add(0.5));
  const entry = varying(k, 'vLutEntry').setInterpolation(THREE.InterpolationSamplingType.FLAT);
  const material = new THREE.MeshBasicNodeMaterial();
  material.colorNode = texture(lut, vec2(entry.add(0.5).div(LUT_SIZE), 0.5)).rgb;
  material.side = THREE.DoubleSide;
  // the mesh grid's lines lie on the surface: push the surface back a little so they win the depth test
  material.polygonOffset = true;
  material.polygonOffsetFactor = 1;
  material.polygonOffsetUnits = 1;
  return { material, lo: lo as unknown as { value: number }, inv: inv as unknown as { value: number } };
}

/** The camera that shows the whole surface: a three-quarter view (z up), or straight on for a flat one. */
function fit(camera: THREE.PerspectiveCamera, controls: OrbitControls | null, s: SurfaceData, aspect: number, box?: { min: readonly number[]; max: readonly number[] }): void {
  const b = box ?? bounds(s.points);
  const c = new THREE.Vector3((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
  const ext = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
  const big = Math.max(...ext) || 1;
  const r = 0.5 * Math.hypot(ext[0]!, ext[1]!, ext[2]!) || 1;
  const flat = ext.findIndex((e) => e < 1e-6 * big);
  const dir = new THREE.Vector3();
  camera.up.set(0, 0, 1);
  if (flat === 2) {
    dir.set(0, 0, 1);
    camera.up.set(0, 1, 0);
  } else if (flat === 1) dir.set(0, -1, 0);
  else if (flat === 0) dir.set(1, 0, 0);
  else dir.set(1, -1.6, 1.1).normalize();
  const half = THREE.MathUtils.degToRad(camera.fov / 2);
  const fov = Math.min(half, Math.atan(Math.tan(half) * aspect));
  const dist = (1.08 * r) / Math.sin(fov);
  camera.position.copy(c).addScaledVector(dir, dist);
  camera.near = dist / 100;
  camera.far = dist * 100;
  camera.lookAt(c);
  camera.updateProjectionMatrix();
  if (controls) {
    controls.target.copy(c);
    controls.update();
  }
}

interface SceneProps extends Omit<FieldCanvasProps, 'forceWebGL' | 'onBackend'> {
  readonly apiRef: Ref<FieldCanvasApi>;
}

let gridIds = 0;

function Scene({ surface, values, range, grid, onMoved, onDrawn, onGridState, apiRef, prepared, scalars }: SceneProps) {
  // F-832: the arrays made off the main thread, when they belong to this surface
  const prep = prepared && prepared.surface === surface ? prepared : null;
  const box = prep ? { min: prep.min, max: prep.max } : undefined;
  const boxRef = useRef(box);
  boxRef.current = box;
  const { gl, camera, scene, invalidate, size } = useThree();
  const renderer = gl as unknown as THREE.WebGPURenderer;
  const cam = camera as THREE.PerspectiveCamera;
  const lut = useMemo(lutTexture, []);
  const mat = useMemo(() => fieldMaterial(lut), [lut]);
  const mesh = useMemo(() => new THREE.Mesh(new THREE.BufferGeometry(), mat.material), [mat]);
  const lines = useMemo(() => {
    const m = new THREE.LineBasicNodeMaterial({ color: raw(palette.white), transparent: true, opacity: 0.38, depthWrite: false });
    const l = new THREE.LineSegments(new THREE.BufferGeometry(), m);
    l.visible = false;
    return l;
  }, []);
  const controls = useRef<OrbitControls | null>(null);
  const report = useRef(onMoved);
  report.current = onMoved;
  // whether the user has moved the view since it was fitted (a resize refits an unmoved view)
  const userMoved = useRef(false);
  const moved = useRef((m: boolean) => {
    userMoved.current = m;
    report.current(m);
  });
  const aspect = size.width / Math.max(1, size.height);
  const aspectRef = useRef(aspect);
  aspectRef.current = aspect;
  const gridFor = useRef<SurfaceData | null>(null);

  useEffect(() => {
    scene.add(mesh, lines);
    return () => {
      scene.remove(mesh, lines);
      mesh.geometry.dispose();
      lines.geometry.dispose();
      mat.material.dispose();
      lut.dispose();
    };
  }, [scene, mesh, lines, mat, lut]);

  useEffect(() => {
    const c = new OrbitControls(cam, renderer.domElement);
    controls.current = c;
    c.enableDamping = false;
    c.screenSpacePanning = true;
    c.addEventListener('start', () => moved.current(true));
    c.addEventListener('change', () => invalidate());
    return () => {
      c.dispose();
      controls.current = null;
    };
  }, [cam, renderer, invalidate]);

  // the surface: new geometry, fitted view
  useEffect(() => {
    lines.visible = false;
    gridFor.current = null;
    if (!surface) {
      mesh.visible = false;
      invalidate();
      return;
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(prep ? prep.positions : cornerPositions(surface), 3));
    g.setAttribute('scalar', new THREE.BufferAttribute(new Float32Array(surface.triangles.length), 1));
    if (prep) {
      g.boundingSphere = new THREE.Sphere(new THREE.Vector3(...prep.centre), prep.radius);
      g.boundingBox = new THREE.Box3(new THREE.Vector3(...prep.min), new THREE.Vector3(...prep.max));
    } else {
      g.computeBoundingSphere();
      g.computeBoundingBox();
    }
    const old = mesh.geometry;
    mesh.geometry = g;
    old.dispose();
    lines.geometry.dispose();
    lines.geometry = new THREE.BufferGeometry();
    mesh.visible = false; // until its values are in
    fit(cam, controls.current, surface, aspectRef.current, boxRef.current);
    moved.current(false);
    // `prep` is made from `surface` (a new surface comes with its own); it is read when the surface is set
  }, [surface, mesh, lines, cam, invalidate]);

  // the values and the range, then the first picture of them
  useEffect(() => {
    if (!surface || !values || !range) return;
    const a = mesh.geometry.getAttribute('scalar') as THREE.BufferAttribute;
    if (scalars && scalars.length === a.array.length) (a.array as Float32Array).set(scalars);
    else cornerScalars(surface.cells, values, a.array as Float32Array<ArrayBuffer>);
    a.needsUpdate = true;
    mat.lo.value = range.loF32;
    mat.inv.value = range.invF32;
    mesh.visible = true;
    let live = true;
    renderer.render(scene, cam);
    void finished(renderer)
      .then(nextFrame)
      .then(() => {
        if (live) onDrawn?.();
      });
    invalidate();
    return () => {
      live = false;
    };
    // onDrawn is read when the picture is done; a new callback alone does not redraw
  }, [surface, values, scalars, range, mesh, mat, renderer, scene, cam, invalidate]);

  // the mesh grid: computed once per surface, in a worker
  useEffect(() => {
    if (!grid || !surface) {
      lines.visible = false;
      onGridState?.('off');
      invalidate();
      return;
    }
    if (gridFor.current === surface) {
      lines.visible = true;
      onGridState?.('on');
      invalidate();
      return;
    }
    let live = true;
    onGridState?.('computing');
    const show = (segments: Float32Array) => {
      if (!live) return;
      lines.geometry.dispose();
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.BufferAttribute(segments, 3));
      lines.geometry = g;
      lines.visible = true;
      gridFor.current = surface;
      onGridState?.('on');
      invalidate();
    };
    let worker: Worker | null = null;
    try {
      worker = new Worker(new URL('./grid.worker.ts', import.meta.url), { type: 'module' });
    } catch {
      worker = null;
    }
    const id = ++gridIds;
    if (worker) {
      worker.onmessage = (e: MessageEvent<{ id: number; segments: Float32Array }>) => {
        if (e.data.id === id) show(e.data.segments);
        worker?.terminate();
      };
      worker.onerror = () => {
        worker?.terminate();
        show(gridEdges(surface));
      };
      // copies: the surface stays with the page
      worker.postMessage({ id, points: surface.points.slice(), triangles: surface.triangles.slice(), cells: surface.cells.slice() });
    } else show(gridEdges(surface));
    return () => {
      live = false;
      worker?.terminate();
    };
    // onGridState is a reporter only, not a reason to compute again
  }, [grid, surface, lines, invalidate]);

  useEffect(() => {
    cam.aspect = aspect;
    cam.updateProjectionMatrix();
    if (surface && !userMoved.current) fit(cam, controls.current, surface, aspect, boxRef.current);
    invalidate();
  }, [aspect, cam, invalidate, surface]);

  useImperativeHandle(
    apiRef,
    (): FieldCanvasApi => {
      const raycaster = new THREE.Raycaster();
      const pickNdc = (x: number, y: number): Pick | null => {
        if (!surface || !values || !mesh.visible) return null;
        raycaster.setFromCamera(new THREE.Vector2(x, y), cam);
        const hit = raycaster.intersectObject(mesh, false)[0];
        if (!hit || hit.faceIndex == null) return null;
        const triangle = hit.faceIndex;
        const cell = surface.cells[triangle];
        if (cell === undefined) return null;
        return { triangle, cell, value: values[cell]! };
      };
      const orbit = (dAzimuth: number, dPolar: number, scale: number) => {
        const c = controls.current;
        const target = c ? c.target : new THREE.Vector3();
        const offset = cam.position.clone().sub(target);
        const q = new THREE.Quaternion().setFromUnitVectors(cam.up, new THREE.Vector3(0, 1, 0));
        offset.applyQuaternion(q);
        const s = new THREE.Spherical().setFromVector3(offset);
        s.theta += dAzimuth;
        s.phi = Math.min(Math.max(s.phi + dPolar, 0.01), Math.PI - 0.01);
        s.radius *= scale;
        offset.setFromSpherical(s).applyQuaternion(q.invert());
        cam.position.copy(target).add(offset);
        cam.lookAt(target);
        c?.update();
        moved.current(true);
        invalidate();
      };
      const reset = () => {
        if (!surface) return;
        fit(cam, controls.current, surface, aspectRef.current, boxRef.current);
        moved.current(false);
        invalidate();
      };
      return {
        reset,
        pickAt(clientX, clientY) {
          const r = renderer.domElement.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) return null;
          return pickNdc(((clientX - r.left) / r.width) * 2 - 1, -(((clientY - r.top) / r.height) * 2 - 1));
        },
        pickCentre: () => pickNdc(0, 0),
        key(key) {
          const step = THREE.MathUtils.degToRad(5);
          switch (key) {
            case 'ArrowLeft':
              orbit(-step, 0, 1);
              return true;
            case 'ArrowRight':
              orbit(step, 0, 1);
              return true;
            case 'ArrowUp':
              orbit(0, -step, 1);
              return true;
            case 'ArrowDown':
              orbit(0, step, 1);
              return true;
            case '+':
            case '=':
              orbit(0, 0, 1 / 1.1);
              return true;
            case '-':
              orbit(0, 0, 1.1);
              return true;
            case 'Home':
            case '0':
              reset();
              return true;
            default:
              return false;
          }
        },
        async bench({ warm, frames, uncapped }) {
          const c = controls.current;
          if (c) c.enabled = false;
          const target = c ? c.target.clone() : new THREE.Vector3();
          const start = cam.position.clone().sub(target);
          const radius = Math.hypot(start.x, start.y);
          const height = start.z;
          const empty: number[] = [];
          let last = await nextFrame();
          for (let i = 0; i < 240; i++) {
            const t = await nextFrame();
            empty.push(t - last);
            last = t;
          }
          const out: number[] = [];
          const work: number[] = [];
          let prev = 0;
          for (let f = 0; f < warm + frames + 1; f++) {
            const stamp = await nextFrame();
            const a = (2 * Math.PI * f) / 600;
            cam.position.set(target.x + radius * Math.cos(a), target.y + radius * Math.sin(a), target.z + height);
            cam.lookAt(target);
            const t0 = performance.now();
            renderer.render(scene, cam);
            if (uncapped) {
              await finished(renderer);
              if (f > warm) work.push(performance.now() - t0);
            }
            if (f > warm) out.push(stamp - prev);
            prev = stamp;
          }
          await finished(renderer);
          if (c) c.enabled = true;
          reset();
          return { triangles: surface ? surface.cells.length : 0, uncapped, frameMs: out, workMs: work, emptyFrameMs: empty };
        },
      };
    },
    [cam, mesh, surface, values, renderer, scene, invalidate],
  );

  return null;
}

/** The canvas: WebGPURenderer, initialised before the first frame; WebGL 2 when WebGPU is missing or forced. */
export const FieldCanvas = forwardRef(function FieldCanvas({ forceWebGL, onBackend, ...rest }: FieldCanvasProps, ref: Ref<FieldCanvasApi>) {
  const backend = useRef(onBackend);
  backend.current = onBackend;
  return (
    <Canvas
      flat
      linear
      frameloop="demand"
      camera={{ fov: 30, near: 0.01, far: 1000, position: [0, 0, 5] }}
      gl={async (defaults) => {
        const renderer = new THREE.WebGPURenderer({ canvas: defaults.canvas as HTMLCanvasElement, antialias: false, alpha: false, forceWebGL });
        await renderer.init();
        renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
        renderer.toneMapping = THREE.NoToneMapping;
        renderer.setClearColor(raw(palette['silver-wash']), 1);
        const isWebGPU = (renderer.backend as unknown as { isWebGPUBackend?: boolean }).isWebGPUBackend === true;
        backend.current(
          isWebGPU
            ? { kind: 'webgpu' }
            : {
                kind: 'webgl2',
                reason: forceWebGL ? 'asked for by the view' : 'gpu' in navigator ? 'no WebGPU adapter or device' : 'this window has no WebGPU',
              },
        );
        return renderer;
      }}
    >
      <Scene {...rest} apiRef={ref} />
    </Canvas>
  );
});
