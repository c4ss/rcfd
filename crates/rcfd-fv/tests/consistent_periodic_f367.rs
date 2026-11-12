//! F-367 (ADR 0360): the consistent correctors' new combinations with periodic pressure faces. Per combination (non-orthogonal
//! correctors PN, the component diagonal PD, face jumps PJ, all three PX): the set-up checks, the periodic steady identity (S1:
//! from the default's steady state the option follows the default's own trajectory, in velocity and in the mean-removed pressure
//! against the default's own drift) and the no-harm transient (T); the refusals and acceptances through the solver (RP1 to RP3)
//! and as pure functions (RP4); the opt-in inert where it lifts nothing (NO). P0 (printed only): ADR 0223's E2 alone under this
//! item's pressure limit.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::GradientOptions;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::{
    ConsistentContext, ConsistentCorrectors, ConsistentRefusal, Divergence, OuterHook, OuterUpdate, PimpleSettings,
    PimpleSolver, PimpleState,
};
use rcfd_fv::pressure::pressure_bc;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, warp, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.01;
const U0: f64 = 0.1;
const V0: f64 = 0.02;
const LP: f64 = 0.3;
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

/// The pressure with its volume mean removed (copied from `consistent_correctors_f301.rs`, on a field).
fn mean_removed(geom: &FvGeometry, p: &[f64]) -> Vec<f64> {
    let vol = geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let m = p.iter().zip(vol).map(|(p, v)| p * v).sum::<f64>() / tv;
    p.iter().map(|p| p - m).collect()
}

// =================================================================================================================
// The set-ups of ADR 0360
// =================================================================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
enum Case {
    /// PN: the sheared channel, two non-orthogonal correctors (row 4).
    NonOrth,
    /// PD: the moving bottom, a symmetry plane on top, the component diagonal (row 7).
    Diagonal,
    /// PJ: PC with the jump on the lower half of the plane `x = 0.15` (row 11).
    Jump,
    /// PX: all three.
    All,
    /// P0: PC with ADR 0223 E2 alone (calibration).
    E2,
}

