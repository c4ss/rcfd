//! F-471 (ADR 0748): the nine tests of registered fingerprints and records of the overset solver that moved with the
//! corrected order of the coarsest dense LU (ADR 0743), restated.
//!
//! Every pressure solve of an overset step with a coupling is preconditioned with a scalar AMG, so the default of ADR 0743
//! moved the bits of the runs whose coarsest factorisation interchanges rows at a step `k >= 1`. The full gate of main
//! 252c9279 (run 44599067147) failed at nine tests of eight files, which are now ignored records; the maintainer approved
//! the moves on 2026-11-20. Each is restated here twice: under the default with the values of that gate, and under the
//! old order, asked for with `LinearOptions::amg_coarse_lu_interleaved` (the way back ADR 0748 gave `overset.rs`), with
//! every value as registered by the test's own ADR. Each module holds copies of the helpers of one source file, unchanged
//! but for one line: `settings` sets the flag of the old order from `old_order()`. The source files are not edited beyond
//! the attribute that retires each test.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_fingerprints_adr0748`.

use std::cell::Cell;

thread_local! {
    /// Whether the `settings` of this thread ask for the old order of the coarsest dense LU solve.
    static OLD_ORDER: Cell<bool> = const { Cell::new(false) };
}

fn old_order() -> bool {
    OLD_ORDER.with(Cell::get)
}

/// `f` with the old order asked for by every `settings` it calls. Every solver of this file is built on the thread of its
/// test, so the flag of one test does not reach another.
fn under_old_order<T>(f: impl FnOnce() -> T) -> T {
    OLD_ORDER.with(|c| c.set(true));
    let r = f();
    OLD_ORDER.with(|c| c.set(false));
    r
}

