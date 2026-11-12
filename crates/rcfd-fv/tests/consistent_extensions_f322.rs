//! F-322 (ADR 0258), the extended combinations of the consistent correctors: non-orthogonal correctors (X1), the component
//! diagonal (X2), face jumps (X3) and all together with a body force. Per extension: the set-up checks, the steady identity
//! (S1: from the default's steady state the option follows the default's own trajectory, ADR 0223 amendment 1's form) and the
//! no-harm transient (T: the option against the default from rest at two step sizes); TI: the existing set-ups of ADR 0223 are
//! bit-identical with the flags given through the value and with the new flags added; LN: the pressure-lag estimate with X1.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::{
    ConsistentCorrectors, LagAction, LagCheck, OuterHook, OuterUpdate, PimpleSettings, PimpleSolver, PimpleState,
};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d_graded, warp, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.01;
const U0: f64 = 0.1;
const DT: f64 = 0.002;
const SHEAR: f64 = 0.5;
const R0: f64 = 0.2;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

/// ADR 0211's `fb_settings` (copied).
fn fb_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(20000);
    s.amg_pressure = true;
    s
}

fn ubits(v: &[Vec3]) -> Vec<u64> {
    v.iter().flat_map(|x| x.iter().map(|y| y.to_bits())).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_du(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .fold(0.0f64, |m, (x, y)| m.max(vec3::norm(vec3::sub(*x, *y))))
}

fn max_dp(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()))
}

fn range(p: &[f64]) -> f64 {
    let hi = p.iter().fold(f64::NEG_INFINITY, |m, x| m.max(*x));
    let lo = p.iter().fold(f64::INFINITY, |m, x| m.min(*x));
    hi - lo
}

/// The four set-ups of ADR 0258.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Duct {
    /// DN: the sheared grid, two non-orthogonal correctors.
    NonOrth,
    /// DD: a symmetry plane at `ymax`, the component diagonal.
    Diagonal,
    /// DJ: a face jump on the lower half of the plane `x = 0.15`.
    Jump,
    /// DX: all of them and a body force.
    All,
}

impl Duct {
    fn sheared(self) -> bool {
        matches!(self, Duct::NonOrth | Duct::All)
    }
    fn symmetry(self) -> bool {
        matches!(self, Duct::Diagonal | Duct::All)
    }
    fn has_jump(self) -> bool {
        matches!(self, Duct::Jump | Duct::All)
    }
    /// The default settings of the set-up (D).
    fn settings(self) -> PimpleSettings {
        let mut s = fb_settings();
        if self.sheared() {
            s.non_orth_correctors = 2;
        }
        if self.symmetry() {
            s.component_diagonal = true;
        }
        s
    }
    /// The option of the set-up (C).
    fn option(self) -> ConsistentCorrectors {
        ConsistentCorrectors {
            enabled: true,
            periodic: false,
            body_force: self == Duct::All,
            non_orthogonal: self.sheared(),
            component_diagonal: self.symmetry(),
            face_jump: self.has_jump(),
        }
    }
    fn settings_c(self) -> PimpleSettings {
        let mut s = self.settings();
        s.consistent = self.option();
        s
    }
    fn geom(self) -> Arc<FvGeometry> {
        let g0 = box_2d_graded(
            20,
            12,
            0.3,
            0.1,
            [Grading::Uniform, Grading::Symmetric(3.0)],
            false,
            false,
        )
        .unwrap();
        let mesh = if self.sheared() {
            warp(&g0, |p| [p[0] + SHEAR * p[1], p[1], p[2]]).unwrap()
        } else {
            g0
        };
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }
    /// The jump of DJ and DX and its faces.
    fn jump(self, geom: &FvGeometry) -> (FaceJump, Vec<usize>) {
        let mesh = &geom.mesh;
        let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
        let shear = if self.sheared() { SHEAR } else { 0.0 };
        let mut j = FaceJump::zeros(mesh.n_faces());
        let mut faces = Vec::new();
        for f in 0..geom.n_internal {
            let x = fc[f][0] - shear * fc[f][1];
            if (x - 0.15).abs() < 1e-9 && fc[f][1] < 0.05 {
                j.rise[f] = R0 * sf[f][0].signum();
                j.resist[f] = 0.2 * R0 / (U0 * vec3::norm(sf[f]));
                faces.push(f);
            }
        }
        assert_eq!(faces.len(), 6, "the jump faces");
        (j, faces)
    }
    /// The solver of the set-up from rest; `hook_jump`: the jump is not set on the solver (the hook supplies it).
    fn solver(self, settings: PimpleSettings, hook_jump: bool) -> PimpleSolver {
        let geom = self.geom();
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([U0, 0.0, 0.0])))
            .unwrap();
        u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
            .unwrap();
        if self.symmetry() {
            u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
        } else {
            u.set_bc(&geom, "ymax", Box::new(FixedValue::vector(vec3::ZERO)))
                .unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
        if self.has_jump() && !hook_jump {
            flow.face_jump = Some(self.jump(&geom).0);
        }
        if self == Duct::All {
            flow.body_force = Some(BodyForce::uniform(&geom, [0.5, -1.0, 0.0]));
        }
        flow
    }
}

