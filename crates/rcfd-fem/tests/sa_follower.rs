//! F-424 (ADR 0480): the follower pressure and its load stiffness (S1 to S4), the thin ring under internal pressure (R1 to
//! R4), the ring buckling with and without the load stiffness (B1 to B3), and the modes about a nonlinear state (M1 to M6).
//! Each registered run is made once; a miss stays as an ignored `*_known_miss` test with its limit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::follower::{buckling_factors, tangent_modes, CentrifugalFollower, FollowerPressure};
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{initial_states, solve, solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, FaceRef, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

fn dense(n: usize, triplets: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut k = vec![0.0; n * n];
    for &(r, c, v) in triplets {
        k[r * n + c] += v;
    }
    k
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

// ------------------------------------------------------------------------------------------------------- S1 to S4

/// One element of `kind` with the surface set "p" of one face.
fn element_model(kind: ElementKind) -> SolidModel {
    let mesh = match kind {
        ElementKind::Hex8 | ElementKind::Hex20 => {
            let mut m = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
            m.add_surface_set("p", m.surface_set("zmax").unwrap().to_vec()).unwrap();
            if kind == ElementKind::Hex20 {
                m = upgrade_to_hex20(&m).unwrap().0;
            }
            m
        }
        _ => {
            let mut m =
                SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
            m.add_block(ElementKind::Tet4, vec![0, 1, 2, 3]).unwrap();
            m.add_element_set("all", vec![0]).unwrap();
            m.add_surface_set("p", vec![FaceRef { element: 0, face: 0 }]).unwrap();
            if kind == ElementKind::Tet10 {
                m = upgrade_to_tet10(&m).unwrap().0;
            }
            m
        }
    };
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material()).unwrap();
    model
}

const P_S: f64 = 3.0e7;

#[test]
fn s1_s3_s4_load_stiffness_against_central_differences() {
    let mut ck = Checks::new();
    let mut seed = 0xF424_0480_u64;
    for kind in [
        ElementKind::Hex8,
        ElementKind::Hex20,
        ElementKind::Tet4,
        ElementKind::Tet10,
    ] {
        let model = element_model(kind);
        let n = model.dof_map().n_equations();
        let fp = FollowerPressure::new(&model, "p", P_S).unwrap();
        let u: Vec<f64> = (0..n).map(|_| 0.05 * (2.0 * uniform(&mut seed) - 1.0)).collect();
        let k = dense(n, &fp.load_stiffness(&u, 1.0).unwrap());
        let h = 1e-6;
        let mut kcd = vec![0.0; n * n];
        for j in 0..n {
            let mut up = u.clone();
            let mut um = u.clone();
            up[j] += h;
            um[j] -= h;
            let fpl = fp.force(&up, 1.0).unwrap();
            let fmi = fp.force(&um, 1.0).unwrap();
            for i in 0..n {
                kcd[i * n + j] = (fpl[i] - fmi[i]) / (2.0 * h);
            }
        }
        let diff: Vec<f64> = k.iter().zip(&kcd).map(|(a, b)| a - b).collect();
        let scale = max_abs(&k);
        ck.at_most(
            &format!("S1 {} max |K_p - K_cd| / max |K_p|", kind.name()),
            max_abs(&diff) / scale,
            1e-6,
        );
        let mut anti: f64 = 0.0;
        for i in 0..n {
            for j in 0..n {
                anti = anti.max((k[i * n + j] - k[j * n + i]).abs());
            }
        }
        ck.at_least(
            &format!("S3 {} max |K_p - K_p^T| / max |K_p| (free edges)", kind.name()),
            anti / scale,
            1e-3,
        );
        // S4: at u = 0 the follower load is the dead pressure, bit for bit.
        let mut dead = model.clone();
        dead.add_load(Load::pressure("p", P_S)).unwrap();
        let fd = nodal_load_vector(&dead, &[]).unwrap();
        let ff = fp.force(&vec![0.0; n], 1.0).unwrap();
        let same = fd.len() == ff.len() && fd.iter().zip(&ff).all(|(a, b)| a.to_bits() == b.to_bits());
        ck.holds(
            &format!("S4 {} force(0, 1) == dead Load::Pressure", kind.name()),
            same,
            "bits",
        );
    }
    // S4 on the ring of R1.
    let ring = ring_model(8);
    let n = ring.dof_map().n_equations();
    let fp = FollowerPressure::new(&ring, "xmin", 1.0e8).unwrap();
    let mut dead = ring.clone();
    dead.add_load(Load::pressure("xmin", 1.0e8)).unwrap();
    let fd = nodal_load_vector(&dead, &[]).unwrap();
    let ff = fp.force(&vec![0.0; n], 1.0).unwrap();
    let same = fd.iter().zip(&ff).all(|(a, b)| a.to_bits() == b.to_bits());
    ck.holds("S4 ring force(0, 1) == dead Load::Pressure", same, "bits");
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- ring

const R: f64 = 1.0;
const T: f64 = 0.02;
const BW: f64 = 0.02;

/// The quarter ring of D5 with `n` Hex20 elements over the quarter, the symmetry planes and the plane held.
fn ring_model(n: usize) -> SolidModel {
    let b = hex_box([R - T / 2.0, 0.0, 0.0], [T, PI / 2.0, BW], [1, n, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

fn radius(p: Vec3) -> f64 {
    (p[0] * p[0] + p[1] * p[1]).sqrt()
}

/// The mean of `|x + u| / R0` over the nodes at the reference radius `r0`.
fn mean_stretch(model: &SolidModel, u: &[f64], r0: f64) -> f64 {
    let dofs = model.dof_map();
    let (mut s, mut k) = (0.0, 0usize);
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if (radius(*x) - r0).abs() < 1e-9 {
            let ux = u[dofs.equation(node, 0).unwrap()];
            let uy = u[dofs.equation(node, 1).unwrap()];
            s += radius([x[0] + ux, x[1] + uy, 0.0]) / r0;
            k += 1;
        }
    }
    s / k as f64
}

fn p_thin(lambda: f64) -> f64 {
    E * T * (lambda * lambda - 1.0) / (2.0 * R)
}

/// The thick SVK ring of `nu = 0` (arithmetic 2): from the outer radius `r_o`, RK4 in `R` with 2000 steps; the follower and
/// the dead bore pressures.
fn exact_ring(r_o: f64) -> (f64, f64) {
    let (ri, ro) = (R - T / 2.0, R + T / 2.0);
    let rprime = |q_over_r: f64| {
        let mut s = 1.0f64;
        for _ in 0..60 {
            let f = 0.5 * s * (s * s - 1.0) - q_over_r / E;
            let d = 0.5 * (3.0 * s * s - 1.0);
            s -= f / d;
        }
        s
    };
    let rhs = |rr: f64, y: [f64; 2]| -> [f64; 2] {
        let s = rprime(y[1] / rr);
        let lt = y[0] / rr;
        [s, E * 0.5 * lt * (lt * lt - 1.0)]
    };
    let n = 2000;
    let h = -(ro - ri) / n as f64;
    let mut rr = ro;
    let mut y = [r_o, 0.0];
    for _ in 0..n {
        let k1 = rhs(rr, y);
        let k2 = rhs(rr + h / 2.0, [y[0] + h / 2.0 * k1[0], y[1] + h / 2.0 * k1[1]]);
        let k3 = rhs(rr + h / 2.0, [y[0] + h / 2.0 * k2[0], y[1] + h / 2.0 * k2[1]]);
        let k4 = rhs(rr + h, [y[0] + h * k3[0], y[1] + h * k3[1]]);
        for c in 0..2 {
            y[c] += h / 6.0 * (k1[c] + 2.0 * k2[c] + 2.0 * k3[c] + k4[c]);
        }
        rr += h;
    }
    (-y[1] / y[0], -y[1] / ri)
}

/// A zero contribution that records `(load factor, u)` of every call, after an optional inner contribution.
struct Recorder<'a> {
    inner: Option<&'a dyn Contribution>,
    calls: RefCell<Vec<(f64, Vec<f64>)>>,
}

impl Contribution for Recorder<'_> {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        if let Some(c) = self.inner {
            c.add(u, load_factor, force, tangent)?;
        }
        self.calls.borrow_mut().push((load_factor, u.to_vec()));
        Ok(())
    }
}

