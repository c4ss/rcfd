//! Orbit camera: pure math, no GPU (ADR 0125 C1-C3).
//!
//! World up is +y. The eye sits at `target + distance * d(yaw, pitch)` with
//! `d = (cos p sin y, sin p, cos p cos y)`, so yaw = pitch = 0 looks down -z with +x right and +y up.
//! The camera basis is written in closed form (no cross product with a fixed up vector), so it has no
//! singularity at pitch = +-90 degrees. Clip space is the wgpu one: x, y in [-1, 1], depth in [0, 1].
//! Pixel coordinates have their origin at the top-left image corner, y down, pixel `i` centred at `i + 0.5`.

use rcfd_core::vec3::{add, dot, scale, sub, Vec3};

/// Row-major 4x4 matrix.
pub type Mat4 = [[f64; 4]; 4];

/// Projection kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// Perspective with a vertical field of view in radians.
    Perspective {
        /// Vertical field of view (radians).
        fovy: f64,
    },
    /// Orthographic with the half height of the view volume in world units.
    Orthographic {
        /// Half the visible height (world units).
        half_height: f64,
    },
}

/// Orbit camera around a target point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Point the camera looks at and orbits around.
    pub target: Vec3,
    /// Eye-to-target distance.
    pub distance: f64,
    /// Rotation about +y (radians).
    pub yaw: f64,
    /// Elevation above the x-z plane (radians).
    pub pitch: f64,
    /// Projection.
    pub projection: Projection,
    /// Near plane distance (> 0).
    pub near: f64,
    /// Far plane distance (> near).
    pub far: f64,
}

impl Camera {
    /// Camera that frames the box `[lo, hi]` (its bounding sphere) from the given yaw and pitch, for images with
    /// width >= height. An orthographic `half_height` passed in is replaced by the sphere radius.
    pub fn fit(lo: Vec3, hi: Vec3, yaw: f64, pitch: f64, projection: Projection) -> Camera {
        let target = scale(add(lo, hi), 0.5);
        let r = (0.5 * rcfd_core::vec3::norm(sub(hi, lo))).max(1e-30);
        let (distance, projection) = match projection {
            Projection::Perspective { fovy } => (r / (0.5 * fovy).sin(), projection),
            Projection::Orthographic { .. } => (2.0 * r, Projection::Orthographic { half_height: r }),
        };
        Camera {
            target,
            distance,
            yaw,
            pitch,
            projection,
            near: (distance - 1.01 * r).max(1e-3 * r),
            far: distance + 1.01 * r,
        }
    }

    /// Orthonormal camera basis (right, up, back): back points from the target to the eye.
    pub fn basis(&self) -> [Vec3; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [[cy, 0.0, -sy], [-sp * sy, cp, -sp * cy], [cp * sy, sp, cp * cy]]
    }

    /// Eye position.
    pub fn eye(&self) -> Vec3 {
        add(self.target, scale(self.basis()[2], self.distance))
    }

