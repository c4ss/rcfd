import { useEffect, useMemo, useRef } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import * as THREE from 'three/webgpu';
import { attribute, texture, uniform, vec2, clamp } from 'three/tsl';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { DOMAIN, U_MAX, velocity, velocityMagnitude } from './data';
import { viridisLUT } from './colormap';

export type ViewMode = 'surface' | 'iso' | 'streamlines';

interface Props {
  time: number;
  mode: ViewMode;
  meshGrid: boolean;
  resetKey: number;
  onMoved: () => void;
}

const NX = 330;
const NY = 120;
const INK = new THREE.Color('#1D3A5F');

function sampleScalar(geom: THREE.BufferGeometry, t: number) {
  const pos = geom.getAttribute('position') as THREE.BufferAttribute;
  const sc = geom.getAttribute('scalar') as THREE.BufferAttribute;
  const arr = sc.array as Float32Array;
  for (let i = 0; i < pos.count; i++) {
    arr[i] = velocityMagnitude(pos.getX(i) + DOMAIN.W / 2, pos.getY(i) + DOMAIN.H / 2, t);
  }
  sc.needsUpdate = true;
}

/** Marching squares over the vertex grid: iso-lines of |U| at fixed levels. */
function isoLines(geom: THREE.BufferGeometry, levels: number[]): Float32Array {
  const pos = geom.getAttribute('position') as THREE.BufferAttribute;
  const s = (geom.getAttribute('scalar') as THREE.BufferAttribute).array as Float32Array;
  const out: number[] = [];
  const W = NX + 1;
  const lerp = (a: number, b: number, va: number, vb: number, l: number) => {
    const f = (l - va) / (vb - va);
    return [pos.getX(a) + f * (pos.getX(b) - pos.getX(a)), pos.getY(a) + f * (pos.getY(b) - pos.getY(a))];
  };
  for (const l of levels) {
    for (let j = 0; j < NY; j++) {
      for (let i = 0; i < NX; i++) {
        const a = j * W + i, b = a + 1, c = a + W + 1, d = a + W;
        const corners = [a, b, c, d];
        const pts: number[][] = [];
        for (let e = 0; e < 4; e++) {
          const p = corners[e], q = corners[(e + 1) % 4];
          if ((s[p] - l) * (s[q] - l) < 0) pts.push(lerp(p, q, s[p], s[q], l));
        }
        if (pts.length >= 2) out.push(pts[0][0], pts[0][1], 0.002, pts[1][0], pts[1][1], 0.002);
        if (pts.length === 4) out.push(pts[2][0], pts[2][1], 0.002, pts[3][0], pts[3][1], 0.002);
      }
    }
  }
  return new Float32Array(out);
}

function streamlines(t: number): Float32Array {
  const out: number[] = [];
  const seeds = 22;
  for (let k = 0; k < seeds; k++) {
    let x = 0.01, y = 0.03 + (k / (seeds - 1)) * (DOMAIN.H - 0.06);
    for (let step = 0; step < 520; step++) {
      const [u1, v1] = velocity(x, y, t);
      const h = 0.0045 / Math.max(0.05, Math.hypot(u1, v1));
      const [u2, v2] = velocity(x + 0.5 * h * u1, y + 0.5 * h * v1, t);
      const nx = x + h * u2, ny = y + h * v2;
      if (nx > DOMAIN.W || ny < 0 || ny > DOMAIN.H || Math.hypot(u2, v2) < 1e-3) break;
      out.push(x - DOMAIN.W / 2, y - DOMAIN.H / 2, 0.003, nx - DOMAIN.W / 2, ny - DOMAIN.H / 2, 0.003);
      x = nx; y = ny;
    }
  }
  return new Float32Array(out);
}