/// Copies from `overset_f487.rs` and the restated test.
mod f487 {
    use super::under_old_order;
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::bc::{FixedValue, Profile};
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, warp, Grading};
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    /// ADR 0219 PS.
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    const NU: f64 = 0.01;

    const DT: f64 = 0.005;

    const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

    /// The flow V of ADR 0382: box side, vortex amplitude, axial velocity.
    const L: f64 = 2.4;

    const VA: f64 = 1.0;

    const WZ: f64 = 0.5;

    fn k_wave() -> f64 {
        PI / L
    }

    /// `u_ex` of V at `(x, t)`.
    fn u_ex(x: Vec3, t: f64) -> Vec3 {
        let k = k_wave();
        let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
        let f = (-2.0 * NU * k * k * t).exp();
        [
            -VA * (k * xi).cos() * (k * eta).sin() * f,
            VA * (k * xi).sin() * (k * eta).cos() * f,
            WZ,
        ]
    }

    /// `p_ex` of V at `(x, t)`.
    fn p_ex(x: Vec3, t: f64) -> f64 {
        let k = k_wave();
        let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
        let f = (-2.0 * NU * k * k * t).exp();
        -(VA * VA / 4.0) * ((2.0 * k * xi).cos() + (2.0 * k * eta).cos()) * f * f
    }

    fn settings(p_level: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(NU);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.p_ref = (0, p_level);
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    /// A fixed pseudo-random number in [-1, 1] of a position.
    fn jitter(x: Vec3, seed: f64) -> f64 {
        let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
        2.0 * (v - v.floor()) - 1.0
    }

    fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
        let dirs = dirs.to_vec();
        warp(mesh, move |p| {
            let tol = 1e-9 * (hi - lo);
            if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
                return p;
            }
            let key = if planar { [p[0], p[1], 0.0] } else { p };
            let mut q = p;
            for &d in &dirs {
                q[d] += amp * jitter(key, 1.0 + d as f64);
            }
            q
        })
        .unwrap()
    }

    /// The background of B3 (`distort`: of B3d).
    fn background(distort: bool) -> Mesh {
        let bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false)
        } else {
            bg
        }
    }

    /// The component of B3 (`distort`: of B3d).
    fn component(distort: bool) -> Mesh {
        let comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false)
        } else {
            comp
        }
    }

    /// The overset of ADR 0251 from a background and a component, with the donor interpolation `mode`.
    fn overset(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> Overset {
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
                .map(String::from)
                .to_vec(),
            hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
        };
        let mut ov = Overset::new(compose(bg, comp, "c_").unwrap(), spec).unwrap();
        if mode != DonorInterpolation::Linear {
            ov.set_donor_interpolation(mode);
        }
        ov
    }

    /// The motion of ADR 0255: `P(0)` is the pose of ADR 0251.
    fn pose(t: f64) -> Pose {
        Pose {
            origin: [0.7; 3],
            axis: vec3::unit([1.0, 2.0, 3.0]),
            angle: 0.3 + 0.5 * t,
            translation: [0.53 - 0.25 * t, 0.52 + 0.25 * t, 0.49 + 0.25 * t],
        }
    }

    /// `u_ex(., t)` at the face centres of the six background patches.
    fn set_conditions(u: &mut VectorField, geom: &FvGeometry, t: f64) {
        for name in PATCHES {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| u_ex(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// An overset solver at `P(0)` on `bg` and `comp` with V at `t = 0` (ADR 0382 set-ups).
    fn vortex_solver(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> OversetSolver {
        let ov = overset(bg, comp, mode);
        let p0 = pose(0.0);
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::from_fn("U", &geom, |x| u_ex(x, 0.0));
        set_conditions(&mut u, &geom, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| p_ex(x, 0.0));
        let level = p_ex(geom.mesh.cell_centres()[0], 0.0);
        OversetSolver::new(
            geom,
            u,
            p,
            settings(level),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose: p0 }),
        )
        .unwrap()
    }

    /// One step of V: the boundary values at the new time, then the step to `P(k dt)`.
    fn vortex_step(s: &mut OversetSolver, k: usize) -> Result<rcfd_fv::overset::OversetInfo, String> {
        let t = k as f64 * DT;
        let g = s.geom.clone();
        set_conditions(&mut s.u, &g, t);
        s.step(DT, pose(t), &[])
    }

    /// FNV-1a over bytes.
    struct Fnv(u64);

    impl Fnv {
        fn new() -> Self {
            Fnv(0xcbf2_9ce4_8422_2325)
        }
        fn bytes(&mut self, b: &[u8]) {
            for &x in b {
                self.0 ^= x as u64;
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn f64s(&mut self, v: impl IntoIterator<Item = f64>) {
            for x in v {
                self.bytes(&x.to_bits().to_le_bytes());
            }
        }
    }

    /// 5 steps of V on B3d with the geometry corrections and ADR 0382's L on; a failing step folds its message and ends the run.
    fn d1c_fingerprint(mode: DonorInterpolation) -> u64 {
        let mut s = vortex_solver(&background(true), &component(true), mode);
        s.set_geometry_corrections(true);
        s.set_least_squares_pressure_gradient(true);
        let mut h = Fnv::new();
        for k in 1..=5 {
            match vortex_step(&mut s, k) {
                Ok(_) => {
                    for c in 0..3 {
                        h.f64s(s.u.values.iter().map(|v| v[c]));
                    }
                    h.f64s(s.p.values.iter().copied());
                    h.f64s(s.phi.iter().copied());
                }
                Err(e) => {
                    h.bytes(format!("step {k}: {e}").as_bytes());
                    break;
                }
            }
        }
        h.0
    }

    fn d1c_fingerprints() -> [u64; 2] {
        [
            d1c_fingerprint(DonorInterpolation::Linear),
            d1c_fingerprint(DonorInterpolation::Quadratic),
        ]
    }

    /// The values of ADR 0387 D1c (linear, quadratic).
    const BASE: [u64; 2] = [0x6820_a06f_1b6a_1496, 0x861a_b174_c9e7_dbe2];
    /// The values under the default of ADR 0743 (gate run 44599067147).
    const NEW: [u64; 2] = [0x0c3e_c8cd_8e21_0f07, 0xec0a_524a_76ab_219e];

    /// ADR 0748 N1: `overset_f487::d1c_l_fingerprint_equals_the_base_tree` under the default.
    #[test]
    fn t1_d1c_default() {
        let f = d1c_fingerprints();
        println!("N1 D1c, default: {f:#018x?}");
        assert_eq!(f, NEW, "N1: D1c under the default");
    }

    /// ADR 0748 W1: the same under the old order gives the values of ADR 0387.
    #[test]
    fn t1_d1c_old_order() {
        let f = under_old_order(d1c_fingerprints);
        println!("W1 D1c, old order: {f:#018x?}");
        assert_eq!(f, BASE, "W1: D1c under the old order");
    }
}

/// Copies from `overset_f472.rs` and the restated test.
mod f472 {
    use super::under_old_order;
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::bc::{FixedValue, Profile};
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, warp, Grading};
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    /// ADR 0219 PS.
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    const NU: f64 = 0.01;

    const DT: f64 = 0.005;

    const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

    /// The flow V of ADR 0382: box side, vortex amplitude, axial velocity.
    const L: f64 = 2.4;

    const VA: f64 = 1.0;

    const WZ: f64 = 0.5;

    fn k_wave() -> f64 {
        PI / L
    }

    /// `u_ex` of V at `(x, t)`.
    fn u_ex(x: Vec3, t: f64) -> Vec3 {
        let k = k_wave();
        let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
        let f = (-2.0 * NU * k * k * t).exp();
        [
            -VA * (k * xi).cos() * (k * eta).sin() * f,
            VA * (k * xi).sin() * (k * eta).cos() * f,
            WZ,
        ]
    }

    /// `p_ex` of V at `(x, t)`.
    fn p_ex(x: Vec3, t: f64) -> f64 {
        let k = k_wave();
        let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
        let f = (-2.0 * NU * k * k * t).exp();
        -(VA * VA / 4.0) * ((2.0 * k * xi).cos() + (2.0 * k * eta).cos()) * f * f
    }

    fn settings(p_level: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(NU);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.p_ref = (0, p_level);
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    /// A fixed pseudo-random number in [-1, 1] of a position.
    fn jitter(x: Vec3, seed: f64) -> f64 {
        let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
        2.0 * (v - v.floor()) - 1.0
    }

    fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
        let dirs = dirs.to_vec();
        warp(mesh, move |p| {
            let tol = 1e-9 * (hi - lo);
            if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
                return p;
            }
            let key = if planar { [p[0], p[1], 0.0] } else { p };
            let mut q = p;
            for &d in &dirs {
                q[d] += amp * jitter(key, 1.0 + d as f64);
            }
            q
        })
        .unwrap()
    }

    /// The background of B3 (`distort`: of B3d).
    fn background(distort: bool) -> Mesh {
        let bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false)
        } else {
            bg
        }
    }

    /// The component of B3 (`distort`: of B3d).
    fn component(distort: bool) -> Mesh {
        let comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false)
        } else {
            comp
        }
    }

    /// The overset of ADR 0251 from a background and a component, with the donor interpolation `mode`.
    fn overset(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> Overset {
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
                .map(String::from)
                .to_vec(),
            hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
        };
        let mut ov = Overset::new(compose(bg, comp, "c_").unwrap(), spec).unwrap();
        if mode != DonorInterpolation::Linear {
            ov.set_donor_interpolation(mode);
        }
        ov
    }

    /// The motion of ADR 0255: `P(0)` is the pose of ADR 0251.
    fn pose(t: f64) -> Pose {
        Pose {
            origin: [0.7; 3],
            axis: vec3::unit([1.0, 2.0, 3.0]),
            angle: 0.3 + 0.5 * t,
            translation: [0.53 - 0.25 * t, 0.52 + 0.25 * t, 0.49 + 0.25 * t],
        }
    }

    /// `u_ex(., t)` at the face centres of the six background patches.
    fn set_conditions(u: &mut VectorField, geom: &FvGeometry, t: f64) {
        for name in PATCHES {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| u_ex(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// An overset solver at `P(0)` on `bg` and `comp` with V at `t = 0` (ADR 0382 set-ups).
    fn vortex_solver(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> OversetSolver {
        let ov = overset(bg, comp, mode);
        let p0 = pose(0.0);
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::from_fn("U", &geom, |x| u_ex(x, 0.0));
        set_conditions(&mut u, &geom, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| p_ex(x, 0.0));
        let level = p_ex(geom.mesh.cell_centres()[0], 0.0);
        OversetSolver::new(
            geom,
            u,
            p,
            settings(level),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose: p0 }),
        )
        .unwrap()
    }

    /// One step of V: the boundary values at the new time, then the step to `P(k dt)`.
    fn vortex_step(s: &mut OversetSolver, k: usize) -> Result<rcfd_fv::overset::OversetInfo, String> {
        let t = k as f64 * DT;
        let g = s.geom.clone();
        set_conditions(&mut s.u, &g, t);
        s.step(DT, pose(t), &[])
    }

    /// FNV-1a over bytes.
    struct Fnv(u64);

    impl Fnv {
        fn new() -> Self {
            Fnv(0xcbf2_9ce4_8422_2325)
        }
        fn bytes(&mut self, b: &[u8]) {
            for &x in b {
                self.0 ^= x as u64;
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn f64s(&mut self, v: impl IntoIterator<Item = f64>) {
            for x in v {
                self.bytes(&x.to_bits().to_le_bytes());
            }
        }
    }

    /// 5 steps of V on B3d with the geometry corrections on; a failing step folds its message and ends the run.
    fn d1b_fingerprint(mode: DonorInterpolation) -> u64 {
        let mut s = vortex_solver(&background(true), &component(true), mode);
        s.set_geometry_corrections(true);
        let mut h = Fnv::new();
        for k in 1..=5 {
            match vortex_step(&mut s, k) {
                Ok(_) => {
                    for c in 0..3 {
                        h.f64s(s.u.values.iter().map(|v| v[c]));
                    }
                    h.f64s(s.p.values.iter().copied());
                    h.f64s(s.phi.iter().copied());
                }
                Err(e) => {
                    h.bytes(format!("step {k}: {e}").as_bytes());
                    break;
                }
            }
        }
        h.0
    }

    fn d1b_fingerprints() -> [u64; 2] {
        [
            d1b_fingerprint(DonorInterpolation::Linear),
            d1b_fingerprint(DonorInterpolation::Quadratic),
        ]
    }

    /// The values of ADR 0382 D1b (linear, quadratic).
    const BASE: [u64; 2] = [0x6e61_17e1_b918_51a9, 0x69a7_19b8_6040_e649];
    /// The values under the default of ADR 0743 (gate run 44599067147).
    const NEW: [u64; 2] = [0x1495_40b7_5a11_eb76, 0x688b_d967_8efe_83d4];

    /// ADR 0748 N2: `overset_f472::d1b_opt_in_fingerprint_equals_the_base_tree` under the default.
    #[test]
    fn t2_d1b_default() {
        let f = d1b_fingerprints();
        println!("N2 D1b, default: {f:#018x?}");
        assert_eq!(f, NEW, "N2: D1b under the default");
    }

    /// ADR 0748 W2: the same under the old order gives the values of ADR 0382.
    #[test]
    fn t2_d1b_old_order() {
        let f = under_old_order(d1b_fingerprints);
        println!("W2 D1b, old order: {f:#018x?}");
        assert_eq!(f, BASE, "W2: D1b under the old order");
    }
}

/// Copies from `overset_f335_records.rs` and the restated tests.
mod f335 {
    use super::under_old_order;
    use rcfd_core::vec3::Vec3;
    use rcfd_fv::bc::FixedValue;
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    const DEPTH: f64 = 0.1;

    /// Solver controls of every set-up (ADR 0219 PS).
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    /// ADR 0219 PS.
    fn settings(nu: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    fn push(bytes: &mut Vec<u8>, x: f64) {
        bytes.extend_from_slice(&x.to_bits().to_le_bytes());
    }

    /// The hash of a solver's state and of the `delta` of every step.
    fn state_hash(s: &OversetSolver) -> String {
        let mut bytes = Vec::new();
        for v in &s.u.values {
            for x in v {
                push(&mut bytes, *x);
            }
        }
        for x in s.p.values.iter().chain(&s.phi) {
            push(&mut bytes, *x);
        }
        push(&mut bytes, s.time);
        bytes.extend_from_slice(&(s.history.len() as u64).to_le_bytes());
        for h in &s.history {
            push(&mut bytes, h.delta);
        }
        format!("{:016x}", rcfd_core::hash::fnv64(&bytes))
    }

    fn check(name: &str, got: &str, want: &str) {
        println!("{name}: {got}");
        if cfg!(windows) {
            assert_eq!(got, want, "{name}");
        }
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
        let cs = uniform(-half, 2.0 * half, n);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
        };
        Overset::new(composite, spec).unwrap()
    }

    const U_FLOW: Vec3 = [1.0, 0.5, 0.0];

    fn u_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [t, 0.5 * t, 0.0],
        }
    }

    /// U for 80 steps; `opt` is applied to the solver before the first step.
    fn u_hash(opt: impl Fn(&mut OversetSolver)) -> String {
        const DT_U: f64 = 0.01;
        let bs = uniform(-2.5, 5.0, 50);
        let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25);
        let pose = u_pose(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::uniform("U", &geom, U_FLOW);
        for name in ["xmin", "ymin", "ymax"] {
            u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(0.01),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        opt(&mut s);
        for k in 1..=80 {
            s.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
        }
        state_hash(&s)
    }

    const NU_TG: f64 = 0.1;

    fn tg_u(x: Vec3, t: f64) -> Vec3 {
        let e = (-2.0 * NU_TG * t).exp();
        [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
    }

    fn tg_p(x: Vec3, t: f64) -> f64 {
        -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
    }

    fn tg_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
        }
    }

    fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
        for name in ["xmin", "xmax", "ymin", "ymax"] {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| tg_u(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// TG_1 for 200 steps (`dt = 0.0025`); `opt` is applied to the solver before the first step.
    fn tg_hash(opt: impl Fn(&mut OversetSolver)) -> String {
        let dt = 0.5 / 200.0;
        let bs = uniform(0.0, PI, 40);
        let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 20, 0.25);
        let pose = tg_pose(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(NU_TG),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        opt(&mut s);
        for k in 1..=200 {
            let t = k as f64 * dt;
            let g = s.geom.clone();
            tg_conditions(&g, &mut s.u, t);
            s.step(dt, tg_pose(t), &[]).unwrap();
        }
        state_hash(&s)
    }

    fn displacement(t: f64) -> f64 {
        if t <= 1.0 {
            -(0.5 * t - (PI * t).sin() / (2.0 * PI))
        } else {
            -(0.5 + (t - 1.0))
        }
    }

    fn graded(x0: f64, len: f64, n: usize, h: f64, toward_end: bool) -> Vec<f64> {
        let r = expansion_for_end_cell(n, len, h).unwrap();
        let r = if toward_end { r } else { 1.0 / r };
        axis_coords(n, len, Grading::Expansion(r))
            .unwrap()
            .into_iter()
            .map(|x| x0 + x)
            .collect()
    }

    fn join(parts: &[Vec<f64>]) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for p in parts {
            let skip = usize::from(!out.is_empty());
            out.extend_from_slice(&p[skip..]);
        }
        out
    }

    /// TR for its first 20 steps (`dt = 0.02`); `opt` is applied to the solver before the first step.
    fn tr_hash(opt: impl Fn(&mut OversetSolver)) -> String {
        const DT: f64 = 0.02;
        let xs = join(&[
            graded(-25.0, 17.0, 30, 0.1, true),
            uniform(-8.0, 12.5, 125),
            graded(4.5, 17.5, 30, 0.1, false),
        ]);
        let ys = join(&[
            graded(-20.0, 17.5, 30, 0.1, true),
            uniform(-2.5, 5.0, 50),
            graded(2.5, 17.5, 30, 0.1, false),
        ]);
        let bg = rectilinear_2d(&xs, &ys, DEPTH).unwrap();
        let q: f64 = 1.08;
        let r_out = 0.5 + 0.01 * (q.powi(30) - 1.0) / (q - 1.0);
        let comp = cylinder_ogrid(96, 30, 0.5, r_out, Grading::Expansion(q.powi(29)), DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: vec!["c_farfield".into()],
            hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
        };
        let ov = Overset::new(composite, spec).unwrap();
        let pose_at = |t: f64| Pose::translation([2.0 + displacement(t), 0.0, 0.0]);
        let pose = pose_at(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for name in ["xmin", "ymin", "ymax", "c_cylinder"] {
            u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(0.025),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        opt(&mut s);
        let walls = vec!["c_cylinder".to_string()];
        for n in 1..=20 {
            s.step(DT, pose_at(n as f64 * DT), &walls).unwrap();
        }
        state_hash(&s)
    }

    const R2A: &str = "7aa14d1e2e2da052";

    const R2B: &str = "75ac2daf13879872";

    const R2C: &str = "fddb420dc287502a";

    /// The opt-in with one outer iteration: the step must be the PISO step of a solver that did not opt in.
    fn one(s: &mut OversetSolver) {
        s.set_outer_iterations(1, 1.0, 1.0).unwrap();
    }

    /// TG_1, 200 steps, under the default of ADR 0743 (gate run 44599067147); R2B is the value of ADR 0238.
    const TG_NEW: &str = "a126cd75e3d481d7";

    /// ADR 0748 N3: `overset_f335_records::r2b_taylor_green_level_1_record` under the default.
    #[test]
    fn t3_r2b_default() {
        check("N3 R2b, default", &tg_hash(|_| {}), TG_NEW);
    }

    /// ADR 0748 W3: the same under the old order gives the record of ADR 0238.
    #[test]
    fn t3_r2b_old_order() {
        check("W3 R2b, old order", &under_old_order(|| tg_hash(|_| {})), R2B);
    }

    /// ADR 0748 N4: `r2_prime_opted_in_with_one_outer_iteration_gives_the_records` under the default: U and TR as registered,
    /// TG_1 the new value. The three runs are made before the first assertion.
    #[test]
    fn t4_r2_prime_default() {
        let (a, b, c) = (u_hash(one), tg_hash(one), tr_hash(one));
        println!("N4 R2', default: {a} {b} {c}");
        check("N4 R2'a, default", &a, R2A);
        check("N4 R2'b, default", &b, TG_NEW);
        check("N4 R2'c, default", &c, R2C);
    }

    /// ADR 0748 W4: the same under the old order gives the three records of ADR 0238.
    #[test]
    fn t4_r2_prime_old_order() {
        let (a, b, c) = under_old_order(|| (u_hash(one), tg_hash(one), tr_hash(one)));
        println!("W4 R2', old order: {a} {b} {c}");
        check("W4 R2'a, old order", &a, R2A);
        check("W4 R2'b, old order", &b, R2B);
        check("W4 R2'c, old order", &c, R2C);
    }
}

/// Copies from `overset_motion_f442.rs` and the restated test.
mod f442 {
    use super::under_old_order;
    use rcfd_core::vec3::Vec3;
    use rcfd_fv::bc::FixedValue;
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::ddt::TimeScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::rectilinear_2d;
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, Connectivity, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    const DEPTH: f64 = 0.1;

    const NU_TG: f64 = 0.1;

    /// Half the side of the component square.
    const HALF: f64 = 0.6;

    /// Solver controls of every set-up (ADR 0219 PS).
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    /// ADR 0219 PS.
    fn settings(nu: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    fn tg_u(x: Vec3, t: f64) -> Vec3 {
        let e = (-2.0 * NU_TG * t).exp();
        [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
    }

    fn tg_p(x: Vec3, t: f64) -> f64 {
        -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
    }

    /// The exact velocity at `t` on the four sides.
    fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
        for name in ["xmin", "xmax", "ymin", "ymax"] {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| tg_u(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    fn tg_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
        }
    }

    fn tg_background(level: usize) -> Mesh {
        let bs = uniform(0.0, PI, 40 * level);
        rectilinear_2d(&bs, &bs, DEPTH).unwrap()
    }

    /// The overset of TG_`level`; `hole` false: no background cell is a hole (and so none is a fringe cell).
    fn tg_overset(level: usize, hole: bool) -> Overset {
        let cs = uniform(-HALF, 2.0 * HALF, 20 * level);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&tg_background(level), &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(move |x: Vec3| hole && x[0].abs().max(x[1].abs()) < 0.25),
        };
        Overset::new(composite, spec).unwrap()
    }

    /// FNV-1a over the bits of a sequence of numbers.
    fn fnv(values: impl Iterator<Item = u64>) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for v in values {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }

    fn state_fingerprint(s: &OversetSolver) -> u64 {
        let u = s.u.values.iter().flat_map(|v| v.iter().map(|x| x.to_bits()));
        let p = s.p.values.iter().map(|x| x.to_bits());
        let phi = s.phi.iter().map(|x| x.to_bits());
        fnv(u.chain(p).chain(phi))
    }

    fn conn_fingerprint(c: &Connectivity) -> u64 {
        let kinds = c.kind.iter().map(|k| *k as u64);
        let st = c.stencils.iter().flat_map(|(f, s)| {
            std::iter::once(*f as u64)
                .chain(s.cells.iter().map(|&k| k as u64))
                .chain(s.weights.iter().map(|w| w.to_bits()))
        });
        fnv(kinds.chain(st))
    }

    /// TG_1, 20 steps of 0.005, the component moving: default settings (implicit Euler, one outer iteration), or BDF2, three
    /// outer iterations and C.
    fn f0_run(bdf2_c: bool) -> OversetSolver {
        let dt = 0.005;
        let ov = tg_overset(1, true);
        let pose = tg_pose(0.0);
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let coupling = Some(OversetCoupling { overset: ov, pose });
        let mut set = settings(NU_TG);
        set.rhie_chow_step_consistent = bdf2_c;
        let mut s = OversetSolver::new(geom, u, p, set, MassMode::Conservative, coupling).unwrap();
        if bdf2_c {
            s.set_time_scheme(TimeScheme::Bdf2).unwrap();
            s.set_outer_iterations(3, 1.0, 1.0).unwrap();
        }
        for k in 1..=20 {
            let t = k as f64 * dt;
            let g = s.geom.clone();
            tg_conditions(&g, &mut s.u, t);
            s.step(dt, tg_pose(t), &[]).unwrap();
        }
        s
    }

    /// The four fingerprints of F0: TG_1 default, TG_1 with BDF2 and C, the connectivity of TG_2 at `tg_pose(0.3)` with the
    /// linear and with the quadratic stencil.
    fn f0_fingerprints() -> [u64; 4] {
        let a = state_fingerprint(&f0_run(false));
        let b = state_fingerprint(&f0_run(true));
        let mut ov = tg_overset(2, true);
        let pose = tg_pose(0.3);
        let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
        let c = conn_fingerprint(&ov.connectivity(&moved, &pose).unwrap());
        ov.set_donor_interpolation(DonorInterpolation::Quadratic);
        let d = conn_fingerprint(&ov.connectivity(&moved, &pose).unwrap());
        let f = [a, b, c, d];
        println!(
            "F0 fingerprints: {:#018x} {:#018x} {:#018x} {:#018x}",
            f[0], f[1], f[2], f[3]
        );
        f
    }

    /// The values of ADR 0359 F0: TG_1 default, TG_1 with BDF2 and C, the two connectivities of TG_2.
    const BASE: [u64; 4] = [
        0x3dee_ab3c_fda1_d8d0,
        0x4758_f6c2_1597_0201,
        0xc931_bf94_74a9_7f49,
        0xac84_b084_2670_29d0,
    ];
    /// The values under the default of ADR 0743 (gate run 44599067147): the two flow runs moved, the connectivities did not.
    const NEW: [u64; 4] = [
        0xb66f_8e86_5def_b9eb,
        0x87e9_c7c8_539a_0a98,
        0xc931_bf94_74a9_7f49,
        0xac84_b084_2670_29d0,
    ];

    /// ADR 0748 N5: `overset_motion_f442::f0_defaults_fingerprint_equals_the_base_tree` under the default.
    #[test]
    fn t5_f0_default() {
        assert_eq!(f0_fingerprints(), NEW, "N5: F0 under the default");
    }

    /// ADR 0748 W5: the same under the old order gives the values of ADR 0359.
    #[test]
    fn t5_f0_old_order() {
        assert_eq!(under_old_order(f0_fingerprints), BASE, "W5: F0 under the old order");
    }
}

/// Copies from `overset_f338.rs` and the restated test.
mod f338 {
    use super::under_old_order;
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::bc::FixedValue;
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::ddt::TimeScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::rectilinear_2d;
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, CellKind, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;
    use std::time::Instant;

    const DEPTH: f64 = 0.1;

    /// Solver controls of every set-up (ADR 0219 PS).
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    /// ADR 0219 PS.
    fn settings(nu: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
        let cs = uniform(-half, 2.0 * half, n);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
        };
        Overset::new(composite, spec).unwrap()
    }

    fn push(bytes: &mut Vec<u8>, x: f64) {
        bytes.extend_from_slice(&x.to_bits().to_le_bytes());
    }

    /// The hash of ADR 0238 R2: a solver's state and the `delta` of every step.
    fn state_hash(s: &OversetSolver) -> String {
        let mut bytes = Vec::new();
        for v in &s.u.values {
            for x in v {
                push(&mut bytes, *x);
            }
        }
        for x in s.p.values.iter().chain(&s.phi) {
            push(&mut bytes, *x);
        }
        push(&mut bytes, s.time);
        bytes.extend_from_slice(&(s.history.len() as u64).to_le_bytes());
        for h in &s.history {
            push(&mut bytes, h.delta);
        }
        format!("{:016x}", rcfd_core::hash::fnv64(&bytes))
    }

    const NU_TG: f64 = 0.1;

    const T_END: f64 = 0.5;

    fn tg_u(x: Vec3, t: f64) -> Vec3 {
        let e = (-2.0 * NU_TG * t).exp();
        [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
    }

    fn tg_p(x: Vec3, t: f64) -> f64 {
        -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
    }

    fn tg_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
        }
    }

    fn tg_background(level: usize) -> Mesh {
        let bs = uniform(0.0, PI, 40 * level);
        rectilinear_2d(&bs, &bs, DEPTH).unwrap()
    }

    fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
        for name in ["xmin", "xmax", "ymin", "ymax"] {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| tg_u(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    const U_FLOW: Vec3 = [1.0, 0.5, 0.0];

    const DT_U: f64 = 0.01;

    fn u_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [t, 0.5 * t, 0.0],
        }
    }

    fn u_new() -> OversetSolver {
        u_new_at(u_pose(0.0))
    }

    /// The set-up U with the component at `pose`.
    fn u_new_at(pose: Pose) -> OversetSolver {
        let bs = uniform(-2.5, 5.0, 50);
        let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::uniform("U", &geom, U_FLOW);
        for name in ["xmin", "ymin", "ymax"] {
            u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        OversetSolver::new(
            geom,
            u,
            p,
            settings(0.01),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap()
    }

    /// One run of TG_L: the solver at `t = 0.5`, the error `e` of ADR 0219 (largest over the non-hole cells), the largest
    /// continuity, the range of `delta`, the pressure iterations and the unconverged linear solves over the steps, the sum
    /// over the steps of the cells that took the implicit Euler row, the wall time.
    struct Run {
        s: OversetSolver,
        e: f64,
        cont: f64,
        delta: (f64, f64),
        p_iters: usize,
        unconverged: usize,
        fallback: usize,
        secs: f64,
    }

    /// TG_L in `steps` steps to `t = 0.5`; `prepare` is applied to the solver before the first step.
    fn tg_run_with(level: usize, steps: usize, tag: &str, prepare: impl Fn(&mut OversetSolver)) -> Run {
        let t0 = Instant::now();
        let dt = T_END / steps as f64;
        let ov = box_overset(tg_background(level), 0.6, 20 * level, 0.25);
        let pose = tg_pose(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(NU_TG),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        prepare(&mut s);
        let (mut cont, mut dlo, mut dhi, mut p_iters, mut unconverged, mut fallback) =
            (0.0f64, f64::INFINITY, f64::NEG_INFINITY, 0usize, 0usize, 0usize);
        for k in 1..=steps {
            let t = k as f64 * dt;
            let g = s.geom.clone();
            tg_conditions(&g, &mut s.u, t);
            let info = s.step(dt, tg_pose(t), &[]).unwrap();
            cont = cont.max(info.continuity);
            dlo = dlo.min(info.delta);
            dhi = dhi.max(info.delta);
            p_iters += info.p_iters;
            unconverged += info.unconverged;
            fallback += s.euler_fallback_cells();
        }
        let decay = (-2.0 * NU_TG * T_END).exp();
        let e = (0..s.geom.n_cells())
            .filter(|&c| s.conn.kind[c] != CellKind::Hole)
            .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], T_END))) / decay)
            .fold(0.0, f64::max);
        let r = Run {
            s,
            e,
            cont,
            delta: (dlo, dhi),
            p_iters,
            unconverged,
            fallback,
            secs: t0.elapsed().as_secs_f64(),
        };
        println!(
            "TG_{level} {steps} steps, {tag} ({:.1} s): e {:.4e}, continuity {:.2e}, delta [{:.3e}, {:.3e}], pressure iterations per step {:.1}, unconverged solves {}, cells with the implicit Euler row (summed) {}",
            r.secs,
            r.e,
            r.cont,
            r.delta.0,
            r.delta.1,
            r.p_iters as f64 / steps as f64,
            r.unconverged,
            r.fallback
        );
        r
    }

    const R2A: &str = "7aa14d1e2e2da052";

    const R2B: &str = "75ac2daf13879872";

    fn check_hash(name: &str, got: &str, want: &str) {
        println!("{name}: {got}");
        if cfg!(windows) {
            assert_eq!(got, want, "{name}");
        }
    }

    /// TG_1, 200 steps, under the default of ADR 0743 (gate run 44599067147); R2B is the value of ADR 0238.
    const TG_NEW: &str = "a126cd75e3d481d7";

    /// The four hashes of ADR 0241 R2e, in the order of its checks: U and TG_1 after `set_time_scheme(ImplicitEuler)`, directly
    /// and after `set_time_scheme(Bdf2)` (the body of `r2e_implicit_euler_chosen_by_the_method_gives_the_records`).
    fn r2e_hashes() -> [String; 4] {
        let euler = |s: &mut OversetSolver| s.set_time_scheme(TimeScheme::ImplicitEuler).unwrap();
        let back = |s: &mut OversetSolver| {
            s.set_time_scheme(TimeScheme::Bdf2).unwrap();
            s.set_time_scheme(TimeScheme::ImplicitEuler).unwrap();
        };
        let u_hash = |prepare: &dyn Fn(&mut OversetSolver)| {
            let mut s = u_new();
            prepare(&mut s);
            for k in 1..=80 {
                s.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
            }
            state_hash(&s)
        };
        [
            u_hash(&euler),
            u_hash(&back),
            state_hash(&tg_run_with(1, 200, "implicit Euler chosen", euler).s),
            state_hash(&tg_run_with(1, 200, "BDF2 then implicit Euler", back).s),
        ]
    }

    const NAMES: [&str; 4] = [
        "R2e U, implicit Euler chosen",
        "R2e U, BDF2 then implicit Euler",
        "R2e TG_1, implicit Euler chosen",
        "R2e TG_1, BDF2 then implicit Euler",
    ];

    /// ADR 0748 N6: R2e under the default: U as registered, TG_1 the new value in both checks.
    #[test]
    fn t6_r2e_default() {
        let h = r2e_hashes();
        println!("N6 R2e, default: {h:?}");
        for (i, want) in [R2A, R2A, TG_NEW, TG_NEW].into_iter().enumerate() {
            check_hash(&format!("N6 {}, default", NAMES[i]), &h[i], want);
        }
    }

    /// ADR 0748 W6: the same under the old order gives the records of ADR 0238.
    #[test]
    fn t6_r2e_old_order() {
        let h = under_old_order(r2e_hashes);
        println!("W6 R2e, old order: {h:?}");
        for (i, want) in [R2A, R2A, R2B, R2B].into_iter().enumerate() {
            check_hash(&format!("W6 {}, old order", NAMES[i]), &h[i], want);
        }
    }
}

/// Copies from `overset_f450.rs` and the restated tests.
mod f450 {
    use super::under_old_order;
    use rcfd_core::vec3::Vec3;
    use rcfd_fv::bc::FixedValue;
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::FringeFlux;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::rectilinear_2d;
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    const DEPTH: f64 = 0.1;

    const NU_TG: f64 = 0.1;

    /// ADR 0219 PS.
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    const DEFAULT: FringeFlux = FringeFlux::PressureEquation;

    const Q: DonorInterpolation = DonorInterpolation::Quadratic;

    const L: DonorInterpolation = DonorInterpolation::Linear;

    const SIDES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];

    fn settings(nu: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    /// TG_L of ADR 0219 with the donor interpolation `mode`.
    fn tg_overset(level: usize, mode: DonorInterpolation) -> Overset {
        let bs = uniform(0.0, PI, 40 * level);
        let bg: Mesh = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
        let cs = uniform(-0.6, 1.2, 20 * level);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
        };
        let mut ov = Overset::new(composite, spec).unwrap();
        if mode != DonorInterpolation::Linear {
            ov.set_donor_interpolation(mode);
        }
        ov
    }

    fn tg_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
        }
    }

    fn tg_u(x: Vec3, t: f64) -> Vec3 {
        let e = (-2.0 * NU_TG * t).exp();
        [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
    }

    fn tg_p(x: Vec3, t: f64) -> f64 {
        -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
    }

    fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    /// Set the exact velocity at time `t` on the four background sides.
    fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
        for name in SIDES {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| tg_u(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// `state_hash` of `overset_f445.rs`.
    fn state_hash(s: &OversetSolver) -> u64 {
        let mut bytes = Vec::new();
        for v in &s.u.values {
            for x in v {
                bytes.extend_from_slice(&x.to_bits().to_le_bytes());
            }
        }
        for x in s.p.values.iter().chain(&s.phi) {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&s.time.to_bits().to_le_bytes());
        rcfd_core::hash::fnv64(&bytes)
    }

    /// A TG_L solver at `t = 0` with the fringe mode `mode` and the donor interpolation `donors`.
    fn tg_solver(level: usize, mode: FringeFlux, donors: DonorInterpolation) -> OversetSolver {
        let ov = tg_overset(level, donors);
        let pose = tg_pose(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(NU_TG),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        if mode != DEFAULT {
            s.set_fringe_flux(mode);
        }
        s
    }

    /// 40 steps of TG_1 moving with the default coupling and the donor interpolation `donors`.
    fn b1_hash(donors: DonorInterpolation) -> u64 {
        let mut s = tg_solver(1, DEFAULT, donors);
        let dt = 0.0025;
        for k in 1..=40 {
            let t = k as f64 * dt;
            let g = s.geom.clone();
            tg_conditions(&g, &mut s.u, t);
            s.step(dt, tg_pose(t), &[]).unwrap();
        }
        state_hash(&s)
    }

    /// Measured once on the registration commit 4407610a (the library of main 56a14aa5) before any other run.
    const B1_QUADRATIC: u64 = 0xe43a026a778e83b3;

    const B1_LINEAR: u64 = 0x9c5ab1e5b21d5368;

    /// The quadratic stencils under the default of ADR 0743 (gate run 44599067147).
    const NEW_QUADRATIC: u64 = 0x9f56741b853c367b;

    /// The linear stencils under the default of ADR 0743: measured by the run of N7 on 0a1abe82 (ADR 0748 addendum 1), the
    /// move approved by the maintainer's second answer of 2026-11-20.
    const NEW_LINEAR: u64 = 0xb7eacfdad14044af;

    /// ADR 0748 N7a (amendment 1): `overset_f450::b1_defaults_bit_identical` under the default, with both new values.
    #[test]
    fn t7_b1_default_amended() {
        let (q, l) = (b1_hash(Q), b1_hash(L));
        println!("N7a B1, default: quadratic {q:016x}, linear {l:016x}");
        assert_eq!(q, NEW_QUADRATIC, "N7a: quadratic stencils under the default");
        assert_eq!(l, NEW_LINEAR, "N7a: linear stencils under the default");
    }

    /// ADR 0748 N7: `overset_f450::b1_defaults_bit_identical` under the default: the quadratic stencils the new value, the
    /// linear stencils as registered (the gate did not reach them).
    #[test]
    #[ignore = "known miss, ADR 0748 amendment 1: the linear stencils moved as well (0x9c5ab1e5b21d5368 to 0xb7eacfdad14044af); restated by t7_b1_default_amended"]
    fn t7_b1_default_known_miss() {
        let (q, l) = (b1_hash(Q), b1_hash(L));
        println!("N7 B1, default: quadratic {q:016x}, linear {l:016x}");
        assert_eq!(q, NEW_QUADRATIC, "N7: quadratic stencils under the default");
        assert_eq!(l, B1_LINEAR, "N7: linear stencils under the default");
    }

    /// ADR 0748 W7: the same under the old order gives the values of ADR 0376.
    #[test]
    fn t7_b1_old_order() {
        let (q, l) = under_old_order(|| (b1_hash(Q), b1_hash(L)));
        println!("W7 B1, old order: quadratic {q:016x}, linear {l:016x}");
        assert_eq!(q, B1_QUADRATIC, "W7: quadratic stencils under the old order");
        assert_eq!(l, B1_LINEAR, "W7: linear stencils under the old order");
    }
}

/// Copies from `overset_f368.rs` and the restated test.
mod f368 {
    use super::under_old_order;
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::ale::move_mesh;
    use rcfd_fv::bc::{FixedValue, Profile};
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::ddt::TimeScheme;
    use rcfd_fv::overset::{FringeFlux, MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, warp, Grading};
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::sync::Arc;

    /// ADR 0219 PS.
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    const NU: f64 = 0.01;

    const U0: Vec3 = [1.0, 0.5, 0.25];

    const X0: Vec3 = [1.2, 1.2, 1.2];

    const SHEAR: f64 = 0.5;

    const DT: f64 = 0.005;

    const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

    fn settings() -> PimpleSettings {
        let mut s = PimpleSettings::piso(NU);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    /// The shear direction `a = (1, 2, 1) / sqrt(6)` and the gradient direction `b = (0, 1, -2) / sqrt(5)`.
    fn dirs() -> (Vec3, Vec3) {
        (vec3::unit([1.0, 2.0, 1.0]), vec3::unit([0.0, 1.0, -2.0]))
    }

    /// The flow F of ADR 0264.
    fn u_ex(x: Vec3) -> Vec3 {
        let (a, b) = dirs();
        vec3::add(U0, vec3::scale(a, SHEAR * vec3::dot(b, vec3::sub(x, X0))))
    }

    /// A fixed pseudo-random number in [-1, 1] of a position.
    fn jitter(x: Vec3, seed: f64) -> f64 {
        let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
        2.0 * (v - v.floor()) - 1.0
    }

    fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
        let dirs = dirs.to_vec();
        warp(mesh, move |p| {
            let tol = 1e-9 * (hi - lo);
            if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
                return p;
            }
            let key = if planar { [p[0], p[1], 0.0] } else { p };
            let mut q = p;
            for &d in &dirs {
                q[d] += amp * jitter(key, 1.0 + d as f64);
            }
            q
        })
        .unwrap()
    }

    /// The background of B3 (`distort`: of B3d).
    fn background(distort: bool) -> Mesh {
        let bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false)
        } else {
            bg
        }
    }

    /// The component of B3 (`distort`: of B3d).
    fn component(distort: bool) -> Mesh {
        let comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
        if distort {
            distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false)
        } else {
            comp
        }
    }

    /// The overset of ADR 0251 from a background and a component, with the donor interpolation `mode`.
    fn overset(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> Overset {
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
                .map(String::from)
                .to_vec(),
            hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
        };
        let mut ov = Overset::new(compose(bg, comp, "c_").unwrap(), spec).unwrap();
        if mode != DonorInterpolation::Linear {
            ov.set_donor_interpolation(mode);
        }
        ov
    }

    /// The motion of ADR 0255: `P(0)` is the pose of ADR 0251.
    fn pose(t: f64) -> Pose {
        Pose {
            origin: [0.7; 3],
            axis: vec3::unit([1.0, 2.0, 3.0]),
            angle: 0.3 + 0.5 * t,
            translation: [0.53 - 0.25 * t, 0.52 + 0.25 * t, 0.49 + 0.25 * t],
        }
    }

    /// `u_ex` at the face centres of `at` on the six patches (the patch faces of `at` and `geom` are the same).
    fn set_dirichlet(u: &mut VectorField, geom: &FvGeometry, at: &FvGeometry) {
        for name in PATCHES {
            let v: Vec<Vec3> = at
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| u_ex(at.mesh.face_centres()[f]))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// The shear flow F with its boundary values (ADR 0264).
    fn shear_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
        let mut u = VectorField::from_fn("U", geom, u_ex);
        set_dirichlet(&mut u, geom, geom);
        let p = ScalarField::uniform("p", geom, 0.0);
        (u, p)
    }

    /// The uniform flow of ADR 0255 with its boundary values.
    fn uniform_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
        let mut u = VectorField::uniform("U", geom, U0);
        for name in ["xmin", "ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(geom, name, Box::new(FixedValue::vector(U0))).unwrap();
        }
        let mut p = ScalarField::uniform("p", geom, 0.0);
        p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        (u, p)
    }

    /// An overset solver at `P(0)` on `bg` and `comp` with the fields of `flow`.
    fn overset_solver(
        bg: &Mesh,
        comp: &Mesh,
        mode: DonorInterpolation,
        s: PimpleSettings,
        flow: fn(&Arc<FvGeometry>) -> (VectorField, ScalarField),
    ) -> OversetSolver {
        let ov = overset(bg, comp, mode);
        let p0 = pose(0.0);
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let (u, p) = flow(&geom);
        OversetSolver::new(
            geom,
            u,
            p,
            s,
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose: p0 }),
        )
        .unwrap()
    }

    /// FNV-1a over bytes.
    struct Fnv(u64);

    impl Fnv {
        fn new() -> Self {
            Fnv(0xcbf2_9ce4_8422_2325)
        }
        fn bytes(&mut self, b: &[u8]) {
            for &x in b {
                self.0 ^= x as u64;
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn f64s(&mut self, v: impl IntoIterator<Item = f64>) {
            for x in v {
                self.bytes(&x.to_bits().to_le_bytes());
            }
        }
    }

    /// The fingerprint of a solver's state.
    fn state(h: &mut Fnv, s: &OversetSolver) {
        for k in 0..3 {
            h.f64s(s.u.values.iter().map(|v| v[k]));
        }
        h.f64s(s.p.values.iter().copied());
        h.f64s(s.phi.iter().copied());
    }

    /// 5 steps of an overset solver; a failing step folds its message and ends the run.
    fn overset_fingerprint(mut s: OversetSolver) -> u64 {
        let mut h = Fnv::new();
        for k in 1..=5 {
            match s.step(DT, pose(k as f64 * DT), &[]) {
                Ok(_) => state(&mut h, &s),
                Err(e) => {
                    h.bytes(format!("step {k}: {e}").as_bytes());
                    break;
                }
            }
        }
        h.0
    }

    /// The eight fingerprints of D1 (ADR 0367), in the registered order.
    fn d1_fingerprints() -> [u64; 8] {
        let (bg, comp) = (background(true), component(true));
        let lin = DonorInterpolation::Linear;
        let f1 = overset_fingerprint(overset_solver(&bg, &comp, lin, settings(), shear_fields));
        let f2 = overset_fingerprint(overset_solver(
            &bg,
            &comp,
            DonorInterpolation::Quadratic,
            settings(),
            shear_fields,
        ));
        let f3 = {
            let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
            s.set_outer_iterations(2, 0.8, 0.5).unwrap();
            overset_fingerprint(s)
        };
        let f4 = {
            let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
            s.set_time_scheme(TimeScheme::Bdf2).unwrap();
            overset_fingerprint(s)
        };
        let f5 = {
            let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
            s.set_fringe_flux(FringeFlux::PrescribedOwnGradient);
            overset_fingerprint(s)
        };
        let f6 = {
            let mut st = settings();
            st.rhie_chow_step_consistent = true;
            overset_fingerprint(overset_solver(&bg, &comp, lin, st, shear_fields))
        };
        let f7 = {
            let reference = component(true);
            let at = |t: f64| -> Vec<Vec3> { reference.points().iter().map(|&x| pose(t).apply(x)).collect() };
            let mesh = moved_mesh(&reference, at(0.0)).unwrap();
            let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
            let (u, p) = shear_fields(&geom);
            let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
            let mut h = Fnv::new();
            for k in 1..=5 {
                let ms = move_mesh(&s.geom, at(k as f64 * DT), DT).unwrap();
                let g = s.geom.clone();
                set_dirichlet(&mut s.u, &g, &ms.geom);
                match s.advance_mesh(DT, ms, Vec::new()) {
                    Ok(_) => state(&mut h, &s),
                    Err(e) => {
                        h.bytes(format!("step {k}: {e}").as_bytes());
                        break;
                    }
                }
            }
            h.0
        };
        let f8 = overset_fingerprint(overset_solver(&bg, &comp, lin, settings(), uniform_fields));
        [f1, f2, f3, f4, f5, f6, f7, f8]
    }

    /// The values of ADR 0367 D1, F1 to F8.
    const BASE: [u64; 8] = [
        0x7ed9_37a9_e05b_b338,
        0xbf99_f3bc_07e5_fdff,
        0x3150_9ceb_0d02_8a2f,
        0xcd2e_f9eb_d8dc_9295,
        0xca08_f2d7_cb22_ffe7,
        0xabcd_93b8_59c1_b26e,
        0xa489_4813_b5a6_f2ed,
        0x8b2e_bdb2_82e0_7af2,
    ];
    /// The values under the default of ADR 0743 (gate run 44599067147): F1 to F4 and F6 moved; F5 (the prescribed fringe
    /// flux), F7 (no coupling) and F8 (the uniform flow) did not.
    const NEW: [u64; 8] = [
        0x2979_fa92_9db1_666b,
        0x0cf8_510a_c96f_d9bd,
        0xb13f_7bbe_8098_4a00,
        0x2751_6ddc_9194_8f05,
        0xca08_f2d7_cb22_ffe7,
        0x073b_b140_83b5_cfdf,
        0xa489_4813_b5a6_f2ed,
        0x8b2e_bdb2_82e0_7af2,
    ];

    /// ADR 0748 N8: `overset_f368::d1_defaults_fingerprint_equals_the_base_tree` under the default.
    #[test]
    fn t8_d1_default() {
        let f = d1_fingerprints();
        println!("N8 D1, default: {f:#018x?}");
        assert_eq!(f, NEW, "N8: D1 under the default");
    }

    /// ADR 0748 W8: the same under the old order gives the values of ADR 0367.
    #[test]
    fn t8_d1_old_order() {
        let f = under_old_order(d1_fingerprints);
        println!("W8 D1, old order: {f:#018x?}");
        assert_eq!(f, BASE, "W8: D1 under the old order");
    }
}

/// Copies from `overset_f479.rs` and the restated tests.
mod f479 {
    use super::under_old_order;
    use rcfd_core::vec3::Vec3;
    use rcfd_fv::bc::FixedValue;
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::FringeFlux;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::rectilinear_2d;
    use rcfd_mesh::motion::{moved_mesh, Pose};
    use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
    use rcfd_mesh::Mesh;
    use std::f64::consts::PI;
    use std::sync::Arc;

    const DEPTH: f64 = 0.1;

    const NU_TG: f64 = 0.1;

    /// ADR 0219 PS.
    const SOLVE: SolveControl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-12,
        max_iter: 3000,
    };

    const QD: DonorInterpolation = DonorInterpolation::Quadratic;

    const LD: DonorInterpolation = DonorInterpolation::Linear;

    const SIDES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];

    /// The couplings compared: the default, G of ADR 0347 (D5) and G with the second-order ring gradient of ADR 0409 (D6).
    #[allow(dead_code)] // copied whole; this file does not use every part of it
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Mode {
        Default,
        G,
        Q,
    }

    fn apply(s: &mut OversetSolver, mode: Mode) {
        match mode {
            Mode::Default => {}
            Mode::G => s.set_fringe_flux(FringeFlux::PrescribedOwnGradient),
            Mode::Q => {
                s.set_fringe_flux(FringeFlux::PrescribedOwnGradient);
                s.set_second_order_ring_gradient(true);
            }
        }
    }

    fn settings(nu: f64) -> PimpleSettings {
        let mut s = PimpleSettings::piso(nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = SOLVE;
        s.pressure_solver = SOLVE;
        s.linear.amg_coarse_lu_interleaved = super::old_order();
        s
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    fn tg_overset(level: usize, donors: DonorInterpolation) -> Overset {
        let bs = uniform(0.0, PI, 40 * level);
        let bg: Mesh = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
        let cs = uniform(-0.6, 1.2, 20 * level);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
        };
        let mut ov = Overset::new(composite, spec).unwrap();
        if donors != DonorInterpolation::Linear {
            ov.set_donor_interpolation(donors);
        }
        ov
    }

    fn tg_pose(t: f64) -> Pose {
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: t,
            translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
        }
    }

    fn tg_u(x: Vec3, t: f64) -> Vec3 {
        let e = (-2.0 * NU_TG * t).exp();
        [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
    }

    fn tg_p(x: Vec3, t: f64) -> f64 {
        -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
    }

    fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
        for name in SIDES {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| tg_u(geom.mesh.face_centres()[f], t))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    /// `state_hash` of `overset_f445.rs`.
    fn state_hash(s: &OversetSolver) -> u64 {
        let mut bytes = Vec::new();
        for v in &s.u.values {
            for x in v {
                bytes.extend_from_slice(&x.to_bits().to_le_bytes());
            }
        }
        for x in s.p.values.iter().chain(&s.phi) {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&s.time.to_bits().to_le_bytes());
        rcfd_core::hash::fnv64(&bytes)
    }

    fn tg_solver(level: usize, mode: Mode, donors: DonorInterpolation) -> OversetSolver {
        let ov = tg_overset(level, donors);
        let pose = tg_pose(0.0);
        let geom = geometry_at(&ov, &pose);
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let mut s = OversetSolver::new(
            geom,
            u,
            p,
            settings(NU_TG),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose }),
        )
        .unwrap();
        apply(&mut s, mode);
        s
    }

    /// 40 steps of TG_1 moving (`b1_hash` of `overset_f450.rs`) under `mode`; `explicit_off` switches the opt-in off by a call.
    fn hash_40(mode: Mode, donors: DonorInterpolation, explicit_off: bool) -> u64 {
        let mut s = tg_solver(1, mode, donors);
        if explicit_off {
            s.set_second_order_ring_gradient(false);
        }
        let dt = 0.0025;
        for k in 1..=40 {
            let t = k as f64 * dt;
            let g = s.geom.clone();
            tg_conditions(&g, &mut s.u, t);
            s.step(dt, tg_pose(t), &[]).unwrap();
        }
        state_hash(&s)
    }

    /// The hashes of ADR 0376 B1 (default coupling; library of main 56a14aa5).
    const B1_QUADRATIC: u64 = 0xe43a026a778e83b3;

    const B1_LINEAR: u64 = 0x9c5ab1e5b21d5368;

    /// G without the opt-in, quadratic stencils: measured once on the registration commit eb13160d (unedited library) before
    /// any other run (ADR 0409 A6; log `w49i-work/f479-scratch/b-g-base.log`).
    const G_EB13160D: u64 = 0xb2b43812eb166b04;

    /// The default coupling with the quadratic stencils under the default of ADR 0743 (gate run 44599067147).
    const NEW_QUADRATIC: u64 = 0x9f56741b853c367b;

    /// The four hashes of ADR 0409 A6, in the order of its assertions: the default coupling with the quadratic and with the
    /// linear stencils, G, G with the opt-in set off by a call (the body of `a6_without_the_opt_in_every_bit_is_kept`).
    fn a6_hashes() -> [u64; 4] {
        [
            hash_40(Mode::Default, QD, false),
            hash_40(Mode::Default, LD, false),
            hash_40(Mode::G, QD, false),
            hash_40(Mode::G, QD, true),
        ]
    }

    /// The linear stencils and G under the default of ADR 0743: measured by the run of N9 on 0a1abe82 (ADR 0748 addendum 1),
    /// the moves approved by the maintainer's second answer of 2026-11-20.
    const NEW_LINEAR: u64 = 0xb7eacfdad14044af;
    const NEW_G: u64 = 0xe8f1628a95798aab;

    /// ADR 0748 N9a (amendment 1): A6 under the default, with the four new values.
    #[test]
    fn t9_a6_default_amended() {
        let h = a6_hashes();
        println!("N9a A6, default: {h:#018x?}");
        assert_eq!(
            h,
            [NEW_QUADRATIC, NEW_LINEAR, NEW_G, NEW_G],
            "N9a: A6 under the default"
        );
    }

    /// ADR 0748 N9: A6 under the default: the quadratic stencils the new value; the linear stencils and both G values as
    /// registered (the gate did not reach them).
    #[test]
    #[ignore = "known miss, ADR 0748 amendment 1: the linear stencils and G moved as well (0x9c5ab1e5b21d5368 to 0xb7eacfdad14044af, 0xb2b43812eb166b04 to 0xe8f1628a95798aab); restated by t9_a6_default_amended"]
    fn t9_a6_default_known_miss() {
        let h = a6_hashes();
        println!("N9 A6, default: {h:#018x?}");
        assert_eq!(h[0], NEW_QUADRATIC, "N9: default, quadratic stencils");
        assert_eq!(h[1], B1_LINEAR, "N9: default, linear stencils");
        assert_eq!(h[2], G_EB13160D, "N9: G");
        assert_eq!(h[3], G_EB13160D, "N9: G, opt-in set off by a call");
    }

    /// ADR 0748 W9: the same under the old order gives the values of ADR 0409.
    #[test]
    fn t9_a6_old_order() {
        let h = under_old_order(a6_hashes);
        println!("W9 A6, old order: {h:#018x?}");
        assert_eq!(
            h,
            [B1_QUADRATIC, B1_LINEAR, G_EB13160D, G_EB13160D],
            "W9: A6 under the old order"
        );
    }
}
