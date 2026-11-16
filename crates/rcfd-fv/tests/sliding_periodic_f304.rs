//! F-304 (ADR 0629): planar and sector sliding interfaces with periodic seams in the incompressible PISO path
//! (`rcfd_mesh::sliding_periodic`, `rcfd_fv::sliding_periodic`, `PimpleSolver::step_sliding_periodic`). Checks K4 and K4n
//! (the periodic pairs kept by the join), P (the planar slide: mass and a uniform flow), S (the sector: mass and a uniform
//! axial flow), their negative controls, E (refusals) and X0. The limits are those registered in ADR 0629 before this code
//! ran; each test prints every measured number and then asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, StepInfo};
use rcfd_fv::sliding_periodic::{periodic_sliding_step, PeriodicSlidingStep};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, annulus_sector, rectilinear_2d, Grading};
use rcfd_mesh::non_conformal::{interface_non_orthogonality, join_non_conformal, join_non_conformal_cylindrical};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh, WRAP_A, WRAP_B};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// PS6 of ADR 0629.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 6;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn uniform(n: usize, a: f64, b: f64) -> Vec<f64> {
    (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
}

fn periodic_x(mut m: Mesh) -> Mesh {
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    m
}

fn pp_blocks() -> (Mesh, Mesh) {
    (
        periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap()),
        periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap()),
    )
}

fn ss_blocks() -> (Mesh, Mesh) {
    (
        annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap(),
        annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap(),
    )
}

fn s3_blocks() -> (Mesh, Mesh) {
    (
        annulus_3d_sector(12, 4, 2, 1.0, 1.5, 0.4, FRAC_PI_3, Grading::Uniform).unwrap(),
        annulus_3d_sector(10, 4, 3, 1.5, 2.0, 0.4, FRAC_PI_3, Grading::Uniform).unwrap(),
    )
}

/// PP of ADR 0629.
fn pair_pp() -> PeriodicSlidingInterface {
    let (a, b) = pp_blocks();
    PeriodicSlidingInterface::planar(a, "ymin", b, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap()
}

/// SS of ADR 0629.
fn pair_ss() -> PeriodicSlidingInterface {
    let (a, b) = ss_blocks();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// S3 of ADR 0629.
fn pair_s3() -> PeriodicSlidingInterface {
    let (a, b) = s3_blocks();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

fn per_face(g: &FvGeometry, patch: &str, f: impl Fn(Vec3) -> Vec3) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| f(cf[k])).collect(),
    )))
}

/// GCL residual of a step's data, computed here: `max_c abs(V1 - V0 - dt sum sigma m) / mean(V1)`.
fn gcl_of(st: &PeriodicSlidingStep, dt: f64) -> f64 {
    let m = &st.geom.mesh;
    let ni = m.n_internal_faces();
    let mut s = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        s[m.owner()[f]] += st.mesh_flux[f] * dt;
        if f < ni {
            s[m.neighbour()[f]] -= st.mesh_flux[f] * dt;
        }
    }
    let v1 = m.cell_volumes();
    let mean = v1.iter().sum::<f64>() / v1.len() as f64;
    (0..v1.len()).fold(0.0f64, |a, c| a.max((v1[c] - st.vol_old[c] - s[c]).abs())) / mean
}

/// Continuity of the remapped old flux: `max_c abs(sum sigma phi_old) / max abs(S)`.
fn remap_of(st: &PeriodicSlidingStep) -> f64 {
    let m = &st.geom.mesh;
    let ni = m.n_internal_faces();
    let mut d = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        d[m.owner()[f]] += st.phi_old[f];
        if f < ni {
            d[m.neighbour()[f]] -= st.phi_old[f];
        }
    }
    let smax = m.face_area_vectors().iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
    d.iter().fold(0.0f64, |a, x| a.max(x.abs())) / smax
}