/// One step of `flow`, the jump from the hook when given.
fn advance(flow: &mut PimpleSolver, dt: f64, hook_jump: Option<&FaceJump>) -> f64 {
    let info = match hook_jump {
        Some(j) => {
            let j = j.clone();
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                face_jump: Some(j.clone()),
                ..Default::default()
            };
            flow.step_with(dt, Some(&mut hook as &mut OuterHook))
        }
        None => flow.step(dt),
    };
    info.continuity
}

/// The default's steady state `S0` of a set-up: 3000 steps from rest.
fn steady(duct: Duct) -> (PimpleSolver, PimpleState) {
    let mut a = duct.solver(duct.settings(), false);
    for _ in 0..3000 {
        a.step(DT);
        assert!(a.diverged.is_none(), "{duct:?}: the S0 run stopped: {:?}", a.diverged);
    }
    let s0 = a.snapshot();
    (a, s0)
}

/// The steady identity of a set-up: branch A (default) and branch B (the option; with `hook`: a further branch B' whose jump
/// comes from the hook) side by side from `S0` for 50 steps. Asserts S0 and S1 and prints them.
fn identity(duct: Duct, id: &str, a: &mut PimpleSolver, s0: &PimpleState, hook: bool) {
    let rg = range(&s0.p);
    let mut b = duct.solver(duct.settings_c(), false);
    b.restore(s0).unwrap();
    let mut bh = hook.then(|| {
        let mut h = duct.solver(duct.settings_c(), true);
        h.restore(s0).unwrap();
        (h, duct.jump(&a.geom).0)
    });
    let (mut dev_a, mut dev_b) = ((0.0f64, 0.0f64), (0.0f64, 0.0f64));
    let (mut ba, mut ha) = ((0.0f64, 0.0f64), (0.0f64, 0.0f64));
    for _ in 0..50 {
        a.step(DT);
        b.step(DT);
        assert!(a.diverged.is_none(), "{id}: branch A stopped");
        assert!(b.diverged.is_none(), "{id}: branch B stopped: {:?}", b.diverged);
        dev_a.0 = dev_a.0.max(max_du(&a.u.values, &s0.u));
        dev_a.1 = dev_a.1.max(max_dp(&a.p.values, &s0.p));
        dev_b.0 = dev_b.0.max(max_du(&b.u.values, &s0.u));
        dev_b.1 = dev_b.1.max(max_dp(&b.p.values, &s0.p));
        ba.0 = ba.0.max(max_du(&b.u.values, &a.u.values));
        ba.1 = ba.1.max(max_dp(&b.p.values, &a.p.values));
        if let Some((h, j)) = bh.as_mut() {
            advance(h, DT, Some(j));
            assert!(h.diverged.is_none(), "{id}: branch B' stopped: {:?}", h.diverged);
            ha.0 = ha.0.max(max_du(&h.u.values, &a.u.values));
            ha.1 = ha.1.max(max_dp(&h.p.values, &a.p.values));
        }
    }
    println!(
        "S0{id}: branch A max|u - u_S0| {:.2e} U0 (limit 2e-11), max|p - p_S0| {:.2e} of the range {rg:.4e}; S1{id}: B against A \
         max|du| {:.2e} U0, max|dp| {:.2e} of the range (limits 1e-10); B against S0 {:.2e} U0, {:.2e}",
        dev_a.0 / U0,
        dev_a.1 / rg,
        ba.0 / U0,
        ba.1 / rg,
        dev_b.0 / U0,
        dev_b.1 / rg
    );
    if hook {
        println!(
            "S1{id}h: B' (the jump from the hook) against A max|du| {:.2e} U0, max|dp| {:.2e} of the range (limits 1e-10)",
            ha.0 / U0,
            ha.1 / rg
        );
    }
    assert!(dev_a.0 <= 2e-11 * U0, "S0{id} {}", dev_a.0);
    assert!(ba.0 <= 1e-10 * U0, "S1{id} velocity {}", ba.0);
    assert!(ba.1 <= 1e-10 * rg, "S1{id} pressure {}", ba.1);
    assert!(ha.0 <= 1e-10 * U0, "S1{id}h velocity {}", ha.0);
    assert!(ha.1 <= 1e-10 * rg, "S1{id}h pressure {}", ha.1);
}

