// The field viewport's camera (ADR 0801 Addendum 22; src/renderer/field/camera.ts) as pure maths, without a window: look,
// fly, orbit about a pivot with no pole flip, dolly toward a point, pan, face on (the distance and the centre kept),
// frame and reset (the whole domain in the view), the orthographic 2D case (pan and zoom only), and Play's pace
// (src/renderer/workspace/model.ts, one stored snapshot per second).
import { expect, test } from '@playwright/test';
import {
  FLY_DIAGONALS_PER_S,
  FLY_FAST,
  FRAME_MARGIN,
  ORBIT_KEY_RAD,
  THREE_QUARTER,
  ViewCamera,
  add,
  cameraAttributes,
  dot,
  faceOnRotation,
  flyMove,
  length,
  lookRotation,
  rotate,
  scale,
  sub,
  type Vec3,
} from '../../src/renderer/field/camera.js';
import { PLAY_SAMPLE_MS, PLAY_SNAPSHOT_MS, PLAY_SPEEDS, playWait, speedText, speedWords } from '../../src/renderer/workspace/model.js';

const BOX = { min: [0, 0, 0], max: [6, 5, 4] };
const CENTRE: Vec3 = [3, 2.5, 2];
const RADIUS = 0.5 * Math.hypot(6, 5, 4);
/** A 2D case: 1 x 1 in x and y, one cell (0.01) thick in z. */
const FLAT = { min: [0, 0, 0], max: [1, 1, 0.01] };

const close = (a: readonly number[], b: readonly number[], eps = 1e-9) => {
  expect(a.length).toBe(b.length);
  for (let i = 0; i < a.length; i++) expect(Math.abs(a[i]! - b[i]!), `component ${i}: ${a} vs ${b}`).toBeLessThan(eps);
};

/** Where a world point lands on the view, in normalised device coordinates (perspective or orthographic). */
function project(c: ViewCamera, p: Vec3): [number, number] {
  const v = sub(p, c.eye);
  const x = dot(v, c.right);
  const y = dot(v, c.up);
  const z = dot(v, c.forward);
  if (c.ortho) return [x / ((c.height / 2) * c.aspect), y / (c.height / 2)];
  const t = Math.tan((c.fov * Math.PI) / 360);
  return [x / (z * t * c.aspect), y / (z * t)];
}

function camera3d(aspect = 1.6): ViewCamera {
  const c = new ViewCamera(30);
  c.aspect = aspect;
  c.setDomain(BOX, null);
  return c;
}

/** Every corner of the box is inside the view (NDC within 1) and ahead of the camera. */
function framed(c: ViewCamera, box = BOX): number {
  let worst = 0;
  for (const x of [box.min[0]!, box.max[0]!])
    for (const y of [box.min[1]!, box.max[1]!])
      for (const z of [box.min[2]!, box.max[2]!]) {
        const p: Vec3 = [x, y, z];
        if (!c.ortho) expect(dot(sub(p, c.eye), c.forward)).toBeGreaterThan(0);
        const [u, v] = project(c, p);
        worst = Math.max(worst, Math.abs(u), Math.abs(v));
      }
  return worst;
}