    /// Rotate about the target (turntable orbit); the distance is kept.
    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw += dyaw;
        self.pitch += dpitch;
    }

    /// World-to-view matrix (right-handed, the camera looks down -z).
    pub fn view_matrix(&self) -> Mat4 {
        let [x, y, z] = self.basis();
        let e = self.eye();
        [
            [x[0], x[1], x[2], -dot(x, e)],
            [y[0], y[1], y[2], -dot(y, e)],
            [z[0], z[1], z[2], -dot(z, e)],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// View-to-clip matrix for an image aspect ratio `width / height` (depth range [0, 1]).
    pub fn proj_matrix(&self, aspect: f64) -> Mat4 {
        let (n, f) = (self.near, self.far);
        match self.projection {
            Projection::Perspective { fovy } => {
                let s = 1.0 / (0.5 * fovy).tan();
                [
                    [s / aspect, 0.0, 0.0, 0.0],
                    [0.0, s, 0.0, 0.0],
                    [0.0, 0.0, f / (n - f), n * f / (n - f)],
                    [0.0, 0.0, -1.0, 0.0],
                ]
            }
            Projection::Orthographic { half_height: h } => [
                [1.0 / (h * aspect), 0.0, 0.0, 0.0],
                [0.0, 1.0 / h, 0.0, 0.0],
                [0.0, 0.0, 1.0 / (n - f), n / (n - f)],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// World-to-clip matrix.
    pub fn view_proj(&self, aspect: f64) -> Mat4 {
        mat_mul(&self.proj_matrix(aspect), &self.view_matrix())
    }

    /// Pixel position and depth of a world point in a `width` x `height` image; `None` behind the eye.
    pub fn project(&self, p: Vec3, width: f64, height: f64) -> Option<Vec3> {
        let c = mat_vec(&self.view_proj(width / height), [p[0], p[1], p[2], 1.0]);
        if c[3] <= 0.0 {
            return None;
        }
        let (x, y, z) = (c[0] / c[3], c[1] / c[3], c[2] / c[3]);
        Some([0.5 * (x + 1.0) * width, 0.5 * (1.0 - y) * height, z])
    }

    /// Inverse of [`Camera::project`]: the world point at pixel `(px, py)` and depth `depth`.
    /// Written from the closed-form inverse of each matrix, not by inverting `view_proj`.
    pub fn unproject(&self, px: f64, py: f64, depth: f64, width: f64, height: f64) -> Vec3 {
        let aspect = width / height;
        let (nx, ny) = (2.0 * px / width - 1.0, 1.0 - 2.0 * py / height);
        let (n, f) = (self.near, self.far);
        let v = match self.projection {
            Projection::Perspective { fovy } => {
                // depth = (A z + B) / (-z) with A = f/(n-f), B = n f/(n-f)
                let z = -(n * f / (n - f)) / (depth + f / (n - f));
                let t = (0.5 * fovy).tan() * -z;
                [nx * t * aspect, ny * t, z]
            }
            Projection::Orthographic { half_height: h } => [nx * h * aspect, ny * h, depth * (n - f) - n],
        };
        let [bx, by, bz] = self.basis();
        add(self.eye(), add(scale(bx, v[0]), add(scale(by, v[1]), scale(bz, v[2]))))
    }
}

/// Matrix product `a b`.
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// Matrix-vector product.
pub fn mat_vec(a: &Mat4, v: [f64; 4]) -> [f64; 4] {
    [0, 1, 2, 3].map(|i| (0..4).map(|k| a[i][k] * v[k]).sum())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_core::vec3::norm;

    fn persp() -> Camera {
        Camera {
            target: [1.0, 2.0, 3.0],
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.0,
            projection: Projection::Perspective {
                fovy: 60f64.to_radians(),
            },
            near: 0.5,
            far: 20.0,
        }
    }

    fn ortho() -> Camera {
        Camera {
            projection: Projection::Orthographic { half_height: 2.0 },
            ..persp()
        }
    }

    // C1: closed-form projections of known points.
    #[test]
    fn c1_projection_closed_forms() {
        let (w, h) = (800.0, 600.0);
        let s = 1.0 / (30f64.to_radians()).tan();
        let c = persp();
        // A point offset by (a, b) in the target plane at distance d: px = w/2 (1 + s a / (d aspect)).
        let (a, b) = (0.7, -0.4);
        let p = c.project([1.0 + a, 2.0 + b, 3.0], w, h).unwrap();
        assert!((p[0] - 0.5 * w * (1.0 + s * a / (5.0 * w / h))).abs() <= 1e-9);
        assert!((p[1] - 0.5 * h * (1.0 - s * b / 5.0)).abs() <= 1e-9);
        let t = c.project(c.target, w, h).unwrap();
        assert!((t[0] - 0.5 * w).abs() <= 1e-9 && (t[1] - 0.5 * h).abs() <= 1e-9);
        // Points on the near and far planes have depth 0 and 1.
        let e = c.eye();
        assert!(c.project([e[0], e[1], e[2] - 0.5], w, h).unwrap()[2].abs() <= 1e-12);
        assert!((c.project([e[0], e[1], e[2] - 20.0], w, h).unwrap()[2] - 1.0).abs() <= 1e-12);
        assert!(c.project([e[0], e[1], e[2] + 1.0], w, h).is_none());
        // Orthographic: px = w/2 (1 + a / (h_half aspect)), independent of depth.
        let o = ortho();
        for z in [3.0, 0.0, -10.0] {
            let p = o.project([1.0 + a, 2.0 + b, z], w, h).unwrap();
            assert!((p[0] - 0.5 * w * (1.0 + a / (2.0 * w / h))).abs() <= 1e-9);
            assert!((p[1] - 0.5 * h * (1.0 - b / 2.0)).abs() <= 1e-9);
        }
        // yaw = 90 degrees puts the eye on +x: a point offset along -z appears on the right.
        let mut r = persp();
        r.yaw = std::f64::consts::FRAC_PI_2;
        assert!((r.eye()[0] - 6.0).abs() <= 1e-12);
        assert!(r.project([1.0, 2.0, 3.0 - a], w, h).unwrap()[0] > 0.5 * w + 1.0);
    }

    // C2: orbit invariants.
    #[test]
    fn c2_orbit_invariants() {
        let (w, h) = (640.0, 480.0);
        let mut c = persp();
        let c0 = c;
        for k in 0..200 {
            c.orbit(0.37, 0.11 * (k as f64).sin());
            assert!((norm(sub(c.eye(), c.target)) - 5.0).abs() <= 1e-12 * 5.0);
            let t = c.project(c.target, w, h).unwrap();
            assert!((t[0] - 0.5 * w).abs() <= 1e-9 && (t[1] - 0.5 * h).abs() <= 1e-9);
            let [x, y, z] = c.basis();
            assert!(x[1].abs() <= 1e-15, "right vector stays horizontal");
            for (u, v) in [(x, y), (y, z), (z, x)] {
                assert!(dot(u, v).abs() <= 1e-12);
            }
            for u in [x, y, z] {
                assert!((norm(u) - 1.0).abs() <= 1e-12);
            }
        }
        // A full turn in yaw returns the eye.
        let mut d = c0;
        d.orbit(2.0 * std::f64::consts::PI, 0.0);
        assert!(norm(sub(d.eye(), c0.eye())) <= 1e-12 * 5.0);
        // Pitch = 90 degrees is not singular: the eye is straight above the target.
        d.pitch = std::f64::consts::FRAC_PI_2;
        assert!(norm(sub(d.eye(), [1.0, 7.0, 3.0])) <= 1e-12);
    }

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*s >> 11) as f64 / (1u64 << 53) as f64
    }

    fn roundtrip_err(c: &Camera, w: f64, h: f64, wrong_aspect: bool) -> f64 {
        let mut s = 7u64;
        let mut worst: f64 = 0.0;
        for _ in 0..1000 {
            let p = [0, 1, 2].map(|d| c.target[d] + 3.0 * (lcg(&mut s) - 0.5));
            let q = c.project(p, w, h).unwrap();
            let back = if wrong_aspect {
                c.unproject(q[0], q[1], q[2], h, w)
            } else {
                c.unproject(q[0], q[1], q[2], w, h)
            };
            worst = worst.max(norm(sub(back, p)) / norm(p).max(1e-300));
        }
        worst
    }

    // C3: unproject(project(p)) = p to 1e-6 relative; negative control: a swapped aspect must miss by > 1e-3.
    #[test]
    #[allow(clippy::approx_constant)] // 1.5707963 is the registered near-vertical pitch (ADR 0125 C3), not pi/2
    fn c3_unproject_round_trip() {
        for mut c in [persp(), ortho()] {
            for (yaw, pitch) in [(0.0, 0.0), (0.8, 0.4), (-2.0, 1.5707963), (3.0, -0.9)] {
                c.yaw = yaw;
                c.pitch = pitch;
                let e = roundtrip_err(&c, 800.0, 600.0, false);
                assert!(e <= 1e-6, "round trip {e}");
                let n = roundtrip_err(&c, 800.0, 600.0, true);
                assert!(n > 1e-3, "negative control {n}");
            }
        }
    }

    #[test]
    fn fit_puts_the_box_inside_the_frustum() {
        let (lo, hi) = ([-1.0, 0.0, 2.0], [3.0, 1.0, 2.5]);
        for proj in [
            Projection::Perspective { fovy: 0.8 },
            Projection::Orthographic { half_height: 1.0 },
        ] {
            let c = Camera::fit(lo, hi, 0.6, 0.3, proj);
            for k in 0..8 {
                let p = [0, 1, 2].map(|d| if k >> d & 1 == 1 { hi[d] } else { lo[d] });
                let q = c.project(p, 100.0, 100.0).unwrap();
                assert!((0.0..=100.0).contains(&q[0]) && (0.0..=100.0).contains(&q[1]));
                assert!((0.0..=1.0).contains(&q[2]));
            }
        }
    }
}