/// SE: the option acts in a transient (5 steps from rest, C against D).
fn effect(duct: Duct, id: &str) {
    let mut d = duct.solver(duct.settings(), false);
    let mut c = duct.solver(duct.settings_c(), false);
    for _ in 0..5 {
        d.step(DT);
        c.step(DT);
        assert!(
            d.diverged.is_none() && c.diverged.is_none(),
            "SE {id} stopped: {:?}",
            c.diverged
        );
    }
    let e = max_du(&c.u.values, &d.u.values);
    println!(
        "SE {id}: after 5 steps from rest max|u_C - u_D| {:.3e} U0 (limit >= 1e-6)",
        e / U0
    );
    assert!(e >= 1e-6 * U0, "SE {id} {e}");
}

/// T: the transient of a set-up from rest to `t* = 0.1`, D and C, at `dt_1 = 0.002` and `dt_2 = 0.001`.
fn transient(duct: Duct, id: &str) {
    let arm = |settings: PimpleSettings, dt: f64, steps: usize| {
        let mut f = duct.solver(settings, false);
        let mut cont = 0.0f64;
        for _ in 0..steps {
            cont = cont.max(advance(&mut f, dt, None));
            assert!(f.diverged.is_none(), "T{id}: an arm stopped: {:?}", f.diverged);
        }
        assert!(f.u.values.iter().all(|v| v.iter().all(|x| x.is_finite())));
        (f.u.values.clone(), cont)
    };
    let (d1, cd1) = arm(duct.settings(), DT, 50);
    let (c1, cc1) = arm(duct.settings_c(), DT, 50);
    let (d2, cd2) = arm(duct.settings(), 0.5 * DT, 100);
    let (c2, cc2) = arm(duct.settings_c(), 0.5 * DT, 100);
    let (e1, e2) = (max_du(&c1, &d1), max_du(&c2, &d2));
    let (cont_c, cont_d) = (cc1.max(cc2), cd1.max(cd2));
    println!(
        "T{id}: e(dt_1) {:.3e} U0, e(dt_2) {:.3e} U0 (limit 0.05), ratio {:.3} (limit >= 1.3); the two default arms differ by \
         {:.3e} U0; continuity C {cont_c:.2e} (limit max(1e-10, 2 x D's)), D {cont_d:.2e}",
        e1 / U0,
        e2 / U0,
        e1 / e2,
        max_du(&d1, &d2) / U0
    );
    assert!(cont_c <= 1e-10f64.max(2.0 * cont_d), "T{id} continuity {cont_c}");
    assert!(e2 <= 0.05 * U0, "T{id} e(dt_2) {e2}");
    assert!(e1 / e2 >= 1.3, "T{id} ratio {}", e1 / e2);
}

// =================================================================================================================
// X1: non-orthogonal correctors
// =================================================================================================================