function Scene({ time, mode, meshGrid, resetKey, onMoved }: Props) {
  const { camera, gl, invalidate, size } = useThree();

  const field = useMemo(() => {
    const geom = new THREE.PlaneGeometry(DOMAIN.W, DOMAIN.H, NX, NY);
    geom.setAttribute('scalar', new THREE.BufferAttribute(new Float32Array(geom.getAttribute('position').count), 1));
    const lut = viridisLUT();
    const uMax = uniform(U_MAX);
    const mat = new THREE.MeshBasicNodeMaterial();
    // The colour comes from the raw scalar on the GPU: value / U_MAX looked up in a 256-entry viridis table.
    mat.colorNode = texture(lut, vec2(clamp(attribute('scalar', 'float').div(uMax), 0, 1), 0.5)).rgb;
    return new THREE.Mesh(geom, mat);
  }, []);

  const body = useMemo(() => {
    const m = new THREE.Mesh(new THREE.CircleGeometry(DOMAIN.R, 64), new THREE.MeshBasicNodeMaterial({ color: INK }));
    m.position.set(DOMAIN.CX - DOMAIN.W / 2, DOMAIN.CY - DOMAIN.H / 2, 0.004);
    return m;
  }, []);

  const frame = useMemo(() => {
    const w = DOMAIN.W / 2, h = DOMAIN.H / 2;
    const g = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-w, -h, 0.005), new THREE.Vector3(w, -h, 0.005), new THREE.Vector3(w, h, 0.005),
      new THREE.Vector3(-w, h, 0.005), new THREE.Vector3(-w, -h, 0.005),
    ]);
    return new THREE.Line(g, new THREE.LineBasicNodeMaterial({ color: INK }));
  }, []);

  const grid = useMemo(() => {
    const pts: number[] = [];
    const gx = 88, gy = 32, w = DOMAIN.W, h = DOMAIN.H;
    for (let i = 0; i <= gx; i++) { const x = -w / 2 + (i / gx) * w; pts.push(x, -h / 2, 0.001, x, h / 2, 0.001); }
    for (let j = 0; j <= gy; j++) { const y = -h / 2 + (j / gy) * h; pts.push(-w / 2, y, 0.001, w / 2, y, 0.001); }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
    const m = new THREE.LineBasicNodeMaterial({ color: new THREE.Color('#FFFFFF'), transparent: true, opacity: 0.38 });
    return new THREE.LineSegments(g, m);
  }, []);

  const overlay = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute([], 3));
    return new THREE.LineSegments(g, new THREE.LineBasicNodeMaterial({ color: new THREE.Color('#FFFFFF') }));
  }, []);

  useEffect(() => {
    sampleScalar(field.geometry, time);
    const pts = mode === 'iso' ? isoLines(field.geometry, [0.3, 0.6, 0.9, 1.2, 1.5])
      : mode === 'streamlines' ? streamlines(time) : new Float32Array();
    overlay.geometry.setAttribute('position', new THREE.BufferAttribute(pts, 3));
    overlay.geometry.computeBoundingSphere();
    (overlay.material as THREE.LineBasicNodeMaterial).color = new THREE.Color(mode === 'iso' ? '#1D3A5F' : '#FFFFFF');
    overlay.visible = mode !== 'surface';
    invalidate();
  }, [time, mode, field, overlay, invalidate]);

  useEffect(() => { grid.visible = meshGrid; invalidate(); }, [meshGrid, grid, invalidate]);

  // Fit the domain to the frame (the frame keeps the domain's aspect, 1.04 W by 1.12 H) on resize and on reset.
  const controlsRef = useRef<OrbitControls | null>(null);
  useEffect(() => {
    const cam = camera as THREE.OrthographicCamera;
    cam.position.set(0, 0, 5);
    cam.up.set(0, 1, 0);
    cam.lookAt(0, 0, 0);
    cam.zoom = Math.min(size.width / (DOMAIN.W * 1.04), size.height / (DOMAIN.H * 1.12));
    cam.updateProjectionMatrix();
    controlsRef.current?.target.set(0, 0, 0);
    invalidate();
  }, [camera, size, invalidate, resetKey]);

  const moved = useRef(onMoved);
  moved.current = onMoved;
  useEffect(() => {
    const controls = new OrbitControls(camera, gl.domElement);
    controlsRef.current = controls;
    controls.enableDamping = false;
    controls.screenSpacePanning = true;
    controls.addEventListener('start', () => moved.current());
    controls.addEventListener('change', () => invalidate());
    return () => controls.dispose();
  }, [camera, gl, invalidate]);

  return (
    <>
      <primitive object={field} />
      <primitive object={grid} />
      <primitive object={overlay} />
      <primitive object={body} />
      <primitive object={frame} />
    </>
  );
}

export function FieldView(props: Props) {
  return (
    <div className="field-canvas">
      <Canvas
        orthographic
        frameloop="demand"
        camera={{ position: [0, 0, 5], near: 0.1, far: 50 }}
        gl={async (defaults) => {
          const renderer = new THREE.WebGPURenderer({ canvas: defaults.canvas as HTMLCanvasElement, antialias: true });
          await renderer.init();
          renderer.setClearColor(new THREE.Color('#EEF1F4'));
          return renderer as any; // R3F types expect a WebGLRenderer; the WebGPU renderer is accepted at runtime
        }}
      >
        <Scene {...props} />
      </Canvas>
    </div>
  );
}