impl Case {
    fn sheared(self) -> bool {
        matches!(self, Case::NonOrth | Case::All)
    }
    fn moving_bottom(self) -> bool {
        matches!(self, Case::Diagonal | Case::All)
    }
    fn has_jump(self) -> bool {
        matches!(self, Case::Jump | Case::All)
    }
    /// D.
    fn settings(self) -> PimpleSettings {
        let mut s = fb_settings();
        if self.sheared() {
            s.non_orth_correctors = 2;
        }
        if self.moving_bottom() {
            s.component_diagonal = true;
        }
        s
    }
    /// C.
    fn settings_c(self) -> PimpleSettings {
        let mut s = self.settings();
        s.consistent = ConsistentCorrectors {
            enabled: true,
            periodic: true,
            body_force: false,
            non_orthogonal: self.sheared(),
            component_diagonal: self.moving_bottom(),
            face_jump: self.has_jump(),
        };
        s.consistent_periodic_extensions = self != Case::E2;
        s
    }
    fn geom(self) -> Arc<FvGeometry> {
        let g0 = box_2d_graded(
            20,
            12,
            LP,
            0.1,
            [Grading::Uniform, Grading::Symmetric(3.0)],
            true,
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
    /// The solver from rest; `hook_jump`: the jump is not set on the solver (the hook supplies it).
    fn solver(self, settings: PimpleSettings, hook_jump: bool) -> PimpleSolver {
        let geom = self.geom();
        let profile = |patch: &str| {
            let fc = geom.mesh.face_centres();
            let p = geom.mesh.patches().iter().find(|p| p.name == patch).expect("patch");
            Profile::PerFace(
                p.faces()
                    .map(|f| [U0, V0 * (2.0 * PI * fc[f][0] / LP).sin(), 0.0])
                    .collect(),
            )
        };
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        if self.moving_bottom() {
            u.set_bc(&geom, "ymin", Box::new(FixedValue(profile("ymin")))).unwrap();
            u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
        } else {
            u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
                .unwrap();
            u.set_bc(&geom, "ymax", Box::new(FixedValue(profile("ymax")))).unwrap();
        }
        let p = ScalarField::uniform("p", &geom, 0.0);
        let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
        if self.has_jump() && !hook_jump {
            flow.face_jump = Some(self.jump(&geom).0);
        }
        flow
    }
}

/// One step, the jump from the hook when given.
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

/// The default's steady state `S0`: 3000 steps from rest.
fn steady(case: Case) -> (PimpleSolver, PimpleState) {
    let mut a = case.solver(case.settings(), false);
    for _ in 0..3000 {
        a.step(DT);
        assert!(a.diverged.is_none(), "{case:?}: the S0 run stopped: {:?}", a.diverged);
    }
    let s0 = a.snapshot();
    (a, s0)
}

/// SE: the option acts in a transient (5 steps from rest, C against D).
fn effect(case: Case, id: &str) -> f64 {
    let mut d = case.solver(case.settings(), false);
    let mut c = case.solver(case.settings_c(), false);
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
        "SE{id}: after 5 steps from rest max|u_C - u_D| {:.3e} U0 (limit >= 1e-6)",
        e / U0
    );
    e
}

/// The numbers of one periodic identity.
struct Identity {
    rg: f64,
    seam: f64,
    da_u: f64,
    da_p: f64,
    ba_u: f64,
    ba_p: f64,
    ha_u: f64,
    ha_p: f64,
}

impl Identity {
    fn p_limit(&self) -> f64 {
        (1e-10 * self.rg).max(2.0 * self.da_p)
    }
}

/// The periodic steady identity: branch A (default), B (the option) and with `hook` B' (the jump from the hook) side by side
/// from `S0` for 50 steps; prints and returns the numbers (asserted by the caller).
fn identity(case: Case, id: &str, a: &mut PimpleSolver, s0: &PimpleState, hook: bool) -> Identity {
    let geom = a.geom.clone();
    let rg = range(&s0.p);
    // SPs: the largest pressure difference across a periodic pair at S0
    let pbc = pressure_bc(&a.geom, &a.u, &a.p);
    let own = a.geom.mesh.owner();
    let ni = a.geom.n_internal;
    let seam = pbc
        .coupled
        .iter()
        .enumerate()
        .filter_map(|(i, q)| q.map(|q| (s0.p[own[q]] - s0.p[own[ni + i]]).abs()))
        .fold(0.0f64, f64::max);
    let p_s0 = mean_removed(&geom, &s0.p);
    let mut b = case.solver(case.settings_c(), false);
    b.restore(s0).unwrap();
    let mut bh = hook.then(|| {
        let mut h = case.solver(case.settings_c(), true);
        h.restore(s0).unwrap();
        (h, case.jump(&geom).0)
    });
    let (mut da_u, mut da_p, mut db_p) = (0.0f64, 0.0f64, 0.0f64);
    let (mut ba_u, mut ba_p, mut ba_raw) = (0.0f64, 0.0f64, 0.0f64);
    let (mut ha_u, mut ha_p) = (0.0f64, 0.0f64);
    for _ in 0..50 {
        a.step(DT);
        b.step(DT);
        assert!(a.diverged.is_none(), "{id}: branch A stopped");
        assert!(b.diverged.is_none(), "{id}: branch B stopped: {:?}", b.diverged);
        let (pa, pb) = (mean_removed(&geom, &a.p.values), mean_removed(&geom, &b.p.values));
        da_u = da_u.max(max_du(&a.u.values, &s0.u));
        da_p = da_p.max(max_dp(&pa, &p_s0));
        db_p = db_p.max(max_dp(&pb, &p_s0));
        ba_u = ba_u.max(max_du(&b.u.values, &a.u.values));
        ba_p = ba_p.max(max_dp(&pb, &pa));
        ba_raw = ba_raw.max(max_dp(&b.p.values, &a.p.values));
        if let Some((h, j)) = bh.as_mut() {
            advance(h, DT, Some(j));
            assert!(h.diverged.is_none(), "{id}: branch B' stopped: {:?}", h.diverged);
            let ph = mean_removed(&geom, &h.p.values);
            ha_u = ha_u.max(max_du(&h.u.values, &a.u.values));
            ha_p = ha_p.max(max_dp(&ph, &pa));
        }
    }
    let r = Identity {
        rg,
        seam,
        da_u,
        da_p,
        ba_u,
        ba_p,
        ha_u,
        ha_p,
    };
    println!(
        "S0{id}: A max|u - u_S0| {:.2e} U0 (limit 2e-11); SF{id}: A's mean-removed drift dA_p {:.2e} of the range {rg:.4e} (limit \
         1e-8); SPs{id}: seam {:.3e} of the range (limit >= 1e-2); S1{id}: B against A max|du| {:.2e} U0 (limit 1e-10), max|dp'| \
         {:.2e} of the range (limit {:.2e} = max(1e-10, 2 dA_p); ratio to dA_p {:.3}); raw pressures {:.2e}; B's own drift dB_p \
         {:.2e}",
        da_u / U0,
        da_p / rg,
        seam / rg,
        ba_u / U0,
        ba_p / rg,
        r.p_limit() / rg,
        if da_p > 0.0 { ba_p / da_p } else { f64::NAN },
        ba_raw / rg,
        db_p / rg
    );
    if hook {
        println!(
            "S1{id}h: B' (the jump from the hook) against A max|du| {:.2e} U0 (limit 1e-10), max|dp'| {:.2e} of the range (limit \
             {:.2e})",
            ha_u / U0,
            ha_p / rg,
            r.p_limit() / rg
        );
    }
    r
}

/// The registered assertions of one identity, set-up checks first.
fn assert_identity(id: &str, r: &Identity, se: f64) {
    assert!(r.da_u <= 2e-11 * U0, "S0{id} {}", r.da_u);
    assert!(r.da_p <= 1e-8 * r.rg, "SF{id} {}", r.da_p);
    assert!(r.seam >= 1e-2 * r.rg, "SPs{id} {}", r.seam);
    assert!(se >= 1e-6 * U0, "SE{id} {se}");
    assert!(r.ba_u <= 1e-10 * U0, "S1{id} velocity {}", r.ba_u);
    assert!(r.ba_p <= r.p_limit(), "S1{id} pressure {}", r.ba_p);
    assert!(r.ha_u <= 1e-10 * U0, "S1{id}h velocity {}", r.ha_u);
    assert!(r.ha_p <= r.p_limit(), "S1{id}h pressure {}", r.ha_p);
}

/// T: the transient from rest to `t* = 0.1`, D and C, at `dt_1 = 0.002` and `dt_2 = 0.001`.
fn transient(case: Case, id: &str) {
    let arm = |settings: PimpleSettings, dt: f64, steps: usize| {
        let mut f = case.solver(settings, false);
        let mut cont = 0.0f64;
        for _ in 0..steps {
            cont = cont.max(advance(&mut f, dt, None));
            assert!(f.diverged.is_none(), "T{id}: an arm stopped: {:?}", f.diverged);
        }
        assert!(
            f.u.values.iter().all(|v| v.iter().all(|x| x.is_finite())),
            "T{id}: not finite"
        );
        (f.u.values.clone(), cont)
    };
    let (d1, cd1) = arm(case.settings(), DT, 50);
    let (c1, cc1) = arm(case.settings_c(), DT, 50);
    let (d2, cd2) = arm(case.settings(), 0.5 * DT, 100);
    let (c2, cc2) = arm(case.settings_c(), 0.5 * DT, 100);
    let (e1, e2) = (max_du(&c1, &d1), max_du(&c2, &d2));
    println!(
        "T{id}: e(dt_1) {:.3e} U0, e(dt_2) {:.3e} U0 (limit 0.05), ratio {:.3} (printed); the two default arms differ by {:.3e} \
         U0; continuity C {:.2e}, D {:.2e} (printed)",
        e1 / U0,
        e2 / U0,
        e1 / e2,
        max_du(&d1, &d2) / U0,
        cc1.max(cc2),
        cd1.max(cd2)
    );
    assert!(e2 <= 0.05 * U0, "T{id} e(dt_2) {e2}");
}

// =================================================================================================================
// S and T per combination
// =================================================================================================================

/// ADR 0360 SNs, SEPN, S0PN, SFPN, SPsPN, S1PN.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1pn_periodic_identity_with_non_orthogonal_correctors() {
    let case = Case::NonOrth;
    let geom = case.geom();
    let sf = geom.mesh.face_area_vectors();
    let ratio = (0..geom.n_internal)
        .map(|f| {
            let d = geom.delta[f];
            let k = vec3::sub(sf[f], vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d)));
            vec3::norm(k) / vec3::norm(sf[f])
        })
        .fold(f64::INFINITY, f64::min);
    println!("SNs: the smallest |k|/|S| of the internal faces {ratio:.4} (limit >= 0.4)");
    let se = effect(case, "PN");
    let (mut a, s0) = steady(case);
    let r = identity(case, "PN", &mut a, &s0, false);
    assert!(ratio >= 0.4, "SNs {ratio}");
    assert_identity("PN", &r, se);
}

