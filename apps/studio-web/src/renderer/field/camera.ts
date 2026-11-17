// The field viewport's camera (F-808; ADR 0801 Addendum 22, the maintainer's choice of 2026-11-17: Unity's scene-view
// scheme), as pure maths with no three.js, DOM or React, so tests/unit/field-camera.spec.ts checks it without a window.
// FieldCanvas.tsx turns pointer and key events into these calls and copies the state onto its three.js camera.
//
// The state is an eye, an orientation (a unit quaternion, camera to world: the camera looks along its -Z, its up is +Y,
// its right +X, as three.js has it), a pivot (the point orbits turn about), and the projection: perspective with a
// vertical field of view, or orthographic with the world height of the view (a 2D case, below).
//
// - look (hold the right button): yaw about the world's up and pitch about the camera's right, about the eye; the pivot
//   turns with the view (it stays the same depth ahead). Fly (the right button held, W A S D, Q and E, Shift faster):
//   eye and pivot move together, at a speed that is a fraction of the domain's size per second.
// - orbit (Alt and the left button, and the arrow keys): the same rotation about the pivot. No pole lock: there is no
//   polar angle to clamp, the rotation is a quaternion; past the pole the view is upside down, and the yaw turns the
//   other way there so that a drag to the right still turns the picture to the right ("up" stays sensible).
// - pan (the middle button; the right button too in 2D): eye and pivot move across the view, the point under the
//   cursor staying under it at the pivot's depth.
// - dolly (the wheel, + and -): toward a point (the one under the cursor), so that point stays where it is on screen; in
//   orthographic the view's height shrinks about that point instead.
// - frame (F): the whole domain in the view, the orientation kept; reset (Home): the starting orientation, framed.
// - faceOn: the nearest principal axis of the view direction (in 2D, the thin axis), the up the nearest axis to the
//   current one, the point at the centre of the view and its distance (or the orthographic height) kept.
//
// A 2D case (one cell thick along `thin`) is drawn orthographic, looking straight along the thin axis, and only pans and
// zooms: look, orbit and fly do nothing there.

export type Vec3 = readonly [number, number, number];
/** A unit quaternion x, y, z, w. */
export type Quat = readonly [number, number, number, number];
export type Axis = 0 | 1 | 2;

export interface Bounds {
  readonly min: readonly number[];
  readonly max: readonly number[];
}

// ---- vector and quaternion maths

export const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
export const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const scale = (a: Vec3, s: number): Vec3 => [a[0] * s, a[1] * s, a[2] * s];
export const dot = (a: Vec3, b: Vec3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
export const length = (a: Vec3): number => Math.hypot(a[0], a[1], a[2]);
export const normalize = (a: Vec3): Vec3 => {
  const l = length(a);
  return l > 0 ? scale(a, 1 / l) : [0, 0, 0];
};
const unit = (axis: Axis, sign = 1): Vec3 => [axis === 0 ? sign : 0, axis === 1 ? sign : 0, axis === 2 ? sign : 0];

export function axisAngle(axis: Vec3, angle: number): Quat {
  const n = normalize(axis);
  const s = Math.sin(angle / 2);
  return [n[0] * s, n[1] * s, n[2] * s, Math.cos(angle / 2)];
}

/** a then b applied to a vector is mul(b, a). */
export function mul(a: Quat, b: Quat): Quat {
  const [ax, ay, az, aw] = a;
  const [bx, by, bz, bw] = b;
  return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz];
}

function normalizeQ(q: Quat): Quat {
  const l = Math.hypot(q[0], q[1], q[2], q[3]) || 1;
  return [q[0] / l, q[1] / l, q[2] / l, q[3] / l];
}

export function rotate(q: Quat, v: Vec3): Vec3 {
  const [x, y, z, w] = q;
  // v + 2w (q x v) + 2 q x (q x v)
  const tx = 2 * (y * v[2] - z * v[1]);
  const ty = 2 * (z * v[0] - x * v[2]);
  const tz = 2 * (x * v[1] - y * v[0]);
  return [v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx)];
}

