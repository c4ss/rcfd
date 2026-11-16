//! F-724 (ADR 0659): the threshold scale of the coupled interface solves. Checks E1, E1u (ADR 0638's uniform flows: the
//! default `sliding_step` against F-710's step), E2 (ADR 0640's uniform flows: `periodic_sliding_step_coupled` against
//! `periodic_sliding_step`), E3, X3 (an axial uniform flow on the 3-D full annulus at coincidences: the default
//! `sliding_step` against the uncoupled step), K1, K1r, K2 (ADR 0641's non-symmetric flows: the coupled step against F-721's)
//! and the controls NC1 to NC3. The limits are those registered in ADR 0659 before this code ran; each test prints every
//! measured number and then asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sliding::{sliding_step, sliding_step_with, PartCoupling, SlidingStep};
use rcfd_fv::sliding_periodic::{
    periodic_sliding_step, periodic_sliding_step_coupled, periodic_sliding_step_with, PeriodicSlidingStep,
};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d, annulus_3d_sector, annulus_sector, cylinder_ogrid, rectilinear_2d, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::PeriodicSlidingInterface;
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::{FRAC_PI_3, PI, TAU};
use std::sync::Arc;
use std::time::Instant;

const Z: Vec3 = [0.0, 0.0, 1.0];
const L: f64 = 0.1;
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

// -------------------------------------------------------------------------------------------------------------------
// pairs and settings (as registered in ADR 0638, 0640, 0641)
// -------------------------------------------------------------------------------------------------------------------