/// `p_ref` of R1 (D5): the thin law at the stretch 1.51.
fn p_ref_ring() -> f64 {
    p_thin(1.51)
}

/// The converged steps of the ring under `p_ref` (follower or dead), 20 load steps: `(lambda, p, r_o)` per step.
fn ring_path(follower: bool) -> Vec<(f64, f64, f64)> {
    let mut model = ring_model(8);
    let p_ref = p_ref_ring();
    if !follower {
        model.add_load(Load::pressure("xmin", p_ref)).unwrap();
    }
    let fp = FollowerPressure::new(&model, "xmin", p_ref).unwrap();
    let rec = Recorder {
        inner: if follower { Some(&fp) } else { None },
        calls: RefCell::new(Vec::new()),
    };
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: None,
    };
    let r = solve_with(&model, &[], &[], &NewtonOptions::new(20), ext).unwrap();
    let calls = rec.calls.borrow();
    r.steps
        .iter()
        .map(|s| {
            let u = &calls
                .iter()
                .rev()
                .find(|(l, _)| l.to_bits() == s.load_factor.to_bits())
                .unwrap()
                .1;
            let lam = mean_stretch(&model, u, R);
            let ro = mean_stretch(&model, u, R + T / 2.0) * (R + T / 2.0);
            (lam, s.load_factor * p_ref, ro)
        })
        .collect()
}