/** The orientation that looks along `forward` with `up` as near its up as it can be (both need not be unit). */
export function lookRotation(forward: Vec3, up: Vec3): Quat {
  const f = normalize(forward);
  let r = cross(f, up);
  if (length(r) < 1e-9) r = cross(f, Math.abs(f[2]) < 0.9 ? [0, 0, 1] : [0, 1, 0]);
  r = normalize(r);
  const u = cross(r, f);
  const b = scale(f, -1);
  // the rotation matrix with columns right, up, back, as a quaternion
  const m00 = r[0], m01 = u[0], m02 = b[0];
  const m10 = r[1], m11 = u[1], m12 = b[1];
  const m20 = r[2], m21 = u[2], m22 = b[2];
  const tr = m00 + m11 + m22;
  let q: Quat;
  if (tr > 0) {
    const s = 0.5 / Math.sqrt(tr + 1);
    q = [(m21 - m12) * s, (m02 - m20) * s, (m10 - m01) * s, 0.25 / s];
  } else if (m00 > m11 && m00 > m22) {
    const s = 2 * Math.sqrt(1 + m00 - m11 - m22);
    q = [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s];
  } else if (m11 > m22) {
    const s = 2 * Math.sqrt(1 + m11 - m00 - m22);
    q = [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s];
  } else {
    const s = 2 * Math.sqrt(1 + m22 - m00 - m11);
    q = [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s];
  }
  return normalizeQ(q);
}

// ---- the domain

/** The box's centre, its bounding sphere's radius, and its flat axis (an extent below 1e-6 of the largest), if any. */
export function domainOf(b: Bounds): { readonly centre: Vec3; readonly radius: number; readonly extent: Vec3; readonly flat: Axis | null } {
  const extent: Vec3 = [b.max[0]! - b.min[0]!, b.max[1]! - b.min[1]!, b.max[2]! - b.min[2]!];
  const big = Math.max(...extent) || 1;
  const f = extent.findIndex((e) => e < 1e-6 * big);
  return {
    centre: [(b.min[0]! + b.max[0]!) / 2, (b.min[1]! + b.max[1]!) / 2, (b.min[2]! + b.max[2]!) / 2],
    radius: 0.5 * Math.hypot(extent[0], extent[1], extent[2]) || 1,
    extent,
    flat: f < 0 ? null : (f as Axis),
  };
}

/**
 * The face-on orientation along an axis: the other two axes across the view in their order, the lower one to the right
 * and the higher one up (z thin: x right, y up, from +z; y thin: x right, z up, from -y; x thin: y right, z up, from +x).
 */
export function faceOnRotation(axis: Axis): Quat {
  switch (axis) {
    case 2:
      return lookRotation([0, 0, -1], [0, 1, 0]);
    case 1:
      return lookRotation([0, 1, 0], [0, 0, 1]);
    default:
      return lookRotation([-1, 0, 0], [0, 0, 1]);
  }
}

/** The three-quarter view of a 3D domain (z up), as the view was fitted before Addendum 22. */
export const THREE_QUARTER: Vec3 = normalize([-1, 1.6, -1.1]);

// ---- the controller

/** Radians per pixel of a mouse-look drag, and of an orbit drag (and of one arrow key: ORBIT_KEY_RAD). */
export const LOOK_RAD_PER_PX = 0.004;
export const ORBIT_RAD_PER_PX = 0.008;
export const ORBIT_KEY_RAD = (5 * Math.PI) / 180;
/** Fly speed: this fraction of the domain's diagonal per second; Shift multiplies it by FLY_FAST. */
export const FLY_DIAGONALS_PER_S = 0.5;
export const FLY_FAST = 4;
/** One wheel pixel (deltaY) scales the distance to the point (or the orthographic height) by this. */
export const WHEEL_BASE = 1.0015;
/** The + and - keys scale it by this (in by its inverse). */
export const ZOOM_KEY = 1.1;
/** The frame's margin around the domain (the fitted view of F-808). */
export const FRAME_MARGIN = 1.08;

export interface CameraState {
  readonly eye: Vec3;
  readonly rot: Quat;
  readonly pivot: Vec3;
  readonly ortho: boolean;
  /** Orthographic: the world height of the view. */
  readonly height: number;
  /** Perspective: the vertical field of view, degrees. */
  readonly fov: number;
  readonly aspect: number;
}

export interface Ray {
  readonly origin: Vec3;
  readonly dir: Vec3;
}