/// ADR 0360 TPN.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn tpn_transient_with_non_orthogonal_correctors_and_periodic_faces() {
    transient(Case::NonOrth, "PN");
}

/// ADR 0360 SDs, SEPD, S0PD, SFPD, SPsPD, S1PD.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1pd_periodic_identity_with_the_component_diagonal() {
    let case = Case::Diagonal;
    let one = |diag: bool| {
        let mut s = case.settings();
        s.component_diagonal = diag;
        let mut f = case.solver(s, false);
        f.step(DT);
        assert!(f.diverged.is_none(), "SDs stopped");
        f.u.values.clone()
    };
    let sd = max_du(&one(true), &one(false));
    println!(
        "SDs: one default step with and without the component diagonal: max|du| {:.3e} U0 (limit >= 1e-6)",
        sd / U0
    );
    let se = effect(case, "PD");
    let (mut a, s0) = steady(case);
    let r = identity(case, "PD", &mut a, &s0, false);
    assert!(sd >= 1e-6 * U0, "SDs {sd}");
    assert_identity("PD", &r, se);
}

/// ADR 0360 TPD.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn tpd_transient_with_the_component_diagonal_and_periodic_faces() {
    transient(Case::Diagonal, "PD");
}

/// ADR 0360 SJs, SEPJ, S0PJ, SFPJ, SPsPJ, S1PJ, S1PJh.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1pj_s1pjh_periodic_identity_with_a_face_jump() {
    let case = Case::Jump;
    let se = effect(case, "PJ");
    let (mut a, s0) = steady(case);
    let (j, faces) = case.jump(&a.geom);
    let n = faces.len() as f64;
    let total = faces.iter().map(|&f| j.total(f, &a.phi).abs()).sum::<f64>() / n;
    let resist = faces.iter().map(|&f| (j.resist[f] * a.phi[f]).abs()).sum::<f64>() / n;
    println!(
        "SJs: the mean total rise of the jump faces {:.4} R0 (limit >= 0.3), its resistance part {:.4} R0 (limit >= 0.01)",
        total / R0,
        resist / R0
    );
    let r = identity(case, "PJ", &mut a, &s0, true);
    assert!(total >= 0.3 * R0, "SJs total {total}");
    assert!(resist >= 0.01 * R0, "SJs resistance {resist}");
    assert_identity("PJ", &r, se);
}