test.describe('the controller of the field view (Unity scene-view scheme)', () => {
  test('quaternion helpers: lookRotation looks along forward with up, a right-handed frame', () => {
    const q = lookRotation([0, 1, 0], [0, 0, 1]);
    close(rotate(q, [0, 0, -1]), [0, 1, 0]);
    close(rotate(q, [0, 1, 0]), [0, 0, 1]);
    close(rotate(q, [1, 0, 0]), [1, 0, 0]);
    // the face-on orientations: the other two axes across, the lower one right, the higher one up
    close(rotate(faceOnRotation(2), [1, 0, 0]), [1, 0, 0]);
    close(rotate(faceOnRotation(2), [0, 1, 0]), [0, 1, 0]);
    close(rotate(faceOnRotation(1), [1, 0, 0]), [1, 0, 0]);
    close(rotate(faceOnRotation(1), [0, 1, 0]), [0, 0, 1]);
    close(rotate(faceOnRotation(0), [1, 0, 0]), [0, 1, 0]);
    close(rotate(faceOnRotation(0), [0, 1, 0]), [0, 0, 1]);
  });

  test('reset: the three-quarter view, z up, the whole domain framed, the pivot at its centre', () => {
    const c = camera3d();
    close(c.forward, THREE_QUARTER);
    expect(c.up[2]).toBeGreaterThan(0.5);
    expect(Math.abs(c.right[2])).toBeLessThan(1e-12); // no roll: the right stays level
    close(c.pivot, CENTRE);
    expect(c.ortho).toBe(false);
    const w = framed(c);
    expect(w).toBeLessThan(1);
    expect(w).toBeGreaterThan(0.5);
    // moved anyhow, reset gives the same view back
    const eye = c.eye;
    const rot = c.rot;
    c.look(40, -20);
    c.fly([1, 0, 1], 0.5, false);
    c.orbit(-90, 30);
    c.wheel(-400, 0.3, 0.1, null);
    c.reset();
    close(c.eye, eye);
    close(c.rot, rot);
    close(c.pivot, CENTRE);
  });

  test('look: turns about the eye (the eye stays), the pivot turns with the view at the same depth', () => {
    const c = camera3d();
    const eye = c.eye;
    const depth = c.depth;
    c.look(50, 0);
    close(c.eye, eye);
    expect(Math.abs(c.depth - depth)).toBeLessThan(1e-9);
    close(c.pivot, add(c.eye, scale(c.forward, depth)), 1e-9);
    // a drag to the right looks to the right: the new forward leans toward the old right
    const before = camera3d();
    expect(dot(c.forward, before.right)).toBeGreaterThan(0);
    // a drag down looks down
    const d = camera3d();
    d.look(0, 50);
    expect(d.forward[2]).toBeLessThan(before.forward[2]);
    // yaw is about the world's up: the right stays level
    expect(Math.abs(c.right[2])).toBeLessThan(1e-12);
  });

  test('fly: W A S D Q E move eye and pivot together, at a speed of the domain size; Shift faster', () => {
    expect(flyMove(new Set(['KeyW']))).toEqual([0, 0, 1]);
    expect(flyMove(new Set(['KeyA', 'KeyQ']))).toEqual([-1, -1, 0]);
    expect(flyMove(new Set(['KeyD', 'KeyE', 'KeyS']))).toEqual([1, 1, -1]);
    const c = camera3d();
    const eye = c.eye;
    const offset = sub(c.pivot, c.eye);
    const f = c.forward;
    c.fly([0, 0, 1], 1, false);
    close(sub(c.eye, eye), scale(f, 2 * RADIUS * FLY_DIAGONALS_PER_S), 1e-9);
    close(sub(c.pivot, c.eye), offset, 1e-9);
    const fast = camera3d();
    fast.fly([0, 0, 1], 1, true);
    expect(length(sub(fast.eye, eye))).toBeCloseTo(FLY_FAST * 2 * RADIUS * FLY_DIAGONALS_PER_S, 9);
    // E is up along the world's z, Q down
    const up = camera3d();
    up.fly([0, 1, 0], 1, false);
    close(sub(up.eye, eye), [0, 0, 2 * RADIUS * FLY_DIAGONALS_PER_S], 1e-9);
    // nothing held: no move
    expect(camera3d().fly([0, 0, 0], 1, false)).toBe(false);
  });

  test('orbit: about the pivot (its distance kept, the pivot stays on screen where it was); no pole flip', () => {
    const c = camera3d();
    const r0 = length(sub(c.eye, c.pivot));
    c.orbit(120, -45);
    expect(length(sub(c.eye, c.pivot))).toBeCloseTo(r0, 9);
    close(c.pivot, CENTRE);
    close(project(c, CENTRE), [0, 0], 1e-9);
    // about another point (Alt-drag on the surface): it becomes the pivot and stays where it is on the view
    const d = camera3d();
    const p: Vec3 = [6, 0.5, 3];
    const at = project(d, p);
    d.orbit(80, 25, undefined, p);
    close(d.pivot, p);
    close(project(d, p), at, 1e-9);
    // over the pole and on: the pitch goes on smoothly (no clamp), the view upside down past it, never a jump
    const e = camera3d();
    let prevF = e.forward;
    let flipped = false;
    for (let i = 0; i < 90; i++) {
      e.orbit(0, 10, ORBIT_KEY_RAD / 10);
      const step = Math.acos(Math.min(1, dot(prevF, e.forward)));
      expect(step).toBeLessThan(ORBIT_KEY_RAD * 1.01);
      prevF = e.forward;
      if (dot(e.up, [0, 0, 1]) < 0) flipped = true;
      expect(Math.abs(e.right[2])).toBeLessThan(1e-9); // the right stays level: no roll
      expect(length(sub(e.eye, e.pivot))).toBeCloseTo(r0, 6);
    }
    expect(flipped, 'past the pole the camera is upside down (no pole lock)').toBe(true);
    // upside down, a drag to the right still moves the near side of the domain to the right on screen, as upright
    const nearSide = (k: ViewCamera): Vec3 => {
      const o = sub(k.eye, k.pivot);
      const h = length([o[0], o[1], 0]);
      return add(k.pivot, [o[0] / h, o[1] / h, 0]);
    };
    const drag = (k: ViewCamera) => {
      const p = nearSide(k);
      const b = project(k, p);
      k.orbit(5, 0);
      return project(k, p)[0] - b[0];
    };
    const over = camera3d();
    over.orbit(0, 1, Math.PI); // half a turn over the top: upside down
    expect(dot(over.up, [0, 0, 1])).toBeLessThan(0);
    expect(drag(over)).toBeGreaterThan(0);
    expect(drag(camera3d())).toBeGreaterThan(0);
  });

  test('the arrow keys orbit ORBIT_KEY_RAD about the pivot', () => {
    const c = camera3d();
    const angle = (v: Vec3) => Math.atan2(v[1], v[0]);
    const a0 = angle(sub(c.eye, c.pivot));
    const z0 = c.eye[2];
    c.orbit(-1, 0, ORBIT_KEY_RAD);
    // ArrowLeft: a yaw of one step about the world's up through the pivot (the height kept)
    expect(angle(sub(c.eye, c.pivot)) - a0).toBeCloseTo(ORBIT_KEY_RAD, 9);
    expect(c.eye[2]).toBeCloseTo(z0, 9);
    close(c.pivot, CENTRE);
    // ArrowUp: the camera rises over the pivot
    c.orbit(0, 1, ORBIT_KEY_RAD);
    expect(c.eye[2]).toBeGreaterThan(z0);
  });

  test('dolly: toward the point under the cursor, which stays where it is on screen; never onto it', () => {
    const c = camera3d();
    const p: Vec3 = [6, 1, 3.5];
    const at = project(c, p);
    const d0 = length(sub(c.eye, p));
    c.wheel(-200, at[0], at[1], p);
    expect(length(sub(c.eye, p))).toBeCloseTo(d0 * Math.pow(1.0015, -200), 9);
    close(project(c, p), at, 1e-9);
    // nothing under the cursor: toward the point of the pivot's plane there, which stays put too
    const e = camera3d();
    const q = e.pivotPlanePoint(0.4, -0.3);
    close(project(e, q), [0.4, -0.3], 1e-9);
    e.wheel(-300, 0.4, -0.3, null);
    close(project(e, q), [0.4, -0.3], 1e-9);
    // out again by the inverse
    const f = camera3d();
    const eye = f.eye;
    f.dollyTo(p, 0.5);
    f.dollyTo(p, 2);
    close(f.eye, eye, 1e-9);
    // a long zoom in stops short of the point
    for (let i = 0; i < 200; i++) f.dollyTo(p, 0.5);
    expect(length(sub(f.eye, p))).toBeGreaterThan(0);
  });

  test('pan: what is under the cursor at the pivot’s depth follows the cursor', () => {
    const c = camera3d();
    const heightPx = 600;
    const p = c.pivot;
    c.pan(60, -30, heightPx);
    // the old pivot moved 60 px right and 30 px up on the view (y up in NDC: +30 px is +0.1)
    const [u, v] = project(c, p);
    expect(u * c.aspect * (heightPx / 2)).toBeCloseTo(60, 6);
    expect(v * (heightPx / 2)).toBeCloseTo(30, 6);
  });

  test('face on: the nearest principal axis, the up the nearest axis; the centre and its distance kept', () => {
    const c = camera3d();
    c.orbit(30, 10);
    c.zoom(0.7);
    const centre = c.centre;
    const depth = c.depth;
    const f0 = c.forward;
    c.faceOn();
    const k = [0, 1, 2].reduce((a, b) => (Math.abs(f0[b]!) > Math.abs(f0[a]!) ? b : a), 0);
    const want: number[] = [0, 0, 0];
    want[k] = Math.sign(f0[k]!);
    close(c.forward, want);
    // the up is an axis across the view, and z (the nearest to the old up here)
    close(c.up, [0, 0, 1]);
    close(c.centre, centre, 1e-9);
    expect(c.depth).toBeCloseTo(depth, 9);
    close(c.pivot, centre, 1e-9);
    // face on again: nothing moves
    const eye = c.eye;
    c.faceOn();
    close(c.eye, eye, 1e-9);
    // the scale bar's reading: straight along an axis
    expect(c.faceScale(600)?.axis).toBe(k);
  });

  test('frame (F): the orientation kept, the whole domain in the view, the pivot at its centre', () => {
    const c = camera3d();
    c.orbit(70, 20);
    c.fly([1, 0, 1], 2, true);
    const rot = c.rot;
    c.frame();
    close(c.rot, rot);
    close(c.pivot, CENTRE);
    expect(framed(c)).toBeLessThan(1);
    // the bounding sphere fits with FRAME_MARGIN in the narrower angle
    const half = Math.min((c.fov * Math.PI) / 360, Math.atan(Math.tan((c.fov * Math.PI) / 360) * c.aspect));
    expect(length(sub(c.eye, CENTRE))).toBeCloseTo((FRAME_MARGIN * RADIUS) / Math.sin(half), 9);
  });

  test('a 2D case: orthographic along the thin axis, framed; pan and zoom only', () => {
    const c = new ViewCamera(30);
    c.aspect = 1.6;
    c.setDomain(FLAT, 2);
    expect(c.ortho).toBe(true);
    close(c.forward, [0, 0, -1]);
    close(c.up, [0, 1, 0]);
    close(c.right, [1, 0, 0]);
    // the whole face in the view: its height is the limit (1 / 1.6 < 1), with the margin
    expect(c.height).toBeCloseTo(FRAME_MARGIN * 1, 12);
    expect(framed(c, FLAT)).toBeLessThan(1);
    expect(framed(c, FLAT)).toBeGreaterThan(0.9);
    expect(c.faceScale(500)).toEqual({ axis: 2, perPixel: c.height / 500 });
    // no look, orbit or fly
    const eye = c.eye;
    const rot = c.rot;
    expect(c.look(50, 20)).toBe(false);
    expect(c.orbit(50, 20)).toBe(false);
    expect(c.fly([0, 0, 1], 1, false)).toBe(false);
    close(c.eye, eye);
    close(c.rot, rot);
    // zoom toward a point: the point stays put on the view, the height shrinks, the direction stays
    const p: Vec3 = [0.8, 0.3, 0.01];
    const at = project(c, p);
    c.wheel(-300, at[0], at[1], p);
    expect(c.height).toBeCloseTo(FRAME_MARGIN * Math.pow(1.0015, -300), 12);
    close(project(c, p), at, 1e-9);
    close(c.rot, rot);
    // pan: the point under the cursor follows it
    const q: Vec3 = [0.5, 0.5, 0];
    const [u0, v0] = project(c, q);
    c.pan(40, -20, 500);
    const [u1, v1] = project(c, q);
    expect((u1 - u0) * c.aspect * 250).toBeCloseTo(40, 6);
    expect((v1 - v0) * 250).toBeCloseTo(20, 6);
    close(c.rot, rot);
    // face on keeps the height and the centre (it is face on already)
    const h = c.height;
    const centre = c.centre;
    c.faceOn();
    expect(c.height).toBe(h);
    close(c.centre, centre, 1e-9);
    // reset: framed again
    c.reset();
    expect(c.height).toBeCloseTo(FRAME_MARGIN, 12);
    close(c.eye, eye, 1e-9);
    // the near and far planes keep the slab in
    const { near, far } = c.clip;
    const depth = dot(sub([0.5, 0.5, 0.005], c.eye), c.forward);
    expect(near).toBeLessThan(depth);
    expect(far).toBeGreaterThan(depth);
  });

  test('a 2D case along y and along x: the in-plane axes across the view', () => {
    const c = new ViewCamera(30);
    c.setDomain({ min: [0, 0, 0], max: [2, 0.05, 1] }, 1);
    close(c.forward, [0, 1, 0]);
    close(c.right, [1, 0, 0]);
    close(c.up, [0, 0, 1]);
    c.setDomain({ min: [0, 0, 0], max: [0.05, 2, 1] }, 0);
    close(c.forward, [-1, 0, 0]);
    close(c.right, [0, 1, 0]);
    close(c.up, [0, 0, 1]);
  });

  test('the data attributes the e2e tests read', () => {
    const c = camera3d();
    const a = cameraAttributes(c, false);
    expect(a['camera']).toBe('perspective');
    expect(a['cameraMoved']).toBe('false');
    expect(a['cameraForward']!.split(' ').map(Number)).toHaveLength(3);
    expect(Number(a['cameraZoom'])).toBeCloseTo(c.depth, 4);
    const f = new ViewCamera(30);
    f.setDomain(FLAT, 2);
    expect(cameraAttributes(f, true)).toMatchObject({ camera: 'orthographic', cameraMoved: 'true', cameraForward: '0.00000 0.00000 -1.00000' });
  });
});