export class ViewCamera {
  eye: Vec3 = [0, 0, 5];
  rot: Quat = [0, 0, 0, 1];
  pivot: Vec3 = [0, 0, 0];
  height = 1;
  fov: number;
  aspect = 1;
  /** The domain (null before a surface). */
  private box: Bounds | null = null;
  /** The thin axis of a 2D case: orthographic, pan and zoom only. */
  private thinAxis: Axis | null = null;
  readonly worldUp: Vec3 = [0, 0, 1];

  constructor(fov = 30) {
    this.fov = fov;
  }

  get ortho(): boolean {
    return this.thinAxis !== null;
  }
  get thin(): Axis | null {
    return this.thinAxis;
  }
  get forward(): Vec3 {
    return rotate(this.rot, [0, 0, -1]);
  }
  get up(): Vec3 {
    return rotate(this.rot, [0, 1, 0]);
  }
  get right(): Vec3 {
    return rotate(this.rot, [1, 0, 0]);
  }
  get state(): CameraState {
    return { eye: this.eye, rot: this.rot, pivot: this.pivot, ortho: this.ortho, height: this.height, fov: this.fov, aspect: this.aspect };
  }
  /** The domain's size (its bounding sphere's radius; 1 before a surface). */
  get radius(): number {
    return this.box ? domainOf(this.box).radius : 1;
  }
  /** The depth of the pivot ahead of the eye (at least a small part of the domain). */
  get depth(): number {
    return Math.max(dot(sub(this.pivot, this.eye), this.forward), 1e-4 * this.radius);
  }
  /** The point at the centre of the view at the pivot's depth. */
  get centre(): Vec3 {
    return add(this.eye, scale(this.forward, this.depth));
  }
  /** Near and far planes that keep the domain in (and the eye inside it). */
  get clip(): { readonly near: number; readonly far: number } {
    const r = this.radius;
    const c = this.box ? domainOf(this.box).centre : this.pivot;
    const d = length(sub(c, this.eye));
    // orthographic: the eye stays 2 radii before the centre along the view (pan and zoom move it across only)
    if (this.ortho) return { near: r * 1e-2, far: Math.max(dot(sub(c, this.eye), this.forward), 0) + 2 * r };
    return { near: Math.max(r * 1e-3, (d - r) * 0.5), far: d + 4 * r };
  }

  /** A new domain, and whether it is a 2D case (its thin axis): the view is reset to it. */
  setDomain(box: Bounds, thin: Axis | null): void {
    this.box = box;
    this.thinAxis = thin;
    this.reset();
  }

  /** The starting orientation (face on for a 2D or flat domain, else the three-quarter view), framed. */
  reset(): void {
    if (!this.box) return;
    const flat = this.thinAxis ?? domainOf(this.box).flat;
    this.rot = flat !== null ? faceOnRotation(flat) : lookRotation(THREE_QUARTER, this.worldUp);
    this.frame();
  }

  /** The whole domain in the view, the orientation kept; the pivot at the domain's centre. */
  frame(): void {
    if (!this.box) return;
    const { centre, radius, extent } = domainOf(this.box);
    const f = this.forward;
    if (this.ortho) {
      // the box's extents across the view
      const across = (v: Vec3) => Math.abs(v[0]) * extent[0] + Math.abs(v[1]) * extent[1] + Math.abs(v[2]) * extent[2];
      const w = across(this.right);
      const h = across(this.up);
      this.height = FRAME_MARGIN * Math.max(h, w / this.aspect) || 1;
      this.eye = sub(centre, scale(f, 2 * radius));
    } else {
      const half = (this.fov * Math.PI) / 360;
      const fit = Math.min(half, Math.atan(Math.tan(half) * this.aspect));
      this.eye = sub(centre, scale(f, (FRAME_MARGIN * radius) / Math.sin(fit)));
    }
    this.pivot = centre;
  }

  /** World length of one pixel at the pivot's depth, for a view `heightPx` tall. */
  perPixel(heightPx: number): number {
    const h = this.ortho ? this.height : 2 * this.depth * Math.tan((this.fov * Math.PI) / 360);
    return h / Math.max(1, heightPx);
  }