/// ADR 0360 TPJ.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn tpj_transient_with_a_face_jump_and_periodic_faces() {
    transient(Case::Jump, "PJ");
}

/// ADR 0360 SEPX, S0PX, SFPX, SPsPX, S1PX.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1px_periodic_identity_with_all_three() {
    let case = Case::All;
    let se = effect(case, "PX");
    let (mut a, s0) = steady(case);
    let r = identity(case, "PX", &mut a, &s0, false);
    assert_identity("PX", &r, se);
}

/// ADR 0360 P0 (printed only): ADR 0223's E2 alone on PC under this item's limits.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p0_e2_alone_under_the_drift_limit_printed() {
    let case = Case::E2;
    let (mut a, s0) = steady(case);
    let r = identity(case, "P0", &mut a, &s0, false);
    println!(
        "P0: velocity {} the limit, pressure {} the limit (printed, no assertion)",
        if r.ba_u <= 1e-10 * U0 { "within" } else { "above" },
        if r.ba_p <= r.p_limit() { "within" } else { "above" }
    );
}

// =================================================================================================================
// RP: refusals and acceptances
// =================================================================================================================

/// ADR 0214's 2-D box solver (copied from `consistent_value_f322.rs`).
fn box_solver(periodic: bool, settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d(8, 6, 1.0, 0.5, periodic, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let walls: &[&str] = if periodic {
        &["ymin", "ymax"]
    } else {
        &["xmin", "xmax", "ymin", "ymax"]
    };
    for name in walls {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

#[derive(Clone, Copy, PartialEq)]
enum Via {
    Step,
    HookJump,
    Ale,
}

/// One step; the divergence record if stopped, with the state checked unchanged.
fn one_step(label: &str, mut flow: PimpleSolver, via: Via) -> Option<Divergence> {
    let before = (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi));
    let geom = flow.geom.clone();
    match via {
        Via::Step => {
            flow.step(0.01);
        }
        Via::HookJump => {
            let n = geom.mesh.n_faces();
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                face_jump: Some(FaceJump::zeros(n)),
                ..Default::default()
            };
            flow.step_with(0.01, Some(&mut hook as &mut OuterHook));
        }
        Via::Ale => {
            let n = geom.mesh.n_faces();
            flow.step_ale(0.01, geom.clone(), vec![0.0; n]).expect("step_ale");
        }
    }
    let d = flow.diverged.clone()?;
    assert_eq!(
        (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi)),
        before,
        "{label}: the state changed"
    );
    Some(d)
}