/// The owner-cell pairs of the interface: direct faces and wrap pairs.
fn pairs_of(m: &PeriodicSlidingMesh) -> Vec<(usize, usize)> {
    let (own, nei) = (m.mesh.owner(), m.mesh.neighbour());
    let mut v: Vec<(usize, usize)> = m
        .report
        .join
        .interface_faces
        .iter()
        .map(|&f| (own[f], nei[f]))
        .collect();
    v.extend(m.report.wrap_pairs.iter().map(|&(a, b)| (own[a], own[b])));
    v.sort_unstable();
    v
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    cont: f64,
    gcl: f64,
    remap_late: f64,
    non_orth: f64,
    steps: usize,
    changed: usize,
}

type BcHook<'b> = &'b mut dyn FnMut(&mut PimpleSolver, &PeriodicSlidingStep);
type AlterHook<'b> = &'b mut dyn FnMut(&mut PeriodicSlidingStep, &PimpleSolver);

/// The loop SL of ADR 0216 on a periodic sliding pair.
struct Sliding<'a> {
    sc: &'a PeriodicSlidingInterface,
    cur: PeriodicSlidingMesh,
    s: PimpleSolver,
    speed: f64,
    dt: f64,
    stats: Stats,
}

impl<'a> Sliding<'a> {
    fn new(
        sc: &'a PeriodicSlidingInterface,
        make: impl FnOnce(Arc<FvGeometry>) -> PimpleSolver,
        speed: f64,
        dt: f64,
    ) -> Self {
        let cur = sc.mesh_at(0.0).unwrap();
        let s = make(geom_of(&cur.mesh));
        Sliding {
            sc,
            cur,
            s,
            speed,
            dt,
            stats: Stats::default(),
        }
    }

    fn step(&mut self, bc: BcHook, alter: AlterHook) -> StepInfo {
        let off = self.speed * (self.stats.steps + 1) as f64 * self.dt;
        let new = self.sc.mesh_at(off).unwrap();
        let mut st =
            periodic_sliding_step(&self.s.geom, &self.cur, &new, &self.s.phi, &self.s.u.values, self.dt).unwrap();
        alter(&mut st, &self.s);
        let no = interface_non_orthogonality(&new.mesh, &st.interface_faces, 70.0).unwrap();
        let first = self.stats.steps == 0;
        let s = &mut self.stats;
        s.gcl = s.gcl.max(gcl_of(&st, self.dt));
        if !first {
            s.remap_late = s.remap_late.max(remap_of(&st));
        }
        s.non_orth = s.non_orth.max(no.worst_deg);
        if pairs_of(&new) != pairs_of(&self.cur) {
            s.changed += 1;
        }
        bc(&mut self.s, &st);
        let info = self.s.step_sliding_periodic(self.dt, st, Vec::new()).unwrap();
        assert!(
            self.s.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity at step {}",
            self.stats.steps + 1
        );
        self.stats.cont = self.stats.cont.max(info.continuity);
        self.stats.steps += 1;
        self.cur = new;
        info
    }

    /// Net flux out of the rotor through the interface (direct faces and rotor copies), the direct faces alone, and the
    /// summed area.
    fn interface_flux(&self) -> (f64, f64, f64) {
        let m = &self.s.geom.mesh;
        let phi = &self.s.phi;
        let direct: f64 = self.cur.report.join.interface_faces.iter().map(|&f| phi[f]).sum();
        let wrap: f64 = self.cur.report.wrap_pairs.iter().map(|&(a, _)| phi[a]).sum();
        let area: f64 = self
            .cur
            .report
            .join
            .interface_faces
            .iter()
            .map(|&f| m.face_area(f))
            .chain(self.cur.report.wrap_pairs.iter().map(|&(a, _)| m.face_area(a)))
            .sum();
        (direct + wrap, direct, area)
    }

    /// `sum_pairs abs(phi_f + phi_g)` over every periodic pair (seams and wrap).
    fn pair_sum(&self) -> f64 {
        let pairs = self.s.geom.periodic_pairs().unwrap();
        pairs.iter().map(|p| (self.s.phi[p.f] + self.s.phi[p.g]).abs()).sum()
    }
}