/// ADR 0258 SNs, SE, S0N, S1N and LN.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1n_ln_steady_state_with_non_orthogonal_correctors() {
    let duct = Duct::NonOrth;
    // SNs: the non-orthogonality of the internal faces
    let geom = duct.geom();
    let sf = geom.mesh.face_area_vectors();
    let ratio = (0..geom.n_internal)
        .map(|f| {
            let d = geom.delta[f];
            let k = vec3::sub(sf[f], vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d)));
            vec3::norm(k) / vec3::norm(sf[f])
        })
        .fold(f64::INFINITY, f64::min);
    println!("SNs: the smallest |k|/|S| of the internal faces {ratio:.4} (limit >= 0.4)");
    assert!(ratio >= 0.4, "SNs {ratio}");
    effect(duct, "N");
    let (mut a, s0) = steady(duct);
    // LN: the estimate with the option and X1, and the same step without it
    let lag_step = |check: bool| {
        let mut s = duct.settings_c();
        if check {
            s.pressure_lag_check = Some(LagCheck {
                every: 1,
                start: 1,
                iterations: 20,
                action: LagAction::Warn,
            });
        }
        let mut f = duct.solver(s, false);
        f.restore(&s0).unwrap();
        f.step(DT);
        assert!(f.diverged.is_none(), "LN stopped: {:?}", f.diverged);
        let est: Vec<f64> = f.lag_estimates.iter().map(|w| w.rho).collect();
        (ubits(&f.u.values), bits(&f.p.values), bits(&f.phi), est)
    };
    let (with, without) = (lag_step(true), lag_step(false));
    println!("LN: the estimate(s) of the step with the option and X1: {:?}", with.3);
    assert_eq!(with.3.len(), 1, "LN: one estimate");
    assert!(with.3[0].is_finite(), "LN: finite");
    assert!(without.3.is_empty());
    assert_eq!(
        (&with.0, &with.1, &with.2),
        (&without.0, &without.1, &without.2),
        "LN: the estimate changed the step"
    );
    identity(duct, "N", &mut a, &s0, false);
}

/// ADR 0258 TN.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn tn_transient_with_non_orthogonal_correctors() {
    transient(Duct::NonOrth, "N");
}

// =================================================================================================================
// X2: the component diagonal
// =================================================================================================================

/// ADR 0258 SDs, SE, S0D, S1D.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1d_steady_state_with_the_component_diagonal() {
    let duct = Duct::Diagonal;
    // SDs: the component diagonal acts on this set-up
    let one = |diag: bool| {
        let mut s = duct.settings();
        s.component_diagonal = diag;
        let mut f = duct.solver(s, false);
        f.step(DT);
        assert!(f.diverged.is_none(), "SDs stopped");
        f.u.values.clone()
    };
    let e = max_du(&one(true), &one(false));
    println!(
        "SDs: one default step with and without the component diagonal: max|du| {:.3e} U0 (limit >= 1e-6)",
        e / U0
    );
    assert!(e >= 1e-6 * U0, "SDs {e}");
    effect(duct, "D");
    let (mut a, s0) = steady(duct);
    identity(duct, "D", &mut a, &s0, false);
}

/// ADR 0258 TD.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn td_transient_with_the_component_diagonal() {
    transient(Duct::Diagonal, "D");
}

// =================================================================================================================
// X3: face jumps
// =================================================================================================================

/// ADR 0258 SJs, SE, S0J, S1J, S1Jh.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1j_s1jh_steady_state_with_a_face_jump() {
    let duct = Duct::Jump;
    effect(duct, "J");
    let (mut a, s0) = steady(duct);
    // SJs: the jump and its resistance part at S0
    let (j, faces) = duct.jump(&a.geom);
    let n = faces.len() as f64;
    let total = faces.iter().map(|&f| j.total(f, &a.phi).abs()).sum::<f64>() / n;
    let resist = faces.iter().map(|&f| (j.resist[f] * a.phi[f]).abs()).sum::<f64>() / n;
    println!(
        "SJs: the mean total rise of the jump faces {:.4} R0 (limit >= 0.3), its resistance part {:.4} R0 (limit >= 0.05)",
        total / R0,
        resist / R0
    );
    assert!(total >= 0.3 * R0, "SJs total {total}");
    assert!(resist >= 0.05 * R0, "SJs resistance {resist}");
    identity(duct, "J", &mut a, &s0, true);
}