/// P of ADR 0216.
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// P3 of ADR 0216.
fn pair_p3() -> SlidingInterface {
    let a = annulus_3d(48, 4, 2, 1.0, 1.5, 0.4, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(40, 4, 3, 1.5, 2.0, 0.4, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap()
}

/// LP of ADR 0638 (equal axial layers).
fn pair_lp() -> SlidingInterface {
    let a = annulus_3d(48, 4, 2, 1.0, 1.5, 0.4, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(40, 4, 2, 1.5, 2.0, 0.4, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap()
}

fn uniform(n: usize, a: f64, b: f64) -> Vec<f64> {
    (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
}

fn periodic_x(mut m: Mesh) -> Mesh {
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    m
}

fn pair_pp() -> PeriodicSlidingInterface {
    let a = periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap());
    let b = periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap());
    PeriodicSlidingInterface::planar(a, "ymin", b, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap()
}

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

fn pair_s3() -> PeriodicSlidingInterface {
    let a = annulus_3d_sector(12, 4, 2, 1.0, 1.5, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    let b = annulus_3d_sector(10, 4, 3, 1.5, 2.0, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// PS of ADR 0216 with `nu = 0.1` (ADR 0638's `uniform_run`).
fn settings_ps() -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.1);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 2;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

/// PS6 of ADR 0629 (ADR 0640, ADR 0641).
fn settings_ps6() -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.1);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 6;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

fn eq_bits(x: &[f64], y: &[f64]) -> bool {
    x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits())
}

/// E1's fields of the full-annulus step data, bit for bit.
fn same_sliding(a: &SlidingStep, b: &SlidingStep) -> bool {
    eq_bits(&a.mesh_flux, &b.mesh_flux)
        && eq_bits(&a.phi_old, &b.phi_old)
        && eq_bits(&a.old_defect, &b.old_defect)
        && eq_bits(&a.vol_old, &b.vol_old)
        && a.gcl_residual.to_bits() == b.gcl_residual.to_bits()
        && a.flux_residual.to_bits() == b.flux_residual.to_bits()
        && a.interface_faces == b.interface_faces
}

/// E2's fields of the periodic step data, bit for bit.
fn same_periodic(a: &PeriodicSlidingStep, b: &PeriodicSlidingStep) -> bool {
    eq_bits(&a.mesh_flux, &b.mesh_flux)
        && eq_bits(&a.phi_old, &b.phi_old)
        && eq_bits(&a.old_defect, &b.old_defect)
        && eq_bits(&a.vol_old, &b.vol_old)
        && a.gcl_residual.to_bits() == b.gcl_residual.to_bits()
        && a.flux_residual.to_bits() == b.flux_residual.to_bits()
        && a.interface_faces == b.interface_faces
        && a.wrap_pairs == b.wrap_pairs
        && a.images == b.images
        && a.reimage == b.reimage
}

fn root(p: &mut [usize], mut i: usize) -> usize {
    while p[i] != i {
        p[i] = p[p[i]];
        i = p[i];
    }
    i
}

/// The number of connected parts of a graph given by its cell edges.
fn n_parts(nc: usize, edges: &[(usize, usize)]) -> usize {
    let mut p: Vec<usize> = (0..nc).collect();
    let mut touched = vec![false; nc];
    for &(a, b) in edges {
        touched[a] = true;
        touched[b] = true;
        let (ra, rb) = (root(&mut p, a), root(&mut p, b));
        if ra != rb {
            p[ra] = rb;
        }
    }
    let mut r: Vec<usize> = (0..nc).filter(|&c| touched[c]).map(|c| root(&mut p, c)).collect();
    r.sort_unstable();
    r.dedup();
    r.len()
}

/// X3: the parts of the interface graph of a full-annulus mesh.
fn parts_full(m: &SlidingMesh) -> usize {
    let (own, nei) = (m.mesh.owner(), m.mesh.neighbour());
    let edges: Vec<(usize, usize)> = m.report.interface_faces.iter().map(|&f| (own[f], nei[f])).collect();
    n_parts(m.mesh.n_cells(), &edges)
}

// -------------------------------------------------------------------------------------------------------------------
// the full-annulus uniform loop (ADR 0638's `uniform_run`), driven by the default `sliding_step`
// -------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Rec {
    step: usize,
    /// default = `LargestTarget` (F-710's step), bit for bit
    eq_lt: bool,
    /// default = `Off` (the uncoupled step), bit for bit
    eq_off: bool,
    /// `LargestTarget` = `Off`, bit for bit
    lt_eq_off: bool,
    eu: f64,
    ep: f64,
    flux: f64,
    gcl: f64,
    parts: usize,
}

fn uniform_full(sc: &SlidingInterface, u0: Vec3, dt: f64, steps: usize) -> Vec<Rec> {
    let mut cur = sc.mesh_at(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let names: Vec<String> = cur
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty)
        .map(|p| p.name.clone())
        .collect();
    let mut u = VectorField::uniform("U", &g, u0);
    for n in &names {
        u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSolver::new(g, u, p, settings_ps());
    let un = vec3::norm(u0);
    let mut out = vec![];
    for k in 1..=steps {
        let new = sc.mesh_at(k as f64 * dt).unwrap();
        let st = sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let lt = sliding_step_with(
            &s.geom,
            &cur,
            &new,
            &s.phi,
            &s.u.values,
            dt,
            PartCoupling::LargestTarget,
        )
        .unwrap();
        let off = sliding_step_with(&s.geom, &cur, &new, &s.phi, &s.u.values, dt, PartCoupling::Off).unwrap();
        let (eq_lt, eq_off, lt_eq_off) = (same_sliding(&st, &lt), same_sliding(&st, &off), same_sliding(&lt, &off));
        let (flux, gcl) = (st.flux_residual, st.gcl_residual);
        s.step_sliding(dt, st, vec![]).unwrap();
        let eu =
            s.u.values
                .iter()
                .fold(0.0f64, |a, v| a.max(finite_or_inf(vec3::norm(vec3::sub(*v, u0)))))
                / un;
        let p0 = s.p.values[0];
        let ep =
            s.p.values
                .iter()
                .fold(0.0f64, |a, x| a.max(finite_or_inf((x - p0).abs())))
                / (un * un);
        out.push(Rec {
            step: k,
            eq_lt,
            eq_off,
            lt_eq_off,
            eu,
            ep,
            flux,
            gcl,
            parts: parts_full(&new),
        });
        cur = new;
    }
    out
}

fn steps_where(recs: &[Rec], f: impl Fn(&Rec) -> bool) -> Vec<usize> {
    recs.iter().filter(|r| f(r)).map(|r| r.step).collect()
}

/// E1, E1u and NC3 (UC) on ADR 0638's uniform flows.
#[test]
fn e1_uniform_flows_of_adr0638_default_sliding_step_equals_f710() {
    let t0 = Instant::now();
    let mut fails = Fails::new();
    let dt_uc = TAU / 144.0 / 8.0;
    for (name, sc, u0, dt, steps) in [
        ("UC", pair_p(), [1.0, 0.5, 0.0], dt_uc, 50usize),
        ("U2", pair_p(), [1.0, 0.5, 0.0], 0.005, 100),
        ("U3", pair_p3(), [1.0, 0.5, 0.3], 0.01, 40),
        ("LP", pair_lp(), [1.0, 0.5, 0.3], 0.01, 20),
    ] {
        let recs = uniform_full(&sc, u0, dt, steps);
        let not_lt = steps_where(&recs, |r| !r.eq_lt);
        let not_off = steps_where(&recs, |r| !r.eq_off);
        let parts: Vec<usize> = recs.iter().map(|r| r.parts).collect();
        let (pmin, pmax) = (parts.iter().min().unwrap(), parts.iter().max().unwrap());
        check(
            &mut fails,
            "E1",
            recs.len() == steps && not_lt.is_empty(),
            format!(
                "{name}: {} steps, default differs from LargestTarget (F-710) on {not_lt:?} (none); differs from Off on {not_off:?}; parts {pmin} to {pmax}",
                recs.len()
            ),
        );
        if name == "UC" {
            let eu = recs.iter().fold(0.0f64, |a, r| a.max(r.eu));
            let ep = recs.iter().fold(0.0f64, |a, r| a.max(r.ep));
            let coinc: Vec<&Rec> = recs.iter().filter(|r| r.step % 8 == 0).collect();
            let (fl, gc) = coinc.iter().fold((0.0f64, 0.0f64), |a, r| {
                (a.0.max(finite_or_inf(r.flux)), a.1.max(finite_or_inf(r.gcl)))
            });
            check(
                &mut fails,
                "E1u",
                eu <= 1e-10 && ep <= 1e-10 && coinc.len() == 6 && fl <= 1e-12 && gc <= 1e-12,
                format!(
                    "UC: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each); {} coincidence steps: flux residual {fl:.2e}, GCL residual {gc:.2e} (1e-12 each)",
                    coinc.len()
                ),
            );
            let expected: Vec<usize> = (1..=steps).filter(|k| k % 8 == 0).collect();
            check(
                &mut fails,
                "NC3",
                not_off == expected,
                format!("UC: default differs from Off on {not_off:?} (exactly {expected:?})"),
            );
        }
    }
    println!("E1 elapsed {:.1} s", t0.elapsed().as_secs_f64());
    assert!(fails.is_empty(), "{fails:?}");
}

/// E3, X3 and NC2 (AX): an axial uniform flow on the 3-D full annulus P3 at coincidences.
#[test]
fn e3_axial_uniform_flow_on_the_full_annulus_default_equals_the_uncoupled_step() {
    let t0 = Instant::now();
    let steps = 48;
    let recs = uniform_full(&pair_p3(), Z, TAU / 48.0 / 8.0, steps);
    for r in recs.iter().filter(|r| r.step % 8 == 0) {
        println!(
            "AX step {:2}: parts {}, default = Off {}, default = LargestTarget {}, LargestTarget = Off {}, flux residual {:.2e}, GCL {:.2e}",
            r.step, r.parts, r.eq_off, r.eq_lt, r.lt_eq_off, r.flux, r.gcl
        );
    }
    let mut fails = Fails::new();
    let not_off = steps_where(&recs, |r| !r.eq_off);
    let eu = recs.iter().fold(0.0f64, |a, r| a.max(r.eu));
    let ep = recs.iter().fold(0.0f64, |a, r| a.max(r.ep));
    check(
        &mut fails,
        "E3",
        recs.len() == steps && not_off.is_empty() && eu <= 1e-10,
        format!(
            "AX, {} steps: default differs from Off on {not_off:?} (none); velocity {eu:.2e} |U| (1e-10), pressure {ep:.2e} (printed)",
            recs.len()
        ),
    );
    let bad_parts: Vec<(usize, usize)> = recs
        .iter()
        .filter(|r| r.parts != if r.step % 8 == 0 { 8 } else { 1 })
        .map(|r| (r.step, r.parts))
        .collect();
    check(
        &mut fails,
        "X3",
        bad_parts.is_empty(),
        format!("AX: steps whose part count is not 8 on n % 8 == 0 and 1 elsewhere: {bad_parts:?} (none)"),
    );
    let lt_differ = steps_where(&recs, |r| !r.lt_eq_off);
    check(
        &mut fails,
        "NC2",
        !lt_differ.is_empty(),
        format!("AX: LargestTarget differs from Off on {lt_differ:?} (at least one)"),
    );
    println!("E3 elapsed {:.1} s", t0.elapsed().as_secs_f64());
    assert!(fails.is_empty(), "{fails:?}");
}

// -------------------------------------------------------------------------------------------------------------------
// E2: ADR 0640's uniform flows on the periodic path
// -------------------------------------------------------------------------------------------------------------------

/// ADR 0640's uniform loop driven by the base step; on every step the coupled step (FluxScale) and LargestTarget from the
/// same inputs. Returns the steps run, the steps where the coupled step differs from the base, and the steps where
/// LargestTarget differs from the base.
fn uniform_periodic(sc: &PeriodicSlidingInterface, u0: Vec3, dt: f64, steps: usize) -> (usize, Vec<usize>, Vec<usize>) {
    let names: Vec<String> = sc
        .mesh_at(0.0)
        .unwrap()
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty && p.kind != PatchKind::Cyclic)
        .map(|p| p.name.clone())
        .collect();
    let mut cur = sc.mesh_turned(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let mut u = VectorField::uniform("U", &g, u0);
    for n in &names {
        u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSolver::new(g, u, p, settings_ps6());
    let (mut differ, mut lt_differ) = (Vec::new(), Vec::new());
    let mut run = 0;
    for n in 1..=steps {
        let new = sc.mesh_turned(n as f64 * dt).unwrap();
        let st = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let c = periodic_sliding_step_coupled(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let lt = periodic_sliding_step_with(
            &s.geom,
            &cur,
            &new,
            &s.phi,
            &s.u.values,
            dt,
            PartCoupling::LargestTarget,
        )
        .unwrap();
        if !same_periodic(&st, &c) {
            differ.push(n);
        }
        if !same_periodic(&st, &lt) {
            lt_differ.push(n);
        }
        s.step_sliding_periodic(dt, st, Vec::new()).unwrap();
        cur = new;
        run += 1;
    }
    (run, differ, lt_differ)
}

/// E2 and NC1 (CS, CP).
#[test]
fn e2_uniform_flows_of_adr0640_coupled_equals_base() {
    let t0 = Instant::now();
    let (ns3, d3, l3) = uniform_periodic(&pair_s3(), Z, PI / 288.0, 200);
    let (npp, dpp, lpp) = uniform_periodic(&pair_pp(), [1.0, 0.5, 0.0], 1.0 / 640.0, 650);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "E2",
        ns3 == 200 && npp == 650 && d3.is_empty() && dpp.is_empty(),
        format!("CS {ns3} steps, coupled differs from base on {d3:?}; CP {npp} steps, on {dpp:?} (none)"),
    );
    check(
        &mut fails,
        "NC1",
        !l3.is_empty() && !lpp.is_empty(),
        format!(
            "LargestTarget differs from base on {} CS steps {l3:?} and {} CP steps (at least one each)",
            l3.len(),
            lpp.len()
        ),
    );
    println!("E2 elapsed {:.1} s", t0.elapsed().as_secs_f64());
    assert!(fails.is_empty(), "{fails:?}");
}

// -------------------------------------------------------------------------------------------------------------------
// K1, K2: ADR 0641's flows without the parts' symmetry
// -------------------------------------------------------------------------------------------------------------------

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

fn per_face(g: &FvGeometry, patch: &str, f: impl Fn(Vec3) -> Vec3) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| f(cf[k])).collect(),
    )))
}