fn body(mut f: PimpleSolver) -> PimpleSolver {
    f.body_force = Some(BodyForce::uniform(&f.geom.clone(), [0.0, -9.81, 0.0]));
    f
}

fn jump(mut f: PimpleSolver) -> PimpleSolver {
    f.face_jump = Some(FaceJump::zeros(f.geom.mesh.n_faces()));
    f
}

fn settings_with(c: ConsistentCorrectors, ext: bool, f: &dyn Fn(&mut PimpleSettings)) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.01);
    s.consistent = c;
    s.consistent_periodic_extensions = ext;
    f(&mut s);
    s
}

/// The checked refusal of one case: stopped at step 1 with the row's stage and its located message.
fn assert_refused(label: &str, refusal: ConsistentRefusal, d: Option<Divergence>) {
    let d = d.unwrap_or_else(|| panic!("{label}: not refused"));
    let msg = d.message(None);
    println!("{label}: {msg}");
    assert_eq!(d.step, 1, "{label} step");
    assert_eq!(d.quantity, "consistent correctors", "{label} quantity");
    assert_eq!(d.stage, refusal.stage(), "{label} stage");
    for part in [
        "ADR 0214".to_string(),
        format!("are not defined with the {}:", refusal.stage()),
        format!("ConsistentCorrectors::{}", refusal.flag()),
        refusal.setting().to_string(),
        refusal.reason().to_string(),
        "ADR 0258".to_string(),
    ] {
        assert!(msg.contains(&part), "{label}: '{part}' missing in '{msg}'");
    }
}

/// The three rows of ADR 0360 on the periodic box: (row, solver, via).
fn periodic_rows(ext: bool) -> Vec<(ConsistentRefusal, PimpleSolver, Via)> {
    use ConsistentRefusal as R;
    let all = ConsistentCorrectors::all();
    let none = |_: &mut PimpleSettings| {};
    vec![
        (
            R::NonOrthogonalPeriodic,
            box_solver(true, settings_with(all, ext, &|s| s.non_orth_correctors = 1)),
            Via::Step,
        ),
        (
            R::ComponentDiagonalPeriodic,
            box_solver(true, settings_with(all, ext, &|s| s.component_diagonal = true)),
            Via::Step,
        ),
        (
            R::FaceJumpPeriodic,
            jump(box_solver(true, settings_with(all, ext, &none))),
            Via::Step,
        ),
        (
            R::FaceJumpPeriodic,
            box_solver(true, settings_with(all, ext, &none)),
            Via::HookJump,
        ),
    ]
}

