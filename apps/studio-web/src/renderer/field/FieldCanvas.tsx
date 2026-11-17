// The field canvas (F-808; DESIGN.md Field view; guardrails G3, G9, G21): three.js WebGPURenderer, made and initialised
// asynchronously inside react-three-fiber's Canvas, with three's own WebGL 2 back end as the fallback (named on screen by
// FieldViewport). The surface is coloured on the GPU from the raw scalar: a TSL material maps each corner's stored value
// to its entry of the 256-entry viridis table (./colormap.ts, rcfd-render's map) in the vertex stage and reads the entry
// from a 256 x 1 texture with nearest sampling, unlit, so a pixel is exactly a table colour (no tone mapping, no colour
// space conversion, no MSAA). Everything per frame mutates three.js objects outside React (PRODUCT.md, Stack).
//
// The hover readout picks on the CPU against the same triangles (three's Raycaster on the drawn geometry): the triangle's
// owner cell and its stored value. The mesh grid is computed in a worker (./grid.worker.ts) the first time it is shown.
//
// F-809: the cut plane is rcfd-render's (`View::clip`): the mesh grid (and the `arrows` prop's segments) keep a fragment
// only where `n . p <= d` (`maskNode`, the same test as the shader of the removed rcfd-render).
// ADR 0738: the engine's pictures (`rcfd post derive`, ./glb.ts) are drawn as they come, unmasked: its cut surface in
// place of the surface (coloured by the source triangle's value; the pick names that source triangle), its arrows, an
// iso-surface in place of the surface (coloured by its level through the same table), and streamlines in ink over
// everything. The mesh grid keeps the mask. Nothing here moves the camera.
//
// ADR 0801 Addendum 22: the camera is ./camera.ts's controller (Unity's scene view: right button to look and fly, middle
// to pan, Alt and left to orbit about the point under the cursor, the wheel toward the cursor), in place of three's
// OrbitControls; a 2D case (`thin`) is drawn by an orthographic camera straight along its thin axis, pan and zoom only.
import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState, type Ref } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import * as THREE from 'three/webgpu';
import { attribute, dot, float, floor, positionWorld, texture, uniform, varying, vec2, vec3 } from 'three/tsl';
import type { SurfaceData } from '../../shared/data.js';
import { palette } from '../design/tokens.js';
import { onThemeChange, themeHex } from '../theme/theme.js';
import { LUT_SIZE, lutBytes, type ColourRange } from './colormap.js';
import { clipPlane, type Cut } from './derived.js';
import type { EngineCut, EngineIso } from './glb.js';
import { bounds, cornerPositions, cornerScalars, cornerScalarsOf, gridEdges } from './surface.js';
import { drawOnDemand } from '../ui/onDemand.js';
import { FLY_KEYS, ORBIT_KEY_RAD, ViewCamera, ZOOM_KEY, cameraAttributes, flyMove, type Axis, type Vec3 } from './camera.js';
import type { PreparedSurface } from './prepare.js';

export type Backend = { readonly kind: 'webgpu' } | { readonly kind: 'webgl2'; readonly reason: string };

export interface Pick {
  /** Triangle of the drawn surface. */
  readonly triangle: number;
  /** Its owner cell in the stored mesh. */
  readonly cell: number;
  /** The stored value of that cell (the f32 the page received). */
  readonly value: number;
  /** F-809: where the pick ray meets the triangle (mesh coordinates). */
  readonly point?: readonly [number, number, number];
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
  /** Reset view (Addendum 22): the starting orientation, the whole domain framed, the pivot at its centre. */
  reset(): void;
  /** F: the whole domain framed, the orientation kept. */
  frame(): void;
  /** The cell under a point of the page (client coordinates), or null off the surface. */
  pickAt(clientX: number, clientY: number): Pick | null;
  /** The cell at the centre of the view (the keyboard's readout). */
  pickCentre(): Pick | null;
  /** Orbit (pan in 2D), zoom, frame, face on and reset from the keyboard; true when the key was used. */
  key(key: string): boolean;
  /**
   * Face on (Addendum 22): looks along the nearest principal axis of the view direction (a 2D case: its thin axis),
   * keeping the point at the centre of the view and the zoom; only the orientation changes.
   */
  faceOn(): void;
  /**
   * F-837: the axis the camera looks straight along (within 0.1 degree), and the length of one CSS pixel on the surface's
   * face toward the camera there (in the stored mesh's length unit); null when the view looks along no axis.
   */
  faceScale(): { readonly axis: 0 | 1 | 2; readonly perPixel: number; readonly widthPx: number } | null;
  /** P8 of ADR 0350: orbit one turn per 600 frames, `warm` frames, then `frames` timed frame intervals. */
  bench(options: { warm: number; frames: number; uncapped: boolean }): Promise<BenchResult>;
}