fn no_bc() -> impl FnMut(&mut PimpleSolver, &PeriodicSlidingStep) {
    |_: &mut PimpleSolver, _: &PeriodicSlidingStep| {}
}

fn no_alter() -> impl FnMut(&mut PeriodicSlidingStep, &PimpleSolver) {
    |_: &mut PeriodicSlidingStep, _: &PimpleSolver| {}
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

fn require(fails: &Fails, ids: &[&str]) {
    let bad: Vec<_> = fails.iter().filter(|x| ids.contains(&x.0)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

fn x0(fails: &mut Fails, id: &'static str, name: &str, st: &Stats, offset: f64, pitch: f64) {
    check(
        fails,
        id,
        st.non_orth <= 70.0 && offset >= pitch,
        format!(
            "{name}: worst direct-interface non-orthogonality {:.2} deg (70), offset {offset:.4} against the stator pitch {pitch:.4}",
            st.non_orth
        ),
    );
}

// -------------------------------------------------------------------------------------------------------------------
// K4. Periodic pairs kept by the join
// -------------------------------------------------------------------------------------------------------------------

/// The pairs of a mesh as sets of `((patch, local), (patch, local))`, seams and wrap separately.
type Key = (String, usize);
type PairSets = (BTreeSet<(Key, Key)>, BTreeSet<(usize, usize)>);
fn pair_keys(m: &Mesh, offsets: &[(String, usize)]) -> PairSets {
    let g = FvGeometry::new(Arc::new(m.clone())).unwrap();
    let key = |f: usize| -> Key {
        let p = m.patch_of_face(f).unwrap();
        let shift = offsets.iter().find(|x| x.0 == p.name).map_or(0, |x| x.1);
        (p.name.clone(), f - p.start + shift)
    };
    let (mut seam, mut wrap) = (BTreeSet::new(), BTreeSet::new());
    for pr in g.periodic_pairs().unwrap() {
        let name = &m.patch_of_face(pr.f).unwrap().name;
        if name == WRAP_A || name == WRAP_B {
            wrap.insert((pr.f.min(pr.g), pr.f.max(pr.g)));
        } else {
            let (a, b) = (key(pr.f), key(pr.g));
            seam.insert(if a < b { (a, b) } else { (b, a) });
        }
    }
    (seam, wrap)
}

fn k4_on(fails: &mut Fails, name: &str, sc: &PeriodicSlidingInterface, offsets: &[f64], n_seam: usize) {
    let (rotor, stator) = (sc.rotor(), sc.stator());
    // expected seam pairs: the rotor's, and the stator's with its faces after the rotor's in a merged patch
    let (rot_pairs, _) = pair_keys(rotor, &[]);
    let shifts: Vec<(String, usize)> = stator
        .patches()
        .iter()
        .map(|p| (p.name.clone(), rotor.patch(&p.name).map_or(0, |q| q.size)))
        .collect();
    let (st_pairs, _) = pair_keys(stator, &shifts);
    let expected: BTreeSet<(Key, Key)> = rot_pairs.union(&st_pairs).cloned().collect();
    for &o in offsets {
        let m = sc.mesh_at(o).unwrap();
        // (a) partners and transforms kept
        let mut kept = true;
        for blk in [rotor, stator] {
            for p in blk.patches().iter().filter(|p| p.periodic_with.is_some()) {
                let q = m.mesh.patch(&p.name).unwrap();
                kept &= q.periodic_with == p.periodic_with && q.rotation == p.rotation && q.kind == PatchKind::Cyclic;
            }
        }
        let (seam, wrap) = pair_keys(&m.mesh, &[]);
        let reported: BTreeSet<(usize, usize)> =
            m.report.wrap_pairs.iter().map(|&(a, b)| (a.min(b), a.max(b))).collect();
        let n_wrap = m.report.wrap_pairs.len();
        let ok_b = seam.len() == n_seam && wrap.len() == n_wrap;
        let ok_c = seam == expected;
        let ok_d = wrap == reported;
        let ok_e = if o == 0.0 { n_wrap == 0 } else { n_wrap > 0 };
        check(
            fails,
            "K4",
            kept && ok_b && ok_c && ok_d && ok_e,
            format!(
                "{name} offset {o}: (a) kept {kept}; (b) {} seam pairs ({n_seam}) and {} wrap pairs ({n_wrap}); (c) seam identity {ok_c}; \
                 (d) wrap identity {ok_d}; (e) wrap count {ok_e}",
                seam.len(),
                wrap.len()
            ),
        );
    }
}

/// K4 and K4n of ADR 0629.
#[test]
fn k4_periodic_pairs_kept_by_the_join() {
    let mut fails = Fails::new();
    k4_on(&mut fails, "PP", &pair_pp(), &[0.0, 0.0123, 0.3, 0.9], 16);
    k4_on(&mut fails, "SS", &pair_ss(), &[0.0, 0.0123, 0.3, 1.0], 16);
    k4_on(&mut fails, "S3", &pair_s3(), &[0.0, 0.0123, 0.3, 1.0], 20);
    // K4n: the existing joins drop the pairs
    let (a, b) = pp_blocks();
    let (m, _) = join_non_conformal(&a, "ymin", &b, "ymax", 1e-8).unwrap();
    let n_pp = geom_of(&m).periodic_pairs().unwrap().len();
    let (a, b) = ss_blocks();
    let (m, _) = join_non_conformal_cylindrical(&a, "outer", &b, "inner", vec3::ZERO, Z, 1e-8).unwrap();
    let n_ss = geom_of(&m).periodic_pairs().unwrap().len();
    check(
        &mut fails,
        "K4n",
        n_pp == 0 && n_ss == 0,
        format!("the existing joins at offset 0: {n_pp} (PP) and {n_ss} (SS) periodic pairs (0, against K4's 16)"),
    );
    require(&fails, &["K4", "K4n"]);
}

// -------------------------------------------------------------------------------------------------------------------
// P. The planar slide
// -------------------------------------------------------------------------------------------------------------------

const PP_DT: f64 = 0.0049;

/// P1 to P7, PN1 and X0p (PF) of ADR 0629, and P1c, PN1c of its amendment 1.
fn pf_checks() -> Fails {
    let sc = pair_pp();
    let inlet = |x: Vec3| [0.0, 0.5 * (1.0 + 0.5 * (2.0 * PI * x[0]).sin()), 0.0];
    let mut run = Sliding::new(
        &sc,
        |g| {
            let mut u = VectorField::uniform("U", &g, [0.0, 0.5, 0.0]);
            u.set_bc(&g, "ymin", per_face(&g, "ymin", inlet)).unwrap();
            u.set_bc(&g, "ymax", Box::new(ZeroGradient)).unwrap();
            let mut p = ScalarField::uniform("p", &g, 0.0);
            p.set_bc(&g, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
            PimpleSolver::new(g, u, p, settings(0.1))
        },
        1.0,
        PP_DT,
    );
    let (mut p1, mut p1c, mut p2, mut p6): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    let mut n_wrap = 0;
    for _ in 0..100 {
        run.step(&mut no_bc(), &mut no_alter());
        let m = &run.s.geom.mesh;
        let phi = &run.s.phi;
        let q_in = -m.patch("ymin").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let q_out = m.patch("ymax").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let (q_i, _, _) = run.interface_flux();
        p1 = p1.max((q_i - q_in).abs() / q_in);
        p1c = p1c.max((q_i + q_in).abs() / q_in);
        p2 = p2.max((q_out - q_in).abs() / q_in);
        p6 = p6.max(run.pair_sum() / q_in);
        n_wrap = run.cur.report.wrap_pairs.len();
    }
    let m = &run.s.geom.mesh;
    let q_in = -m.patch("ymin").unwrap().faces().map(|f| run.s.phi[f]).sum::<f64>();
    let (_, direct, _) = run.interface_flux();
    let pn1 = (direct - q_in).abs() / q_in;
    let pn1c = (direct + q_in).abs() / q_in;
    let st = run.stats;
    println!("PF: printed, no limit: Q_in {q_in:.6e}, wrap pairs at the end {n_wrap}");
    let mut fails = Fails::new();
    check(
        &mut fails,
        "P1",
        p1 <= 1e-10,
        format!("|Q_I - Q_in|/Q_in {p1:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "P1c",
        p1c <= 1e-10,
        format!("|Q_I + Q_in|/Q_in {p1c:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "P2",
        p2 <= 1e-10,
        format!("|Q_out - Q_in|/Q_in {p2:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "P3",
        st.cont <= 1e-9,
        format!("continuity {:.2e} (1e-9)", st.cont),
    );
    check(
        &mut fails,
        "P4",
        st.gcl <= 1e-12,
        format!("GCL {:.2e} mean(V) (1e-12)", st.gcl),
    );
    check(
        &mut fails,
        "P5",
        st.remap_late <= 1e-9,
        format!("remapped old flux from the second step {:.2e} (1e-9)", st.remap_late),
    );
    check(
        &mut fails,
        "P6",
        p6 <= 1e-12,
        format!("periodic pair sums {p6:.2e} Q_in (1e-12)"),
    );
    check(
        &mut fails,
        "P7",
        st.changed >= 30,
        format!(
            "interface pairs changed in {} of {} steps (>= 30; derived 40)",
            st.changed, st.steps
        ),
    );
    check(
        &mut fails,
        "PN1",
        pn1 >= 0.1,
        format!("direct faces alone {pn1:.3e} Q_in off (>= 0.1)"),
    );
    check(
        &mut fails,
        "PN1c",
        pn1c >= 0.1,
        format!("direct faces alone, corrected sign, {pn1c:.3e} Q_in off (>= 0.1)"),
    );
    x0(&mut fails, "X0p", "PF", &st, run.cur.offset, 1.0 / 16.0);
    fails
}

/// PF of ADR 0629: the checks met in the registered run and P1c, PN1c of amendment 1.
#[test]
fn pf_planar_through_flow_conserves_mass_across_the_sliding_interface() {
    require(
        &pf_checks(),
        &["P1c", "P2", "P3", "P4", "P5", "P6", "P7", "PN1c", "X0p"],
    );
}

/// PF as registered: P1 2.00e0 (1e-10), the flux out of the slider measured against the inflow through the stator with the
/// wrong sign (ADR 0629 amendment 1).
#[test]
#[ignore = "known miss: P1 2.00e0 (1e-10), the registered measure has the wrong sign for the planar set-up (ADR 0629 amendment 1)"]
fn pf_known_miss_p1_sign_of_the_interface_flux() {
    require(&pf_checks(), &["P1"]);
}

/// Largest `max_c |u_c - U|/|U|` and `max_c |p_c - p_0|/|U|^2` over the steps, the net interface flux measure and the
/// stats.
fn uniform_run(
    sc: &PeriodicSlidingInterface,
    u0: Vec3,
    speed: f64,
    dt: f64,
    steps: usize,
    inflow: Option<&str>,
    alter: AlterHook,
) -> (f64, f64, (f64, f64), Stats, f64) {
    let names: Vec<String> = sc
        .mesh_at(0.0)
        .unwrap()
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty && p.kind != PatchKind::Cyclic)
        .map(|p| p.name.clone())
        .collect();
    let mut run = Sliding::new(
        sc,
        |g| {
            let mut u = VectorField::uniform("U", &g, u0);
            for n in &names {
                u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
            }
            let p = ScalarField::uniform("p", &g, 0.0);
            PimpleSolver::new(g, u, p, settings(0.1))
        },
        speed,
        dt,
    );
    let un = vec3::norm(u0);
    let (mut eu, mut ep, mut eq, mut eqc): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0);
    for _ in 0..steps {
        run.step(&mut no_bc(), alter);
        eu = eu.max(
            run.s
                .u
                .values
                .iter()
                .fold(0.0f64, |a, v| a.max(vec3::norm(vec3::sub(*v, u0))))
                / un,
        );
        let p0 = run.s.p.values[0];
        ep = ep.max(run.s.p.values.iter().fold(0.0f64, |a, p| a.max((p - p0).abs())) / (un * un));
        let (q_i, _, area) = run.interface_flux();
        let (a, b) = match inflow {
            Some(name) => {
                let m = &run.s.geom.mesh;
                let q_in = -m.patch(name).unwrap().faces().map(|f| run.s.phi[f]).sum::<f64>();
                ((q_i - q_in).abs() / q_in, (q_i + q_in).abs() / q_in)
            }
            None => (q_i.abs() / (un * area), q_i.abs() / (un * area)),
        };
        eq = eq.max(a);
        eqc = eqc.max(b);
    }
    (eu, ep, (eq, eqc), run.stats, run.cur.offset)
}

/// P8, P9 and X0p (PU) of ADR 0629, and P9c of its amendment 1.
fn pu_checks() -> Fails {
    let (eu, ep, (eq, eqc), st, off) = uniform_run(
        &pair_pp(),
        [1.0, 0.5, 0.0],
        1.0,
        PP_DT,
        100,
        Some("ymin"),
        &mut no_alter(),
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "P8",
        eu <= 1e-10 && ep <= 1e-10,
        format!("PU: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)"),
    );
    check(
        &mut fails,
        "P9",
        eq <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "PU: |Q_I - Q_in|/Q_in {eq:.2e} (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    check(
        &mut fails,
        "P9c",
        eqc <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "PU: |Q_I + Q_in|/Q_in {eqc:.2e} (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    x0(&mut fails, "X0p", "PU", &st, off, 1.0 / 16.0);
    fails
}

/// PU of ADR 0629: the checks met in the registered run and P9c of amendment 1.
#[test]
fn pu_uniform_flow_stays_uniform_across_the_planar_slide() {
    require(&pu_checks(), &["P8", "P9c", "X0p"]);
}

/// PU as registered: P9's net flux 2.00e0 (1e-10), the sign of the measure (ADR 0629 amendment 1).
#[test]
#[ignore = "known miss: P9 net flux 2.00e0 (1e-10), the registered measure has the wrong sign for the planar set-up (ADR 0629 amendment 1)"]
fn pu_known_miss_p9_sign_of_the_interface_flux() {
    require(&pu_checks(), &["P9"]);
}

/// PN2 of ADR 0629 (negative control of P8).
#[test]
fn pn2_uniform_flow_negative_control_wrap_flux_dropped() {
    let mut zero_wrap = |st: &mut PeriodicSlidingStep, _: &PimpleSolver| {
        for &(a, b) in &st.wrap_pairs {
            st.phi_old[a] = 0.0;
            st.phi_old[b] = 0.0;
        }
    };
    let (eu, _, _, _, _) = uniform_run(
        &pair_pp(),
        [1.0, 0.5, 0.0],
        1.0,
        PP_DT,
        100,
        Some("ymin"),
        &mut zero_wrap,
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "PN2",
        eu >= 1e-6,
        format!("wrap old flux zero: velocity {eu:.2e} |U| (>= 1e-6)"),
    );
    require(&fails, &["PN2"]);
}

// -------------------------------------------------------------------------------------------------------------------
// S. The sector
// -------------------------------------------------------------------------------------------------------------------

/// S1 to S7, SN1 and X0s (SF) of ADR 0629.
#[test]
fn sf_radial_source_flow_through_the_sector_sliding_interface() {
    let (u_in, ri, nu, omega, dt) = (1.0, 1.0, 0.1, 1.0, 0.005);
    let sc = pair_ss();
    let mut run = Sliding::new(
        &sc,
        |g| {
            let mut u = VectorField::from_fn("U", &g, |c| vec3::scale(xhat(c), u_in * ri / c[0].hypot(c[1])));
            u.set_bc(&g, "inner", per_face(&g, "inner", |x| vec3::scale(xhat(x), u_in)))
                .unwrap();
            u.set_bc(&g, "outer", Box::new(ZeroGradient)).unwrap();
            let mut p = ScalarField::uniform("p", &g, 0.0);
            p.set_bc(&g, "outer", Box::new(FixedValue::scalar(0.0))).unwrap();
            PimpleSolver::new(g, u, p, settings(nu))
        },
        omega,
        dt,
    );
    let mut bc = |s: &mut PimpleSolver, st: &PeriodicSlidingStep| {
        s.u.set_bc(
            &st.geom,
            "inner",
            per_face(&st.geom, "inner", |x| vec3::scale(xhat(x), u_in)),
        )
        .unwrap();
    };
    let (mut s1, mut s2, mut s6): (f64, f64, f64) = (0.0, 0.0, 0.0);
    let mut n_wrap = 0;
    for _ in 0..100 {
        run.step(&mut bc, &mut no_alter());
        let m = &run.s.geom.mesh;
        let phi = &run.s.phi;
        let q_in = -m.patch("inner").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let q_out = m.patch("outer").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let (q_i, _, _) = run.interface_flux();
        s1 = s1.max((q_i - q_in).abs() / q_in);
        s2 = s2.max((q_out - q_in).abs() / q_in);
        s6 = s6.max(run.pair_sum() / q_in);
        n_wrap = run.cur.report.wrap_pairs.len();
    }
    let m = &run.s.geom.mesh;
    let q_in = -m.patch("inner").unwrap().faces().map(|f| run.s.phi[f]).sum::<f64>();
    let (_, direct, _) = run.interface_flux();
    let sn1 = (direct - q_in).abs() / q_in;
    let cc = m.cell_centres();
    let err = run
        .s
        .u
        .values
        .iter()
        .zip(cc)
        .map(|(v, c)| vec3::norm(vec3::sub(*v, vec3::scale(xhat(*c), u_in * ri / c[0].hypot(c[1])))))
        .fold(0.0, f64::max);
    let st = run.stats;
    println!(
        "SF: printed, no limit: max |u - U_in r_i/r| = {:.3e} U_in (ADR 0216, full annulus: 6.19e-3); wrap pairs at the end {n_wrap}",
        err / u_in
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "S1",
        s1 <= 1e-10,
        format!("|Q_I - Q_in|/Q_in {s1:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "S2",
        s2 <= 1e-10,
        format!("|Q_out - Q_in|/Q_in {s2:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "S3",
        st.cont <= 1e-9,
        format!("continuity {:.2e} (1e-9)", st.cont),
    );
    check(
        &mut fails,
        "S4",
        st.gcl <= 1e-12,
        format!("GCL {:.2e} mean(V) (1e-12)", st.gcl),
    );
    check(
        &mut fails,
        "S5",
        st.remap_late <= 1e-9,
        format!("remapped old flux from the second step {:.2e} (1e-9)", st.remap_late),
    );
    check(
        &mut fails,
        "S6",
        s6 <= 1e-12,
        format!("periodic pair sums {s6:.2e} Q_in (1e-12)"),
    );
    check(
        &mut fails,
        "S7",
        st.changed >= 45,
        format!(
            "interface pairs changed in {} of {} steps (>= 45; derived 58)",
            st.changed, st.steps
        ),
    );
    check(
        &mut fails,
        "SN1",
        sn1 >= 0.1,
        format!("direct faces alone {sn1:.3e} Q_in off (>= 0.1)"),
    );
    x0(&mut fails, "X0s", "SF", &st, run.cur.offset, PI / 60.0);
    require(&fails, &["S1", "S2", "S3", "S4", "S5", "S6", "S7", "SN1", "X0s"]);
}

/// S8, S9 and X0s (SU) of ADR 0629.
#[test]
fn su_uniform_axial_flow_stays_uniform_on_the_3d_sector_slide() {
    let (eu, ep, (eq, _), st, off) = uniform_run(&pair_s3(), [0.0, 0.0, 1.0], 1.0, 0.01, 40, None, &mut no_alter());
    let mut fails = Fails::new();
    check(
        &mut fails,
        "S8",
        eu <= 1e-10 && ep <= 1e-10,
        format!("SU: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)"),
    );
    check(
        &mut fails,
        "S9",
        eq <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "SU: |Q_I|/(|U| A_I) {eq:.2e} (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    x0(&mut fails, "X0s", "SU", &st, off, PI / 30.0);
    require(&fails, &["S8", "S9", "X0s"]);
}

/// SN2 of ADR 0629 (negative control of S8).
#[test]
fn sn2_uniform_axial_flow_negative_control_gcl_not_closed() {
    let mut zero_if = |st: &mut PeriodicSlidingStep, _: &PimpleSolver| {
        for &f in &st.interface_faces {
            st.mesh_flux[f] = 0.0;
        }
        for &(a, b) in &st.wrap_pairs {
            st.mesh_flux[a] = 0.0;
            st.mesh_flux[b] = 0.0;
        }
    };
    let (eu, _, _, _, _) = uniform_run(&pair_s3(), [0.0, 0.0, 1.0], 1.0, 0.01, 40, None, &mut zero_if);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "SN2",
        eu >= 1e-6,
        format!("interface and wrap mesh flux zero: velocity {eu:.2e} |U| (>= 1e-6)"),
    );
    require(&fails, &["SN2"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E. Refusals
// -------------------------------------------------------------------------------------------------------------------

fn state_bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    v.extend(s.p.values.iter().map(|x| x.to_bits()));
    v.extend(s.phi.iter().map(|x| x.to_bits()));
    v.push(s.time.to_bits());
    v
}

/// E1 and E2 of ADR 0629.
#[test]
fn e_refusals_leave_the_solver_unchanged() {
    let dt = 0.005;
    let sc = pair_ss();
    let (m0, m1) = (sc.mesh_at(0.0).unwrap(), sc.mesh_at(dt).unwrap());
    let make = |scheme: TimeScheme| {
        let g = geom_of(&m0.mesh);
        let u = VectorField::uniform("U", &g, [0.0, 0.0, 0.0]);
        let p = ScalarField::uniform("p", &g, 0.0);
        let mut s = PimpleSolver::new(g, u, p, settings(0.1));
        s.settings.time_scheme = scheme;
        s
    };
    let mut fails = Fails::new();
    let mut s = make(TimeScheme::Bdf2);
    let st = periodic_sliding_step(&s.geom, &m0, &m1, &s.phi, &s.u.values, dt).unwrap();
    let before = state_bits(&s);
    let r1 = s.step_sliding_periodic(dt, st, Vec::new());
    let e1a = r1.is_err() && state_bits(&s) == before;
    println!("E1 BDF2: {r1:?}");
    let pp = pair_pp();
    let (a, b) = (pp.mesh_at(0.0).unwrap(), pp.mesh_at(0.0049).unwrap());
    let ga = geom_of(&a.mesh);
    let st_pp = periodic_sliding_step(
        &ga,
        &a,
        &b,
        &vec![0.0; a.mesh.n_faces()],
        &vec![vec3::ZERO; a.mesh.n_cells()],
        0.0049,
    )
    .unwrap();
    let mut s = make(TimeScheme::ImplicitEuler);
    let before = state_bits(&s);
    let r2 = s.step_sliding_periodic(dt, st_pp, Vec::new());
    let e1b = r2.is_err() && state_bits(&s) == before;
    println!("E1 another pair: {r2:?}");
    check(
        &mut fails,
        "E1",
        e1a && e1b,
        format!("refused, unchanged: {e1a}, {e1b}"),
    );
    let s = make(TimeScheme::ImplicitEuler);
    let short = &s.phi[..s.phi.len() - 1];
    let r3 = periodic_sliding_step(&s.geom, &m0, &m1, short, &s.u.values, dt);
    println!("E2 flux one short: {:?}", r3.as_ref().err());
    check(
        &mut fails,
        "E2",
        r3.is_err(),
        "periodic_sliding_step refuses a short flux".into(),
    );
    require(&fails, &["E1", "E2"]);
}