test.describe('Play: one stored snapshot per wall-clock second (ADR 0801 Addendum 22)', () => {
  test('a snapshot stays PLAY_SNAPSHOT_MS from the move to it; a sample PLAY_SAMPLE_MS; reduced motion is a floor', () => {
    expect(PLAY_SNAPSHOT_MS).toBe(1000);
    expect(playWait(true, 0, false, 500)).toBe(1000);
    expect(playWait(true, 400, false, 500)).toBe(600);
    // the field took longer than the second to draw: the next stop at once
    expect(playWait(true, 1300, false, 500)).toBe(0);
    expect(playWait(false, 0, false, 500)).toBe(PLAY_SAMPLE_MS);
    expect(playWait(false, 0, true, 500)).toBe(500);
    expect(playWait(true, 0, true, 500)).toBe(1000);
    expect(playWait(true, -5, false, 500)).toBe(1000);
  });

  test('Q-A22-3: the speeds 0.25x to 4x divide the pace; reduced motion still holds two stops a second', () => {
    expect([...PLAY_SPEEDS]).toEqual([0.25, 0.5, 1, 2, 4]);
    expect(playWait(true, 0, false, 500, 4)).toBe(250);
    expect(playWait(true, 0, false, 500, 2)).toBe(500);
    expect(playWait(true, 0, false, 500, 0.5)).toBe(2000);
    expect(playWait(true, 0, false, 500, 0.25)).toBe(4000);
    expect(playWait(true, 100, false, 500, 4)).toBe(150);
    expect(playWait(false, 0, false, 500, 2)).toBe(PLAY_SAMPLE_MS / 2);
    expect(playWait(true, 0, true, 500, 4)).toBe(500);
    expect(speedText(0.25)).toBe('0.25×');
    expect(speedWords(1)).toBe('Play speed 1×, one snapshot a second');
    expect(speedWords(4)).toBe('Play speed 4×, 4 snapshots a second');
    expect(speedWords(0.25)).toBe('Play speed 0.25×, one snapshot every 4 seconds');
  });
});