/// ADR 0360 RP1: without the opt-in the three rows are refused as before, with their located message.
#[test]
fn rp1_without_the_opt_in_the_periodic_rows_are_refused() {
    for (refusal, flow, via) in periodic_rows(false) {
        let label = format!("RP1 {refusal:?}");
        assert_refused(&label, refusal, one_step(&label, flow, via));
        if ConsistentRefusal::PERIODIC_EXTENSIONS.contains(&refusal) {
            assert!(
                refusal.reason().contains("consistent_periodic_extensions") && refusal.reason().contains("ADR 0360"),
                "{label}: {}",
                refusal.reason()
            );
        }
    }
}

/// ADR 0360 RP2: with the opt-in the rows of `PERIODIC_EXTENSIONS` are accepted (alone and together with a body force); a row not
/// in it stays refused.
#[test]
fn rp2_with_the_opt_in_the_periodic_rows_are_accepted() {
    for (refusal, flow, via) in periodic_rows(true) {
        let label = format!("RP2 {refusal:?}");
        let d = one_step(&label, flow, via);
        if ConsistentRefusal::PERIODIC_EXTENSIONS.contains(&refusal) {
            assert!(d.is_none(), "{label}: stopped: {:?}", d.map(|d| d.message(None)));
        } else {
            assert_refused(&label, refusal, d);
        }
    }
    let s = settings_with(ConsistentCorrectors::all(), true, &|s| {
        s.non_orth_correctors = 1;
        s.component_diagonal = true;
    });
    let d = one_step(
        "RP2 all three with a body force",
        jump(body(box_solver(true, s))),
        Via::Step,
    );
    let all_in = [
        ConsistentRefusal::NonOrthogonalPeriodic,
        ConsistentRefusal::ComponentDiagonalPeriodic,
        ConsistentRefusal::FaceJumpPeriodic,
    ]
    .iter()
    .all(|r| ConsistentRefusal::PERIODIC_EXTENSIONS.contains(r));
    assert_eq!(d.is_none(), all_in, "RP2 all three: {:?}", d.map(|d| d.message(None)));
}

/// ADR 0360 RP3: what stays refused with the opt-in.
#[test]
fn rp3_with_the_opt_in_the_other_rows_stay_refused() {
    use ConsistentRefusal as R;
    let all = ConsistentCorrectors::all();
    let no_periodic = ConsistentCorrectors { periodic: false, ..all };
    let none = |_: &mut PimpleSettings| {};
    let cases: Vec<(R, PimpleSolver, Via)> = vec![
        (
            R::NonOrthogonalMovingMesh,
            box_solver(true, settings_with(all, true, &|s| s.non_orth_correctors = 1)),
            Via::Ale,
        ),
        (
            R::ComponentDiagonalMovingMesh,
            box_solver(true, settings_with(all, true, &|s| s.component_diagonal = true)),
            Via::Ale,
        ),
        (
            R::FaceJumpMovingMesh,
            jump(box_solver(true, settings_with(all, true, &none))),
            Via::Ale,
        ),
        (
            R::NonOrthogonalGradientOptions,
            box_solver(
                true,
                settings_with(all, true, &|s| {
                    s.non_orth_correctors = 1;
                    s.gradient_pressure = Some(GradientOptions::default());
                }),
            ),
            Via::Step,
        ),
        (
            R::PeriodicFaces,
            box_solver(true, settings_with(no_periodic, true, &|s| s.non_orth_correctors = 1)),
            Via::Step,
        ),
    ];
    for (refusal, flow, via) in cases {
        let label = format!("RP3 {refusal:?}");
        assert_refused(&label, refusal, one_step(&label, flow, via));
    }
}