/// ADR 0258 TJ.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn tj_transient_with_a_face_jump() {
    transient(Duct::Jump, "J");
}

// =================================================================================================================
// All together
// =================================================================================================================

/// ADR 0258 SE, S0X, S1X.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1x_steady_state_with_every_extension_and_a_body_force() {
    let duct = Duct::All;
    effect(duct, "X");
    let (mut a, s0) = steady(duct);
    identity(duct, "X", &mut a, &s0, false);
}

// =================================================================================================================
// TI: the existing set-ups of ADR 0223 (copied), bit for bit
// =================================================================================================================

/// ADR 0223's SBC with amplitude `a` (copied from `consistent_correctors_f301.rs`).
fn sbc_solver_a(settings: PimpleSettings, a: f64) -> PimpleSolver {
    let mesh = box_2d_graded(
        20,
        12,
        0.3,
        0.1,
        [Grading::Symmetric(3.0), Grading::Symmetric(3.0)],
        false,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
    let accel = |x: Vec3| [a * (PI * x[1] / 0.1).sin(), -a, 0.0];
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    flow.body_force = Some(BodyForce {
        cell: cc.iter().map(|&c| accel(c)).collect(),
        face: (0..mesh.n_faces())
            .map(|f| {
                if geom.empty[f] {
                    0.0
                } else {
                    vec3::dot(accel(fc[f]), sf[f])
                }
            })
            .collect(),
    });
    flow
}

/// ADR 0223's SPC, one period (copied from `consistent_correctors_f301.rs`).
fn spc_solver(settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d_graded(
        20,
        12,
        0.3,
        0.1,
        [Grading::Uniform, Grading::Symmetric(3.0)],
        true,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let fc = geom.mesh.face_centres();
    let top = geom
        .mesh
        .patches()
        .iter()
        .find(|p| p.name == "ymax")
        .expect("ymax patch");
    let profile: Vec<Vec3> = top
        .faces()
        .map(|f| [0.1, 0.02 * (2.0 * PI * fc[f][0] / 0.3).sin(), 0.0])
        .collect();
    u.set_bc(&geom, "ymax", Box::new(FixedValue(Profile::PerFace(profile))))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

/// ADR 0258 TI: the flags through the older fields, through the value alone, and with the new flags added.
#[test]
fn ti_existing_set_ups_are_bit_identical_through_the_value() {
    let run = |mut f: PimpleSolver| {
        for _ in 0..20 {
            f.step(DT);
            assert!(f.diverged.is_none(), "TI stopped: {:?}", f.diverged);
        }
        (ubits(&f.u.values), bits(&f.p.values), bits(&f.phi))
    };
    for periodic in [false, true] {
        let make = |s: PimpleSettings| {
            if periodic {
                spc_solver(s)
            } else {
                sbc_solver_a(s, 10.0)
            }
        };
        // (i) the older fields
        let mut old = fb_settings();
        old.consistent_correctors = true;
        old.consistent_periodic = periodic;
        old.consistent_body_force = !periodic;
        // (ii) the value alone
        let value = ConsistentCorrectors {
            enabled: true,
            periodic,
            body_force: !periodic,
            ..Default::default()
        };
        let mut by_value = fb_settings();
        by_value.consistent = value;
        assert!(!by_value.consistent_correctors);
        // (iii) the value with the new flags
        let mut with_new = fb_settings();
        with_new.consistent = ConsistentCorrectors {
            non_orthogonal: true,
            component_diagonal: true,
            face_jump: true,
            ..value
        };
        let reference = run(make(old));
        assert_ne!(
            reference,
            run(make(fb_settings())),
            "TI: the option does nothing on this set-up"
        );
        assert_eq!(run(make(by_value)), reference, "TI (ii), periodic {periodic}");
        assert_eq!(run(make(with_new)), reference, "TI (iii), periodic {periodic}");
    }
}