#[test]
fn r1_r2_r3_r4_ring_under_internal_pressure() {
    let mut ck = Checks::new();
    let follower = ring_path(true);
    let dead = ring_path(false);
    for (k, &(lam, p, ro)) in follower.iter().enumerate() {
        let d = p / p_thin(lam) - 1.0;
        let last = k + 1 == follower.len();
        if lam >= 1.2 || last {
            ck.at_most(
                &format!("R1 step {} lambda {lam:.5} |p/p_thin - 1|", k + 1),
                d.abs(),
                1e-2,
            );
        } else {
            ck.record(&format!("R1b step {} lambda {lam:.5} p/p_thin - 1", k + 1), d);
        }
        let (pf, _) = exact_ring(ro);
        ck.at_most(
            &format!("R2 follower step {} |p/p_exact - 1|", k + 1),
            (p / pf - 1.0).abs(),
            1e-3,
        );
    }
    for (k, &(lam, p, ro)) in dead.iter().enumerate() {
        let (_, pd) = exact_ring(ro);
        ck.at_most(
            &format!("R2 dead step {} lambda {lam:.5} |p/p_exact - 1|", k + 1),
            (p / pd - 1.0).abs(),
            1e-3,
        );
    }
    let &(lam_d, p_d, _) = dead.last().unwrap();
    ck.at_most(
        "R3 dead |p/(lambda p_thin) - 1|",
        (p_d / (lam_d * p_thin(lam_d)) - 1.0).abs(),
        2e-2,
    );
    ck.at_least("R3 dead p/p_thin - 1", p_d / p_thin(lam_d) - 1.0, 0.3);
    ck.record("R3 dead final stretch", lam_d);
    ck.at_least("R4 follower final stretch", follower.last().unwrap().0, 1.5);
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- B1 to B3, S2

const P_B: f64 = 1.0e6;

fn buckling_ring() -> (SolidModel, Prestress) {
    let mut m = ring_model(24);
    m.add_load(Load::pressure("xmax", P_B)).unwrap();
    let s = solve_static(&m, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&m, &s).unwrap();
    (m, pre)
}

/// Correlation of the radial displacement of the mid-surface nodes with `cos 2 theta`.
fn cos2_correlation(model: &SolidModel, mode: &[f64]) -> f64 {
    let dofs = model.dof_map();
    let (mut uf, mut uu, mut ff) = (0.0, 0.0, 0.0);
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if (radius(*x) - R).abs() < 1e-9 {
            let r = radius(*x);
            let ur = (mode[dofs.equation(node, 0).unwrap()] * x[0] + mode[dofs.equation(node, 1).unwrap()] * x[1]) / r;
            let f = (2.0 * x[1].atan2(x[0])).cos();
            uf += ur * f;
            uu += ur * ur;
            ff += f * f;
        }
    }
    uf.abs() / (uu * ff).sqrt()
}