/// NS's inlet `(1 + 0.5 sin 6 theta) x_hat` (ADR 0641).
fn ns_inlet(x: Vec3) -> Vec3 {
    let th = x[1].atan2(x[0]);
    vec3::scale(xhat(x), 1.0 + 0.5 * (6.0 * th).sin())
}

/// The per-step record of a coupled periodic loop.
struct KRec {
    step: usize,
    eq_lt: bool,
    eq_off: bool,
    flux: f64,
    gcl: f64,
}

fn k_report(name: &str, recs: &[KRec], steps: usize, coincidences: usize, fails: &mut Fails) {
    let not_lt: Vec<usize> = recs.iter().filter(|r| !r.eq_lt).map(|r| r.step).collect();
    let not_off: Vec<usize> = recs.iter().filter(|r| !r.eq_off).map(|r| r.step).collect();
    let flux = recs
        .iter()
        .filter(|r| r.step >= 2)
        .fold(0.0f64, |a, r| a.max(finite_or_inf(r.flux)));
    let gcl = recs.iter().fold(0.0f64, |a, r| a.max(finite_or_inf(r.gcl)));
    let expected: Vec<usize> = (1..=steps).filter(|k| k % 8 == 0).collect();
    let id = if name == "NS" { "K1" } else { "K2" };
    check(
        fails,
        id,
        recs.len() == steps && not_lt.is_empty() && (name == "NS" || flux <= 1e-12),
        format!(
            "{name}: {} steps, coupled differs from LargestTarget (F-721's step) on {not_lt:?} (none); flux residual steps 2 to {steps} {flux:.2e}{}",
            recs.len(),
            if name == "NS" { "" } else { " (1e-12)" }
        ),
    );
    if name == "NS" {
        check(
            fails,
            "K1r",
            flux <= 1e-12 && gcl <= 1e-12,
            format!("NS: coupled flux residual steps 2 to {steps} {flux:.2e}, GCL residual every step {gcl:.2e} (1e-12 each)"),
        );
    }
    check(
        fails,
        "NC3",
        not_off == expected && expected.len() == coincidences,
        format!(
            "{name}: coupled differs from Off on {} steps, exactly the {coincidences} coincidence steps: {}",
            not_off.len(),
            not_off == expected
        ),
    );
}