export interface FieldCanvasProps {
  /** Null while there is nothing to draw (the canvas, its renderer and device stay). */
  readonly surface: SurfaceData | null;
  /** One value per cell (a cell field), or with `valuesPer` 'triangle' one per triangle of `surface` (its owner cell's). */
  readonly values: Float32Array | null;
  /** F-819b: what `values` is indexed by (default 'cell'); 'triangle' is src/shared/surface-values.ts's gather. */
  readonly valuesPer?: 'cell' | 'triangle';
  readonly range: ColourRange | null;
  readonly grid: boolean;
  readonly forceWebGL: boolean;
  readonly onBackend: (b: Backend) => void;
  /** The view was moved by the user (true) or reset (false). */
  readonly onMoved: (moved: boolean) => void;
  /** F-837: the camera changed (any move, fit or resize), for what is drawn over the view (the scale bar). */
  readonly onView?: () => void;
  /**
   * The first picture of this surface, values and range: drawn, finished on the GPU, and one animation frame later. It
   * names the values drawn, so a host knows which of its fields is on screen (and that the view's handle is set).
   */
  readonly onDrawn?: (values: Float32Array) => void;
  readonly onGridState?: (state: 'off' | 'computing' | 'on') => void;
  /** F-809: rcfd-render's cut plane, or null for none. */
  readonly cut?: Cut | null;
  /** F-809: the vector arrows as line segments (./derived.ts `vectorGlyphs`), or null for none. */
  readonly arrows?: Float32Array<ArrayBuffer> | null;
  /**
   * F-832 (G21): the surface's arrays made off the main thread (./prepare.ts); used when made from `surface`, else the
   * canvas makes them itself (the P8 bench page).
   */
  readonly prepared?: PreparedSurface | null;
  /** F-832: the corner scalars of `values` made off the main thread (3 per triangle); else the canvas makes them. */
  readonly scalars?: Float32Array | null;
  /** ADR 0738: the engine's cut surface, drawn in place of the surface (null: the surface as stored). */
  readonly engineCut?: EngineCut | null;
  /** ADR 0738: the engine's arrows (line segments), drawn unmasked (null: none). */
  readonly engineArrows?: Float32Array<ArrayBuffer> | null;
  /** ADR 0738: the engine's iso-surface, drawn in place of the surface (null: none). */
  readonly iso?: EngineIso | null;
  /** ADR 0738: the engine's streamlines (line segments), in ink over everything (null: none). */
  readonly streamlines?: Float32Array<ArrayBuffer> | null;
  /** ADR 0738: the engine's pictures above are drawn, finished on the GPU, and one animation frame has passed. */
  readonly onPictureDrawn?: (picture: object | null) => void;
  /**
   * Addendum 22: the thin axis of a 2D case (one cell thick), or null for 3D. A 2D case is drawn orthographic, straight
   * along that axis, and only pans and zooms.
   */
  readonly thin?: Axis | null;
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

/** F-809: rcfd-render's cut plane as uniforms shared by every material: `on`, the normal `n` and `d`. */
interface Clip {
  readonly on: { value: number };
  readonly n: { value: THREE.Vector3 };
  readonly d: { value: number };
  /** The mask: true where a fragment is kept (`n . p <= d`, or no cut). */
  readonly mask: ReturnType<typeof dot>;
}

function clipUniforms(): Clip {
  const on = uniform(0);
  const n = uniform(new THREE.Vector3(1, 0, 0));
  const d = uniform(0);
  // rcfd-render's shader discards where `dot(u.clip.xyz, v.world) > u.clip.w`; this keeps the rest
  const mask = on.lessThan(float(0.5)).or(dot(vec3(n), positionWorld).lessThanEqual(d));
  return {
    on: on as unknown as { value: number },
    n: n as unknown as { value: THREE.Vector3 },
    d: d as unknown as { value: number },
    mask: mask as unknown as ReturnType<typeof dot>,
  };
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

interface SceneProps extends Omit<FieldCanvasProps, 'forceWebGL' | 'onBackend'> {
  readonly apiRef: Ref<FieldCanvasApi>;
}

let gridIds = 0;

function Scene({ surface, values, valuesPer = 'cell', range, grid, onMoved, onView, onDrawn, onGridState, cut = null, arrows = null, apiRef, prepared, scalars, engineCut = null, engineArrows = null, iso = null, streamlines = null, onPictureDrawn, thin = null }: SceneProps) {
  // F-832: the arrays made off the main thread, when they belong to this surface
  const prep = prepared && prepared.surface === surface ? prepared : null;
  const box = prep ? { min: prep.min, max: prep.max } : undefined;
  const boxRef = useRef(box);
  boxRef.current = box;
  const { gl, camera, scene, invalidate, size, set } = useThree();
  const renderer = gl as unknown as THREE.WebGPURenderer;
  // Addendum 22: the canvas's own perspective camera (3D), and an orthographic one for a 2D case; both follow the
  // controller of ./camera.ts (`view`), which owns the eye, orientation, pivot and projection
  const [persp] = useState(() => camera as THREE.PerspectiveCamera);
  const ortho = useMemo(() => {
    const o = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 100);
    // react-three-fiber leaves a manual camera's projection alone on a resize (this file sets it)
    (o as unknown as { manual: boolean }).manual = true;
    return o;
  }, []);
  const cam: THREE.PerspectiveCamera | THREE.OrthographicCamera = thin !== null ? ortho : persp;
  const view = useMemo(() => new ViewCamera(persp.fov), [persp]);
  useEffect(() => {
    set({ camera: cam });
  }, [cam, set]);
  const lut = useMemo(lutTexture, []);
  const clip = useMemo(clipUniforms, []);
  // ADR 0738: the surface is not masked: a cut surface is the engine's (engineCut), drawn in its place
  const mat = useMemo(() => fieldMaterial(lut), [lut]);
  const mesh = useMemo(() => new THREE.Mesh(new THREE.BufferGeometry(), mat.material), [mat]);
  // F-818: the overlays drawn on the field (mesh grid, arrows) keep DESIGN.md's light values in every theme: they sit
  // on the colour map, which is the same in every theme, not on the theme's ground
  const lines = useMemo(() => {
    const m = new THREE.LineBasicNodeMaterial({ color: raw(palette.white), transparent: true, opacity: 0.38, depthWrite: false });
    m.maskNode = clip.mask;
    const l = new THREE.LineSegments(new THREE.BufferGeometry(), m);
    l.visible = false;
    return l;
  }, [clip]);
  // F-809: the vector arrows, deep ink like the other annotations of the field (DESIGN.md Field view), opaque
  const arrowLines = useMemo(() => {
    const m = new THREE.LineBasicNodeMaterial({ color: raw(palette.ink) });
    m.maskNode = clip.mask;
    const l = new THREE.LineSegments(new THREE.BufferGeometry(), m);
    l.visible = false;
    return l;
  }, [clip]);
  // ADR 0738: the engine's pictures, unmasked (the engine cut them): its cut surface and iso-surface in the field's
  // table, its arrows in deep ink, its streamlines in deep ink drawn over the surface (they run inside it)
  const engineMat = useMemo(() => fieldMaterial(lut), [lut]);
  const cutMesh = useMemo(() => {
    const m = new THREE.Mesh(new THREE.BufferGeometry(), engineMat.material);
    m.visible = false;
    return m;
  }, [engineMat]);
  const isoMesh = useMemo(() => {
    const m = new THREE.Mesh(new THREE.BufferGeometry(), engineMat.material);
    m.visible = false;
    return m;
  }, [engineMat]);
  const engineArrowLines = useMemo(() => {
    const l = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicNodeMaterial({ color: raw(palette.ink) }));
    l.visible = false;
    return l;
  }, []);
  const streamLines = useMemo(() => {
    const l = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicNodeMaterial({ color: raw(palette.ink), depthTest: false }));
    l.renderOrder = 2;
    l.visible = false;
    return l;
  }, []);
  // the stored surface is hidden while the engine's cut or iso-surface stands in for it
  const surfaceHidden = useRef(false);
  surfaceHidden.current = !!engineCut || !!iso;
  // the surface's values are in (set by the values effect, cleared by a new surface)
  const surfaceReady = useRef(false);
  const engineCutRef = useRef(engineCut);
  engineCutRef.current = engineCut;
  // F-818: the viewport's ground follows the theme, drawn again at once
  useEffect(
    () =>
      onThemeChange(() => {
        renderer.setClearColor(raw(themeHex('silver-wash')), 1);
        invalidate();
      }),
    [renderer, invalidate],
  );
  const report = useRef(onMoved);
  report.current = onMoved;
  const viewCb = useRef(onView);
  viewCb.current = onView;
  const viewed = () => viewCb.current?.();
  // whether the user has moved the view since it was fitted (a resize refits an unmoved view)
  const userMoved = useRef(false);
  const moved = useRef((m: boolean) => {
    userMoved.current = m;
    report.current(m);
  });
  const camRef = useRef(cam);
  camRef.current = cam;
  /** The P8 bench drives the camera itself: no input meanwhile. */
  const benching = useRef(false);
  /** The controller's state onto the three.js camera, its data attributes on the canvas, and a new frame. */
  const apply = useRef(() => {});
  apply.current = () => {
    const c = camRef.current;
    c.position.set(...view.eye);
    c.quaternion.set(...view.rot);
    const { near, far } = view.clip;
    c.near = near;
    c.far = far;
    if (c instanceof THREE.OrthographicCamera) {
      const h = view.height / 2;
      c.left = -h * view.aspect;
      c.right = h * view.aspect;
      c.top = h;
      c.bottom = -h;
      c.zoom = 1;
    } else {
      c.fov = view.fov;
      c.aspect = view.aspect;
    }
    c.updateProjectionMatrix();
    c.updateMatrixWorld();
    Object.assign(renderer.domElement.dataset, cameraAttributes(view, userMoved.current));
    invalidate();
    viewed();
  };
  const aspect = size.width / Math.max(1, size.height);
  const aspectRef = useRef(aspect);
  aspectRef.current = aspect;
  const gridFor = useRef<SurfaceData | null>(null);

  useEffect(() => {
    scene.add(cutMesh, isoMesh, engineArrowLines, streamLines);
    return () => {
      scene.remove(cutMesh, isoMesh, engineArrowLines, streamLines);
      for (const o of [cutMesh, isoMesh, engineArrowLines, streamLines]) o.geometry.dispose();
      engineMat.material.dispose();
      engineArrowLines.material.dispose();
      streamLines.material.dispose();
    };
  }, [scene, cutMesh, isoMesh, engineArrowLines, streamLines, engineMat]);

  useEffect(() => {
    scene.add(mesh, lines, arrowLines);
    return () => {
      scene.remove(mesh, lines, arrowLines);
      mesh.geometry.dispose();
      lines.geometry.dispose();
      arrowLines.geometry.dispose();
      mat.material.dispose();
      lut.dispose();
    };
  }, [scene, mesh, lines, arrowLines, mat, lut]);

  // F-809: the cut plane's uniforms (rcfd-render's (n, d))
  useEffect(() => {
    if (cut) {
      const { n, d } = clipPlane(cut);
      clip.n.value.set(n[0], n[1], n[2]);
      clip.d.value = d;
      clip.on.value = 1;
    } else clip.on.value = 0;
    invalidate();
  }, [cut, clip, invalidate]);

  // F-809: the vector arrows
  useEffect(() => {
    arrowLines.geometry.dispose();
    const g = new THREE.BufferGeometry();
    if (arrows && arrows.length > 0) g.setAttribute('position', new THREE.BufferAttribute(arrows, 3));
    arrowLines.geometry = g;
    arrowLines.visible = !!arrows && arrows.length > 0;
    invalidate();
  }, [arrows, arrowLines, invalidate]);

  // Addendum 22: the point of the drawn picture under a point of the view (normalised device coordinates), or null: the
  // engine's cut surface or iso-surface where one stands in for the surface, else the surface
  const hitAt = useRef<(x: number, y: number) => Vec3 | null>(() => null);
  hitAt.current = (x, y) => {
    const rc = new THREE.Raycaster();
    rc.setFromCamera(new THREE.Vector2(x, y), camRef.current);
    const shown = [cutMesh, isoMesh, mesh].filter((o) => o.visible);
    const hit = rc.intersectObjects(shown, false)[0];
    return hit ? [hit.point.x, hit.point.y, hit.point.z] : null;
  };

  // Addendum 22: the pointer and the fly keys (Unity's scene view). Hold the right button: look, and W A S D, Q and E fly
  // (Shift faster); the middle button pans; Alt and the left button orbit about the point under the cursor (else the
  // pivot); the wheel dollies toward the point under the cursor; a double-click puts the pivot there. A 2D case pans with
  // the middle or right button and zooms with the wheel. The plain left button is the readout's (FieldViewport).
  useEffect(() => {
    const el = renderer.domElement;
    let drag: { mode: 'look' | 'pan' | 'orbit'; id: number; x: number; y: number } | null = null;
    const held = new Set<string>();
    let fast = false;
    let raf = 0;
    let last = 0;
    const ndc = (e: { clientX: number; clientY: number }): [number, number] => {
      const r = el.getBoundingClientRect();
      return [((e.clientX - r.left) / Math.max(1, r.width)) * 2 - 1, -(((e.clientY - r.top) / Math.max(1, r.height)) * 2 - 1)];
    };
    const changed = () => {
      if (!userMoved.current) moved.current(true);
      apply.current();
    };
    const loop = (t: number) => {
      raf = 0;
      if (!drag || drag.mode !== 'look' || held.size === 0) return;
      const dt = Math.min(0.1, Math.max(0, (t - last) / 1000));
      last = t;
      if (view.fly(flyMove(held), dt, fast)) changed();
      raf = requestAnimationFrame(loop);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      fast = e.shiftKey;
      if (!FLY_KEYS.has(e.code)) return;
      e.preventDefault();
      e.stopPropagation();
      if (!held.has(e.code)) {
        held.add(e.code);
        if (!raf) {
          last = performance.now();
          raf = requestAnimationFrame(loop);
        }
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      fast = e.shiftKey;
      if (!FLY_KEYS.has(e.code)) return;
      e.preventDefault();
      e.stopPropagation();
      held.delete(e.code);
    };
    // Q-A22-2 (the maintainer, 2026-11-17): Alt used for an orbit drag does not also move the focus to the window's menu
    // bar when it is let go. Electron toggles the menu bar on an Alt key-up the page did not handle, so the page handles
    // (prevents) that one key-up; a plain Alt press and release still reaches the menu.
    let altDragged = false;
    const onAlt = (e: KeyboardEvent) => {
      if (e.key !== 'Alt') return;
      if (e.type === 'keydown') {
        if (!e.repeat) altDragged = false;
      } else if (altDragged) {
        altDragged = false;
        e.preventDefault();
        el.dataset['cameraAltSwallowed'] = String(Number(el.dataset['cameraAltSwallowed'] ?? '0') + 1);
      }
    };
    const endFly = () => {
      held.clear();
      cancelAnimationFrame(raf);
      raf = 0;
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
    };
    const onDown = (e: PointerEvent) => {
      if (benching.current || drag) return;
      let mode: 'look' | 'pan' | 'orbit' | null = null;
      if (view.ortho) mode = e.button === 1 || e.button === 2 ? 'pan' : null;
      else if (e.button === 2) mode = 'look';
      else if (e.button === 1) mode = 'pan';
      else if (e.button === 0 && e.altKey) mode = 'orbit';
      if (!mode) return;
      e.preventDefault();
      if (mode === 'orbit') {
        altDragged = true;
        const p = hitAt.current(...ndc(e));
        if (p) view.setPivot(p);
      }
      drag = { mode, id: e.pointerId, x: e.clientX, y: e.clientY };
      el.setPointerCapture(e.pointerId);
      el.dataset['cameraDrag'] = mode;
      if (mode === 'look') {
        window.addEventListener('keydown', onKeyDown, true);
        window.addEventListener('keyup', onKeyUp, true);
      }
    };
    const onMove = (e: PointerEvent) => {
      if (!drag || e.pointerId !== drag.id) return;
      const dx = e.clientX - drag.x;
      const dy = e.clientY - drag.y;
      drag.x = e.clientX;
      drag.y = e.clientY;
      if (dx === 0 && dy === 0) return;
      if (drag.mode === 'look') view.look(dx, dy);
      else if (drag.mode === 'orbit') view.orbit(dx, dy);
      else view.pan(dx, dy, el.clientHeight);
      changed();
    };
    const onUp = (e: PointerEvent) => {
      if (!drag || e.pointerId !== drag.id) return;
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      if (drag.mode === 'look') endFly();
      drag = null;
      delete el.dataset['cameraDrag'];
    };
    const onWheel = (e: WheelEvent) => {
      if (benching.current) return;
      e.preventDefault();
      const px = e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1);
      const [x, y] = ndc(e);
      view.wheel(px, x, y, hitAt.current(x, y));
      changed();
    };
    const onDouble = (e: MouseEvent) => {
      if (benching.current || e.button !== 0) return;
      const p = hitAt.current(...ndc(e));
      if (!p) return;
      view.setPivot(p);
      apply.current();
    };
    const noMenu = (e: Event) => e.preventDefault();
    // the middle button's autoscroll is the browser's default of its mousedown
    const noAutoscroll = (e: MouseEvent) => {
      if (e.button === 1) e.preventDefault();
    };
    el.addEventListener('pointerdown', onDown);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerup', onUp);
    el.addEventListener('pointercancel', onUp);
    el.addEventListener('wheel', onWheel, { passive: false });
    el.addEventListener('dblclick', onDouble);
    el.addEventListener('contextmenu', noMenu);
    el.addEventListener('mousedown', noAutoscroll);
    window.addEventListener('keydown', onAlt, true);
    window.addEventListener('keyup', onAlt, true);
    return () => {
      endFly();
      window.removeEventListener('keydown', onAlt, true);
      window.removeEventListener('keyup', onAlt, true);
      el.removeEventListener('pointerdown', onDown);
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerup', onUp);
      el.removeEventListener('pointercancel', onUp);
      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('dblclick', onDouble);
      el.removeEventListener('contextmenu', noMenu);
      el.removeEventListener('mousedown', noAutoscroll);
    };
  }, [renderer, view]);

  // the surface: new geometry, fitted view
  useEffect(() => {
    lines.visible = false;
    gridFor.current = null;
    if (!surface) {
      mesh.visible = false;
      surfaceReady.current = false;
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
    surfaceReady.current = false;
    view.aspect = aspectRef.current;
    view.setDomain(boxRef.current ?? bounds(surface.points), thin);
    moved.current(false);
    apply.current();
    // `prep` is made from `surface` (a new surface comes with its own); it is read when the surface is set; a 2D
    // case's thin axis comes with its surface
  }, [surface, thin, mesh, lines, invalidate]);

  // the values and the range, then the first picture of them
  useEffect(() => {
    if (!surface || !values || !range) return;
    const a = mesh.geometry.getAttribute('scalar') as THREE.BufferAttribute;
    if (scalars && scalars.length === a.array.length) (a.array as Float32Array).set(scalars);
    else if (valuesPer === 'triangle') cornerScalarsOf(values, a.array as Float32Array<ArrayBuffer>);
    else cornerScalars(surface.cells, values, a.array as Float32Array<ArrayBuffer>);
    a.needsUpdate = true;
    mat.lo.value = range.loF32;
    mat.inv.value = range.invF32;
    surfaceReady.current = true;
    mesh.visible = !surfaceHidden.current;
    let live = true;
    renderer.render(scene, cam);
    void finished(renderer)
      .then(nextFrame)
      .then(() => {
        if (live) onDrawn?.(values);
      });
    invalidate();
    return () => {
      live = false;
    };
    // onDrawn is read when the picture is done; a new callback alone does not redraw
  }, [surface, values, valuesPer, scalars, range, mesh, mat, renderer, scene, cam, invalidate]);

  // ADR 0738: the engine's pictures. The cut surface's corners take the scalar of their source triangle (the values
  // effect above wrote the surface's), so it is coloured as the surface it was cut from; the iso-surface's corners take
  // the engine's value (its level) through the same range.
  useEffect(() => {
    const replace = (o: THREE.Mesh | THREE.LineSegments, g: THREE.BufferGeometry) => {
      const old = o.geometry;
      o.geometry = g;
      old.dispose();
    };
    const drawable = !!surface && !!values && !!range;
    if (range) {
      engineMat.lo.value = range.loF32;
      engineMat.inv.value = range.invF32;
    }
    if (engineCut && drawable) {
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.BufferAttribute(engineCut.positions, 3));
      const main = mesh.geometry.getAttribute('scalar') as THREE.BufferAttribute | undefined;
      const sc = new Float32Array(3 * engineCut.source.length);
      if (main) {
        const from = main.array as Float32Array;
        for (let i = 0; i < engineCut.source.length; i++) sc.fill(from[3 * engineCut.source[i]!]!, 3 * i, 3 * i + 3);
      }
      g.setAttribute('scalar', new THREE.BufferAttribute(sc, 1));
      g.computeBoundingSphere();
      replace(cutMesh, g);
    }
    cutMesh.visible = !!engineCut && drawable && !iso;
    if (iso && drawable) {
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.BufferAttribute(iso.positions, 3));
      g.setAttribute('scalar', new THREE.BufferAttribute(iso.values, 1));
      g.computeBoundingSphere();
      replace(isoMesh, g);
    }
    isoMesh.visible = !!iso && drawable;
    const lineGeometry = (seg: Float32Array<ArrayBuffer> | null) => {
      const g = new THREE.BufferGeometry();
      if (seg && seg.length > 0) g.setAttribute('position', new THREE.BufferAttribute(seg, 3));
      return g;
    };
    replace(engineArrowLines, lineGeometry(engineArrows));
    engineArrowLines.visible = !!engineArrows && engineArrows.length > 0 && drawable;
    replace(streamLines, lineGeometry(streamlines));
    streamLines.visible = !!streamlines && streamlines.length > 0 && drawable;
    // the surface goes while the engine's picture stands in for it (it is shown once its values are in)
    mesh.visible = surfaceReady.current && !surfaceHidden.current;
    const picture = engineCut ?? iso ?? engineArrows ?? streamlines;
    let live = true;
    renderer.render(scene, cam);
    void finished(renderer)
      .then(nextFrame)
      .then(() => {
        if (live) onPictureDrawn?.(drawable ? picture : null);
      });
    invalidate();
    return () => {
      live = false;
    };
    // onPictureDrawn is read when the picture is done; `values` and `scalars` change the cut surface's colours
  }, [engineCut, engineArrows, iso, streamlines, surface, values, scalars, range, mesh, cutMesh, isoMesh, engineArrowLines, streamLines, engineMat, renderer, scene, cam, invalidate]);

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
    view.aspect = aspect;
    // a resize frames an unmoved view again
    if (surface && !userMoved.current) view.reset();
    apply.current();
  }, [aspect, cam, view, surface]);

  useImperativeHandle(
    apiRef,
    (): FieldCanvasApi => {
      const raycaster = new THREE.Raycaster();
      const pickNdc = (x: number, y: number): Pick | null => {
        if (!surface || !values) return null;
        raycaster.setFromCamera(new THREE.Vector2(x, y), cam);
        let triangle: number;
        let hit: THREE.Intersection | undefined;
        const ec = engineCutRef.current;
        if (ec && cutMesh.visible) {
          // ADR 0738: the engine's cut surface; the readout names the surface triangle the hit piece was cut from
          hit = raycaster.intersectObject(cutMesh, false)[0];
          if (!hit || hit.faceIndex == null) return null;
          const t = ec.source[hit.faceIndex];
          if (t === undefined) return null;
          triangle = t;
        } else {
          if (!mesh.visible) return null;
          hit = raycaster.intersectObject(mesh, false)[0];
          if (!hit || hit.faceIndex == null) return null;
          triangle = hit.faceIndex;
        }
        const cell = surface.cells[triangle];
        if (cell === undefined) return null;
        // F-819b: surface values are the owner cell's value at the triangle's index
        return { triangle, cell, value: (valuesPer === 'triangle' ? values[triangle] : values[cell])!, point: [hit.point.x, hit.point.y, hit.point.z] };
      };
      const changed = () => {
        if (!userMoved.current) moved.current(true);
        apply.current();
      };
      const reset = () => {
        if (!surface) return;
        view.reset();
        moved.current(false);
        apply.current();
      };
      return {
        reset,
        frame() {
          if (!surface) return;
          view.frame();
          changed();
        },
        faceOn() {
          if (!surface) return;
          view.faceOn();
          changed();
        },
        faceScale() {
          const el = renderer.domElement;
          const f = view.faceScale(el.clientHeight);
          return f ? { ...f, widthPx: el.clientWidth } : null;
        },
        pickAt(clientX, clientY) {
          const r = renderer.domElement.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) return null;
          return pickNdc(((clientX - r.left) / r.width) * 2 - 1, -(((clientY - r.top) / r.height) * 2 - 1));
        },
        pickCentre: () => pickNdc(0, 0),
        key(key) {
          if (!surface) return false;
          // a 2D case pans with the arrows (a twentieth of the view's height a key); 3D orbits ORBIT_KEY_RAD
          const el = renderer.domElement;
          const step = el.clientHeight / 20;
          const arrow = (dx: number, dy: number) => {
            if (view.ortho) view.pan(dx * step, dy * step, el.clientHeight);
            else view.orbit(-dx, dy, ORBIT_KEY_RAD);
            changed();
            return true;
          };
          // a letter as the shortcut table spells it (upper case)
          switch (key.length === 1 ? key.toUpperCase() : key) {
            case 'ArrowLeft':
              return arrow(1, 0);
            case 'ArrowRight':
              return arrow(-1, 0);
            case 'ArrowUp':
              return arrow(0, 1);
            case 'ArrowDown':
              return arrow(0, -1);
            case '+':
            case '=':
              view.zoom(1 / ZOOM_KEY);
              changed();
              return true;
            case '-':
              view.zoom(ZOOM_KEY);
              changed();
              return true;
            case 'F':
              view.frame();
              changed();
              return true;
            case 'O':
              view.faceOn();
              changed();
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
          // the bench drives the perspective camera itself (z up), with the input off; then the view is reset
          benching.current = true;
          const target = new THREE.Vector3(...view.pivot);
          const start = cam.position.clone().sub(target);
          cam.up.set(0, 0, 1);
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
          benching.current = false;
          reset();
          return { triangles: surface ? surface.cells.length : 0, uncapped, frameMs: out, workMs: work, emptyFrameMs: empty };
        },
      };
    },
    [cam, view, mesh, cutMesh, surface, values, valuesPer, renderer, scene],
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
        drawOnDemand(renderer); // F-817b: no frames asked for at rest (frameloop="demand" draws; three's own loop stops)
        renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
        renderer.toneMapping = THREE.NoToneMapping;
        renderer.setClearColor(raw(themeHex('silver-wash')), 1);
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