/// ADR 0360 RP4: `conflicts_extended` and `validate_extended` as pure functions over every context.
#[test]
fn rp4_the_extended_validation_removes_exactly_the_periodic_extensions() {
    let flag_sets = [
        ConsistentCorrectors {
            enabled: true,
            ..Default::default()
        },
        ConsistentCorrectors::all(),
        ConsistentCorrectors {
            periodic: false,
            ..ConsistentCorrectors::all()
        },
    ];
    let mut n = 0;
    for bits in 0u32..(1 << 8) {
        let b = |i: u32| bits & (1 << i) != 0;
        let ctx = ConsistentContext {
            fractional: b(0),
            gradient_pressure: b(1),
            component_diagonal: b(2),
            body_force: b(3),
            face_jump: b(4),
            periodic_faces: b(5),
            moving_mesh: b(6),
            non_orth_correctors: if b(7) { 1 } else { 0 },
        };
        for flags in &flag_sets {
            let base = flags.conflicts(&ctx);
            assert_eq!(flags.conflicts_extended(&ctx, false), base, "{ctx:?}");
            let want: Vec<ConsistentRefusal> = base
                .iter()
                .copied()
                .filter(|r| !ConsistentRefusal::PERIODIC_EXTENSIONS.contains(r))
                .collect();
            let got = flags.conflicts_extended(&ctx, true);
            assert_eq!(got, want, "{ctx:?}");
            assert_eq!(
                flags.validate_extended(&ctx, true),
                want.first().map_or(Ok(()), |r| Err(*r))
            );
            assert_eq!(flags.validate_extended(&ctx, false), flags.validate(&ctx));
            n += 1;
        }
    }
    assert_eq!(n, 768);
}

// =================================================================================================================
// NO: the opt-in inert where it lifts nothing
// =================================================================================================================

/// ADR 0258's DN duct (copied set-up, sheared, X1 through the value).
fn dn_duct(settings: PimpleSettings) -> PimpleSolver {
    let g0 = box_2d_graded(
        20,
        12,
        LP,
        0.1,
        [Grading::Uniform, Grading::Symmetric(3.0)],
        false,
        false,
    )
    .unwrap();
    let mesh = warp(&g0, |p| [p[0] + SHEAR * p[1], p[1], p[2]]).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([U0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, p, settings)
}

/// ADR 0360 NO.
#[test]
fn no_the_opt_in_is_inert_where_it_lifts_nothing() {
    let run = |mut f: PimpleSolver, steps: usize| {
        for _ in 0..steps {
            f.step(DT);
            assert!(f.diverged.is_none(), "NO stopped: {:?}", f.diverged);
        }
        (ubits(&f.u.values), bits(&f.p.values), bits(&f.phi))
    };
    // (i) without `enabled`
    let inert = settings_with(
        ConsistentCorrectors {
            enabled: false,
            ..ConsistentCorrectors::all()
        },
        true,
        &|_| {},
    );
    assert_eq!(
        run(body(box_solver(true, inert)), 3),
        run(body(box_solver(true, PimpleSettings::piso(0.01))), 3),
        "NO (i)"
    );
    // (ii) runs accepted before: PC with E2 (the older fields), the DN duct with X1 (the value)
    let e2 = |ext: bool| {
        let mut s = fb_settings();
        s.consistent_correctors = true;
        s.consistent_periodic = true;
        s.consistent_periodic_extensions = ext;
        s
    };
    assert_eq!(
        run(Case::E2.solver(e2(true), false), 20),
        run(Case::E2.solver(e2(false), false), 20),
        "NO (ii) E2"
    );
    let x1 = |ext: bool| {
        let mut s = fb_settings();
        s.non_orth_correctors = 2;
        s.consistent = ConsistentCorrectors {
            enabled: true,
            non_orthogonal: true,
            ..Default::default()
        };
        s.consistent_periodic_extensions = ext;
        s
    };
    assert_eq!(run(dn_duct(x1(true)), 20), run(dn_duct(x1(false)), 20), "NO (ii) X1");
}