/// K1, K1r and NC3 (NS): the sector flow of ADR 0641 through the coupled step, 400 steps.
#[test]
fn k1_non_symmetric_sector_flow_of_adr0641_coupled_equals_f721() {
    let t0 = Instant::now();
    let (dt, steps) = (PI / 576.0, 400);
    let sc = pair_ss();
    let mut cur = sc.mesh_turned(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let mut u = VectorField::from_fn("U", &g, |c| vec3::scale(xhat(c), 1.0 / c[0].hypot(c[1])));
    u.set_bc(&g, "inner", per_face(&g, "inner", ns_inlet)).unwrap();
    u.set_bc(&g, "outer", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, "outer", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = PimpleSolver::new(g, u, p, settings_ps6());
    let mut recs = Vec::new();
    for n in 1..=steps {
        let new = sc.mesh_turned(n as f64 * dt).unwrap();
        let st = periodic_sliding_step_coupled(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let lt = periodic_sliding_step_with(
            &s.geom,
            &cur,
            &new,
            &s.phi,
            &s.u.values,
            dt,
            PartCoupling::LargestTarget,
        )
        .unwrap();
        let off = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        recs.push(KRec {
            step: n,
            eq_lt: same_periodic(&st, &lt),
            eq_off: same_periodic(&st, &off),
            flux: st.flux_residual,
            gcl: st.gcl_residual,
        });
        let g2 = st.geom.clone();
        s.u.set_bc(&g2, "inner", per_face(&g2, "inner", ns_inlet)).unwrap();
        s.step_sliding_periodic(dt, st, Vec::new()).unwrap();
        cur = new;
    }
    let mut fails = Fails::new();
    k_report("NS", &recs, steps, 50, &mut fails);
    println!("K1 elapsed {:.1} s", t0.elapsed().as_secs_f64());
    assert!(fails.is_empty(), "{fails:?}");
}

/// K2 and NC3 (PL): the planar through-flow of ADR 0641 through the coupled step, 160 steps.
#[test]
fn k2_planar_through_flow_of_adr0641_coupled_equals_f721() {
    let t0 = Instant::now();
    let (dt, steps) = (1.0 / 640.0, 160);
    let sc = pair_pp();
    let mut cur = sc.mesh_turned(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let inlet = |x: Vec3| [0.0, 0.5 * (1.0 + 0.5 * (2.0 * PI * x[0]).sin()), 0.0];
    let mut u = VectorField::uniform("U", &g, [0.0, 0.5, 0.0]);
    u.set_bc(&g, "ymin", per_face(&g, "ymin", inlet)).unwrap();
    u.set_bc(&g, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = PimpleSolver::new(g, u, p, settings_ps6());
    let mut recs = Vec::new();
    for n in 1..=steps {
        let new = sc.mesh_turned(n as f64 * dt).unwrap();
        let st = periodic_sliding_step_coupled(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let lt = periodic_sliding_step_with(
            &s.geom,
            &cur,
            &new,
            &s.phi,
            &s.u.values,
            dt,
            PartCoupling::LargestTarget,
        )
        .unwrap();
        let off = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        recs.push(KRec {
            step: n,
            eq_lt: same_periodic(&st, &lt),
            eq_off: same_periodic(&st, &off),
            flux: st.flux_residual,
            gcl: st.gcl_residual,
        });
        s.step_sliding_periodic(dt, st, Vec::new()).unwrap();
        cur = new;
    }
    let mut fails = Fails::new();
    k_report("PL", &recs, steps, 20, &mut fails);
    println!("K2 elapsed {:.1} s", t0.elapsed().as_secs_f64());
    assert!(fails.is_empty(), "{fails:?}");
}