#[test]
fn b1_b2_b3_s2_ring_buckling() {
    let mut ck = Checks::new();
    let (m, pre) = buckling_ring();
    let ei_r3 = E * T * T * T / 12.0 / (R * R * R);
    let fp = FollowerPressure::new(&m, "xmax", P_B).unwrap();
    // S2: the reduced load stiffness is symmetric.
    let n = m.dof_map().n_equations();
    let zero = vec![0.0; n];
    let kp = CsrMatrix::from_triplets(n, fp.load_stiffness(&zero, 1.0).unwrap());
    let fixed = prescribed_equations(&m, &[]).unwrap();
    let red = reduce(&kp, &vec![0.0; n], &fixed).unwrap().matrix;
    let nf = red.n;
    let mut d = vec![0.0; nf * nf];
    for r in 0..nf {
        for k in red.row_ptr[r]..red.row_ptr[r + 1] {
            d[r * nf + red.col[k]] += red.val[k];
        }
    }
    let mut anti: f64 = 0.0;
    for i in 0..nf {
        for j in 0..nf {
            anti = anti.max((d[i * nf + j] - d[j * nf + i]).abs());
        }
    }
    ck.at_most("S2 reduced max |K_p - K_p^T| / max |K_p|", anti / max_abs(&d), 1e-11);
    let with = buckling_factors(&m, &pre, &[&fp], &BucklingOptions::new(2)).unwrap();
    let without = factors(&m, &pre, &BucklingOptions::new(2)).unwrap();
    let p1 = with.factors[0] * P_B;
    let p2 = without.factors[0] * P_B;
    println!(
        "B factors with {:?}, without {:?}; p_cr/(E I/R^3) = {:.6}, {:.6}",
        with.factors,
        without.factors,
        p1 / ei_r3,
        p2 / ei_r3
    );
    ck.at_most(
        "B1 |p_cr/(3 E I/R^3) - 1| with the load stiffness",
        (p1 / (3.0 * ei_r3) - 1.0).abs(),
        2e-2,
    );
    ck.at_most(
        "B2 |p_cr/(4 E I/R^3) - 1| without",
        (p2 / (4.0 * ei_r3) - 1.0).abs(),
        3e-2,
    );
    ck.at_least(
        "B3 cos 2 theta correlation, with",
        cos2_correlation(&m, &with.modes[0]),
        0.99,
    );
    ck.at_least(
        "B3 cos 2 theta correlation, without",
        cos2_correlation(&m, &without.modes[0]),
        0.99,
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- M1, M2, M6

const LB: f64 = 2.0;
const HB: f64 = 0.1;

/// Beam B of F-395, clamped at both ends in x and y, `u_x = stretch L` at `xmax`, the plane held.
fn clamped_beam(stretch: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [LB, HB, HB], [40, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..2 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("xmax", 0, stretch * LB))
        .unwrap();
    m.add_constraint(Constraint::displacement("xmax", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

fn beam_modal() -> ModalOptions {
    ModalOptions::new(2).residual_test(ResidualTest::Backward)
}

/// `(f_T, f_L, f_0)` for the end stretch `eps`.
fn tension_frequencies(eps: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let me = clamped_beam(eps);
    let m0 = clamped_beam(0.0);
    let r = solve(&me, &[], &[], &NewtonOptions::new(2)).unwrap();
    let t = tangent_modes(
        &me,
        &[],
        r.result.displacement_values(),
        &r.states,
        1.0,
        &[],
        &beam_modal(),
    )
    .unwrap();
    let s = solve_static(&me, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&me, &s).unwrap();
    let l = prestressed_modes(&m0, &pre, &beam_modal()).unwrap();
    let z = modal_analysis(&m0, &beam_modal()).unwrap();
    (t.frequencies, l.frequencies, z.frequencies)
}

#[test]
fn m1_m2_m6_clamped_beam_stretched() {
    let mut ck = Checks::new();
    for (eps, limit, id) in [(1e-3, 1e-2, "M1"), (1e-5, 1e-4, "M2")] {
        let (ft, fl, f0) = tension_frequencies(eps);
        println!("eps {eps:e}: f_T {ft:?}, f_L {fl:?}, f_0 {f0:?}");
        for j in 0..2 {
            ck.at_most(
                &format!("{id} eps {eps:e} mode {} |f_T/f_L - 1|", j + 1),
                (ft[j] / fl[j] - 1.0).abs(),
                limit,
            );
            if id == "M1" {
                ck.record(
                    &format!("M1b mode {} (f_T^2 - f_0^2)/(f_L^2 - f_0^2) - 1", j + 1),
                    (ft[j] * ft[j] - f0[j] * f0[j]) / (fl[j] * fl[j] - f0[j] * f0[j]) - 1.0,
                );
            }
        }
    }
    // M6: the tangent modes about the unloaded state are the modes of the model.
    let m0 = clamped_beam(0.0);
    let n = m0.dof_map().n_equations();
    let t = tangent_modes(
        &m0,
        &[],
        &vec![0.0; n],
        &initial_states(&m0).unwrap(),
        0.0,
        &[],
        &beam_modal(),
    )
    .unwrap();
    let z = modal_analysis(&m0, &beam_modal()).unwrap();
    for j in 0..2 {
        ck.at_most(
            &format!("M6 mode {} |lambda_T/lambda_0 - 1|", j + 1),
            (t.eigenvalues[j] / z.eigenvalues[j] - 1.0).abs(),
            1e-8,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- M3 to M5

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];

/// Beam F of F-396 (`si_rotating.rs`): Hex20, clamped at `x = 0`.
fn radial_beam() -> SolidModel {
    let base = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn rotating_modal() -> ModalOptions {
    ModalOptions::new(4)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn f396_frequencies(part: &RotatingPart, omega: f64) -> Vec<f64> {
    match part.modes(omega).unwrap() {
        SpeedModes::Modes(r) => r.frequencies,
        SpeedModes::LossOfStiffness(l) => panic!("unexpected loss of stiffness {l:?}"),
    }
}

/// The tangent-mode frequencies about the nonlinear state at `omega`, with or without the deformation-dependent load.
fn tangent_frequencies(omega: f64, with_follower: bool) -> Vec<f64> {
    let mut m = radial_beam();
    m.add_load(Load::centrifugal(AXIS, ORIGIN, omega)).unwrap();
    let cf = CentrifugalFollower::new(&m, None).unwrap();
    let list: Vec<&dyn Contribution> = if with_follower { vec![&cf] } else { Vec::new() };
    let ext = DriverExtensions {
        contributions: list.clone(),
        step_control: None,
    };
    let r = solve_with(&m, &[], &[], &NewtonOptions::new(2), ext).unwrap();
    tangent_modes(
        &m,
        &[],
        r.result.displacement_values(),
        &r.states,
        1.0,
        &list,
        &rotating_modal(),
    )
    .unwrap()
    .frequencies
}

#[test]
fn m3_m4_m5_rotating_radial_beam() {
    let mut ck = Checks::new();
    let f = radial_beam();
    let opts = RotatingOptions::new(4).modal(rotating_modal());
    let part = RotatingPart::new(&f, AXIS, ORIGIN, &opts).unwrap();
    let rest = f396_frequencies(&part, 0.0);
    let w0 = 2.0 * PI * rest[0];
    println!("omega_0 = {w0:.6} rad/s");
    for ratio in [0.2, 0.5, 1.0, 2.0] {
        let omega = ratio * w0;
        let ff = f396_frequencies(&part, omega);
        let ft = tangent_frequencies(omega, true);
        println!("Omega = {ratio} omega_0: f_T {ft:?}, f_F396 {ff:?}");
        for j in 0..4 {
            let d = (ft[j] / ff[j] - 1.0).abs();
            if ratio == 0.2 {
                ck.at_most(&format!("M3 0.2 omega_0 mode {} |f_T/f_F - 1|", j + 1), d, 1e-4);
            } else {
                ck.record(
                    &format!("M4 {ratio} omega_0 mode {} f_T/f_F - 1", j + 1),
                    ft[j] / ff[j] - 1.0,
                );
            }
        }
    }
    let omega = 0.2 * w0;
    let ff = f396_frequencies(&part, omega);
    let fno = tangent_frequencies(omega, false);
    ck.at_least(
        "M5 without the centrifugal follower, mode 1 |f_T/f_F - 1|",
        (fno[0] / ff[0] - 1.0).abs(),
        1e-3,
    );
    ck.finish();
}