  /** The rotation a drag of (dx, dy) pixels turns the view by: yaw about the world's up, pitch about the camera's right. */
  private turn(dx: number, dy: number, perPx: number): Quat {
    // upside down (past a pole), the yaw turns the other way, so a drag to the right still turns the picture right
    const s = dot(this.up, this.worldUp) < 0 ? -1 : 1;
    return mul(axisAngle(this.worldUp, -dx * perPx * s), axisAngle(this.right, -dy * perPx));
  }

  private turnAbout(q: Quat, centre: Vec3): void {
    this.eye = add(centre, rotate(q, sub(this.eye, centre)));
    this.pivot = add(centre, rotate(q, sub(this.pivot, centre)));
    this.rot = normalizeQ(mul(q, this.rot));
  }

  /** Mouse-look: turns the view about the eye (the pivot turns with it). Nothing in 2D. */
  look(dx: number, dy: number, perPx = LOOK_RAD_PER_PX): boolean {
    if (this.ortho) return false;
    this.turnAbout(this.turn(dx, dy, perPx), this.eye);
    return true;
  }

  /** Orbit: turns the view about the pivot (or `about`, which becomes the pivot). Nothing in 2D. */
  orbit(dx: number, dy: number, perPx = ORBIT_RAD_PER_PX, about?: Vec3): boolean {
    if (this.ortho) return false;
    if (about) this.pivot = about;
    this.turnAbout(this.turn(dx, dy, perPx), this.pivot);
    return true;
  }

  /** Fly: `move` is right, up (the world's), forward, each -1, 0 or 1, for `dt` seconds. Nothing in 2D. */
  fly(move: Vec3, dt: number, fast: boolean): boolean {
    if (this.ortho || (move[0] === 0 && move[1] === 0 && move[2] === 0)) return false;
    const speed = 2 * this.radius * FLY_DIAGONALS_PER_S * (fast ? FLY_FAST : 1) * dt;
    const d = scale(add(add(scale(this.right, move[0]), scale(this.worldUp, move[1])), scale(this.forward, move[2])), speed);
    this.eye = add(this.eye, d);
    this.pivot = add(this.pivot, d);
    return true;
  }

  /** Pan by a drag of (dx, dy) pixels on a view `heightPx` tall: what is under the cursor at the pivot's depth stays. */
  pan(dx: number, dy: number, heightPx: number): void {
    const p = this.perPixel(heightPx);
    const d = add(scale(this.right, -dx * p), scale(this.up, dy * p));
    this.eye = add(this.eye, d);
    this.pivot = add(this.pivot, d);
  }

  /** The ray through a point of the view (normalised device coordinates, -1 to 1, y up). */
  ray(ndcX: number, ndcY: number): Ray {
    const f = this.forward;
    if (this.ortho) {
      const o = add(this.eye, add(scale(this.right, (ndcX * this.height * this.aspect) / 2), scale(this.up, (ndcY * this.height) / 2)));
      return { origin: o, dir: f };
    }
    const t = Math.tan((this.fov * Math.PI) / 360);
    return { origin: this.eye, dir: normalize(add(f, add(scale(this.right, ndcX * t * this.aspect), scale(this.up, ndcY * t)))) };
  }

  /** Where the ray through a point of the view meets the plane through the pivot across the view. */
  pivotPlanePoint(ndcX: number, ndcY: number): Vec3 {
    const { origin, dir } = this.ray(ndcX, ndcY);
    const f = this.forward;
    const t = dot(sub(this.pivot, origin), f) / (dot(dir, f) || 1);
    return add(origin, scale(dir, t));
  }

  /**
   * Dolly toward `point` (perspective): eye and pivot scaled about it by `factor` (below 1 is closer), so the point stays
   * where it is on screen. Orthographic: the view's height scaled by `factor` about the point across the view.
   */
  dollyTo(point: Vec3, factor: number): void {
    if (!(factor > 0)) return;
    const f = this.forward;
    if (this.ortho) {
      const across = (v: Vec3) => sub(v, scale(f, dot(v, f)));
      this.eye = add(this.eye, scale(across(sub(this.eye, point)), factor - 1));
      this.pivot = add(this.pivot, scale(across(sub(this.pivot, point)), factor - 1));
      this.height *= factor;
      return;
    }
    // never onto the point itself
    const toEye = sub(this.eye, point);
    const k = Math.max(factor, (1e-4 * this.radius) / (length(toEye) || 1));
    this.eye = add(point, scale(toEye, k));
    this.pivot = add(point, scale(sub(this.pivot, point), k));
  }

  /** The wheel's dolly at a point of the view: toward `hit` (the drawn surface under the cursor), else the pivot's plane. */
  wheel(deltaY: number, ndcX: number, ndcY: number, hit: Vec3 | null): void {
    this.dollyTo(hit ?? this.pivotPlanePoint(ndcX, ndcY), Math.pow(WHEEL_BASE, deltaY));
  }

  /** + and -: dolly toward the centre of the view (in: factor below 1). */
  zoom(factor: number): void {
    this.dollyTo(this.centre, factor);
  }

  /** The orbit pivot at a point (a double-click on the surface); the view does not move. */
  setPivot(p: Vec3): void {
    this.pivot = p;
  }

  /**
   * Face on: looks along the nearest principal axis of the view direction (a 2D case: its thin axis), with the up the
   * nearest principal axis to the current up; the point at the centre of the view stays there and stays as far (the
   * orthographic height is kept), and becomes the pivot. Only the orientation changes.
   */
  faceOn(): void {
    const c = this.centre;
    const d = this.depth;
    if (this.thinAxis !== null) this.rot = faceOnRotation(this.thinAxis);
    else {
      const f = this.forward;
      const k = [0, 1, 2].reduce((a, b) => (Math.abs(f[b]!) > Math.abs(f[a]!) ? b : a), 0) as Axis;
      const nf = unit(k, Math.sign(f[k]) || 1);
      const u = this.up;
      let best: Vec3 = [0, 0, 1];
      let bestDot = -Infinity;
      for (const a of [0, 1, 2] as const) {
        if (a === k) continue;
        for (const s of [1, -1]) {
          const e = unit(a, s);
          if (dot(e, u) > bestDot) {
            bestDot = dot(e, u);
            best = e;
          }
        }
      }
      this.rot = lookRotation(nf, best);
    }
    this.eye = sub(c, scale(this.forward, d));
    this.pivot = c;
  }

  /**
   * The axis the view looks straight along (within 0.1 degree), and the world length of one pixel on the domain's face
   * toward the camera there, for a view `heightPx` tall; null when the view looks along no axis.
   */
  faceScale(heightPx: number): { readonly axis: Axis; readonly perPixel: number } | null {
    if (!this.box || heightPx <= 0) return null;
    const look = this.forward;
    for (const axis of [0, 1, 2] as const) {
      if (Math.abs(look[axis]) < Math.cos((0.1 * Math.PI) / 180)) continue;
      if (this.ortho) return { axis, perPixel: this.height / heightPx };
      const face = look[axis] < 0 ? this.box.max[axis]! : this.box.min[axis]!;
      const d = Math.abs(this.eye[axis] - face);
      return { axis, perPixel: (2 * d * Math.tan((this.fov * Math.PI) / 360)) / heightPx };
    }
    return null;
  }
}

/** The fly direction of the keys held (W A S D forward, left, back, right; Q down, E up), as `fly` takes it. */
export function flyMove(held: ReadonlySet<string>): Vec3 {
  const k = (c: string) => (held.has(c) ? 1 : 0);
  return [k('KeyD') - k('KeyA'), k('KeyE') - k('KeyQ'), k('KeyW') - k('KeyS')];
}

/** The keys that fly while the right button is held (KeyboardEvent.code, so every layout flies the same). */
export const FLY_KEYS: ReadonlySet<string> = new Set(['KeyW', 'KeyA', 'KeyS', 'KeyD', 'KeyQ', 'KeyE']);

/** The camera's state as data attributes of the canvas (the e2e tests read them). */
export function cameraAttributes(c: ViewCamera, moved: boolean): Record<string, string> {
  const t = (v: readonly number[]) => v.map((x) => (Math.abs(x) < 1e-12 ? 0 : x).toPrecision(6)).join(' ');
  return {
    camera: c.ortho ? 'orthographic' : 'perspective',
    cameraEye: t(c.eye),
    cameraForward: t(c.forward),
    cameraUp: t(c.up),
    cameraPivot: t(c.pivot),
    cameraZoom: (c.ortho ? c.height : c.depth).toPrecision(6),
    cameraMoved: moved ? 'true' : 'false',
  };
}
