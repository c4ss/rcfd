//! F-454 (ADR 0356): the fractional step's steady lid-driven cavity (ADR 0245 C3 with the fractional step, ADR 0352 F0) traced
//! to its face term (T1) and made independent of the step by the opt-in `PimpleSettings::fractional_step_consistent` (A1, A2,
//! X1); the defaults (D0 to D2). The limits were registered in ADR 0356 before this file existed. Helpers are copied from
//! `step_consistent_paths_f443.rs` (not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values_pbc, pressure_bc};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::cavity_2d;
use std::sync::Arc;

/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS (as `bdf2_remainder_f344.rs`).
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit` (a NaN fails).
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    /// `value >= limit` (a NaN fails).
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, id: &str, value: f64, lo: f64, hi: f64) {
        let ok = value >= lo && value <= hi;
        println!(
            "{id}: {value:.4e} (limit in [{lo:.3e}, {hi:.3e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} not in [{lo:e}, {hi:e}]"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: String) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn field_distance(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)))
        .fold(0.0, f64::max)
}

/// CAV of ADR 0352 (ADR 0245 C3): the PISO settings with `n_outer = 3` and the time scheme named.
fn cavity_solver(scheme: TimeScheme) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(20, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut set = settings(0.1);
    set.time_scheme = scheme;
    set.n_outer = 3;
    PimpleSolver::new(geom, u, p, set)
}

/// The fractional-step variants of ADR 0356 (FR, FR-F, FR-P, FR-C, FR-B).
#[derive(Debug, Clone, Copy)]
struct Fr {
    f: bool,
    incremental: bool,
    kim_moin: bool,
    scheme: TimeScheme,
}

const FR: Fr = Fr {
    f: false,
    incremental: true,
    kim_moin: true,
    scheme: TimeScheme::ImplicitEuler,
};

fn fractional_solver(o: Fr) -> PimpleSolver {
    let mut s = cavity_solver(o.scheme);
    s.settings.fractional = Some(FractionalSettings {
        incremental: o.incremental,
        kim_moin: o.kim_moin,
    });
    s.settings.fractional_step_consistent = o.f;
    s
}

/// The result of a run to `t = 20`.
struct Steady {
    s: PimpleSolver,
    change: f64,
    continuity: f64,
}

impl Steady {
    fn u(&self) -> &[Vec3] {
        &self.s.u.values
    }
    fn stopped(&self) -> bool {
        self.s.diverged.is_some()
    }
}

fn run_to_20(mut s: PimpleSolver, dt: f64) -> Steady {
    let steps = (20.0 / dt).round() as usize;
    let (mut change, mut continuity) = (0.0f64, 0.0f64);
    for k in 0..steps {
        let before = s.u.values.clone();
        let info = s.step(dt);
        continuity = continuity.max(info.continuity);
        if s.diverged.is_some() {
            break;
        }
        if k + 1 == steps {
            change = field_distance(&s.u.values, &before);
        }
    }
    Steady { s, change, continuity }
}

fn fr_run(o: Fr, dt: f64) -> Steady {
    let r = run_to_20(fractional_solver(o), dt);
    println!(
        "run {o:?} dt {dt}: change over the last step {:.3e}, continuity {:.3e}, stopped {}",
        r.change,
        r.continuity,
        r.stopped()
    );
    r
}

/// T1: the internal-face defect `d = phi - interp(u) . S` and `dte R(p)` of the final state; returns
/// (`max abs(d - dte R) / max abs(dte R)`, `max abs(d) / abs(S)`).
fn face_term(r: &Steady, dt: f64) -> (f64, f64) {
    let s = &r.s;
    let g = &s.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let sf = mesh.face_area_vectors();
    let zero_b = vec![[0.0; 3]; mesh.n_boundary_faces()];
    let fu = face_flux(g, &s.u.values, &zero_b);
    let pbc = pressure_bc(g, &s.u, &s.p);
    let p = &s.p.values;
    let gp = green_gauss(g, p, &boundary_values_pbc(g, &pbc, p), 0);
    let (mut worst, mut size, mut defect) = (0.0f64, 0.0f64, 0.0f64);
    for f in 0..g.n_internal {
        let w = g.weights[f];
        let gf = [0, 1, 2].map(|i| w * gp[own[f]][i] + (1.0 - w) * gp[nei[f]][i]);
        let term = dt * (vec3::dot(gf, sf[f]) - g.lap_coeff[f] * (p[nei[f]] - p[own[f]]));
        let d = s.phi[f] - fu[f];
        worst = worst.max((d - term).abs());
        size = size.max(term.abs());
        defect = defect.max(d.abs() / vec3::norm(sf[f]));
    }
    (worst / size, defect)
}

/// ADR 0356 S0, T1, A1, A1d, A2, X1, X1n and the printed P1 and FR-B.
#[test]
fn f454_steady_fractional_cavity_traced_and_made_step_independent() {
    let mut c = Checks::new();
    // S0, T1: the default fractional step
    let d4 = fr_run(FR, 0.04);
    let d1 = fr_run(FR, 0.01);
    c.within(
        "S0 default fractional step, steady cavity, 0.04 against 0.01",
        field_distance(d4.u(), d1.u()),
        8.6e-2,
        8.8e-2,
    );
    let (t4, s4) = face_term(&d4, 0.04);
    let (t1, s1) = face_term(&d1, 0.01);
    c.at_most("T1 face defect against dte R(p), dt 0.04 (relative)", t4, 1e-6);
    c.at_most("T1 face defect against dte R(p), dt 0.01 (relative)", t1, 1e-6);
    println!(
        "T1p printed: max |d| / |S| {s4:.4e} (dt 0.04), {s1:.4e} (dt 0.01), ratio {:.4}",
        s4 / s1
    );
    // A1, A1d: the opt-in
    let f = Fr { f: true, ..FR };
    let f4 = fr_run(f, 0.04);
    let f1 = fr_run(f, 0.01);
    c.holds(
        "A1 runs",
        !f4.stopped() && !f1.stopped(),
        format!("stopped {} {}", f4.stopped(), f1.stopped()),
    );
    c.at_most(
        "A1 fractional step with F, steady cavity, 0.04 against 0.01",
        field_distance(f4.u(), f1.u()),
        1e-9,
    );
    c.at_most("A1d continuity of the A1 runs", f4.continuity.max(f1.continuity), 1e-9);
    let (_, fs4) = face_term(&f4, 0.04);
    let (_, fs1) = face_term(&f1, 0.01);
    println!("A1 printed: max |d| / |S| with F {fs4:.4e} (dt 0.04), {fs1:.4e} (dt 0.01)");
    // A2: the plain boundary
    let fp = Fr { kim_moin: false, ..f };
    let p4 = fr_run(fp, 0.04);
    let p1 = fr_run(fp, 0.01);
    c.at_most(
        "A2 F with the plain boundary, 0.04 against 0.01",
        field_distance(p4.u(), p1.u()),
        1e-9,
    );
    // X1, X1n: against the PISO step with C (ADR 0245 C3)
    let mut piso = cavity_solver(TimeScheme::Bdf2);
    piso.settings.rhie_chow_step_consistent = true;
    let cavc = run_to_20(piso, 0.01);
    println!(
        "CAVC change over the last step {:.3e}, stopped {}",
        cavc.change,
        cavc.stopped()
    );
    c.at_most(
        "X1 F at 0.01 against the PISO step with C at 0.01",
        field_distance(f1.u(), cavc.u()),
        1e-9,
    );
    c.at_least(
        "X1n (negative control) the default fractional step at 0.01 against the PISO step with C",
        field_distance(d1.u(), cavc.u()),
        1e-6,
    );
    // P1 printed: the Chorin form
    let ch = Fr {
        incremental: false,
        ..FR
    };
    let c4 = fr_run(ch, 0.04);
    let c1 = fr_run(ch, 0.01);
    println!(
        "P1 printed: the Chorin form, 0.04 against 0.01: {:.4e}",
        field_distance(c4.u(), c1.u())
    );
    // FR-B printed: BDF2
    let b = Fr {
        scheme: TimeScheme::Bdf2,
        ..FR
    };
    let bf = Fr { f: true, ..b };
    let (b4, b1) = (fr_run(b, 0.04), fr_run(b, 0.01));
    let (bf4, bf1) = (fr_run(bf, 0.04), fr_run(bf, 0.01));
    println!(
        "FR-B printed: BDF2, 0.04 against 0.01: default {:.4e}, with F {:.4e}",
        field_distance(b4.u(), b1.u()),
        field_distance(bf4.u(), bf1.u())
    );
    c.finish();
}

fn state_bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    v.extend(s.p.values.iter().chain(&s.phi).map(|x| x.to_bits()));
    v
}

/// ADR 0356 D1, D2: the flag is not read in the Chorin form nor by the PISO step.
#[test]
fn d1_d2_the_flag_acts_only_in_the_incremental_fractional_step() {
    let mut c = Checks::new();
    let ch = Fr {
        incremental: false,
        ..FR
    };
    let (mut a, mut b) = (fractional_solver(Fr { f: true, ..ch }), fractional_solver(ch));
    let mut equal = true;
    for _ in 0..100 {
        a.step(0.04);
        b.step(0.04);
        equal &= state_bits(&a) == state_bits(&b);
    }
    c.holds(
        "D1 Chorin form with the flag against without, 100 steps of 0.04",
        equal,
        format!("bit for bit {equal}"),
    );
    let (mut a, mut b) = (cavity_solver(TimeScheme::Bdf2), cavity_solver(TimeScheme::Bdf2));
    a.settings.fractional_step_consistent = true;
    let mut equal = true;
    for _ in 0..50 {
        a.step(0.04);
        b.step(0.04);
        equal &= state_bits(&a) == state_bits(&b);
    }
    c.holds(
        "D2 PISO step with the flag against without, 50 steps of 0.04",
        equal,
        format!("bit for bit {equal}"),
    );
    c.finish();
}

// ---- D0: fingerprints of the defaults (this part compiles on main ce0db7d1 too) ---------------------------------------

fn fnv(bits: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bits {
        for byte in b.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// ADR 0356 D0: printed fingerprints of FR (default) after 100 steps and of CAV with BDF2 (PISO) after 50 steps of 0.04;
/// compared by hand against the same test on main.
#[test]
fn d0_default_fingerprints() {
    let mut fr = cavity_solver(TimeScheme::ImplicitEuler);
    fr.settings.fractional = Some(FractionalSettings::default());
    for _ in 0..100 {
        fr.step(0.04);
    }
    let mut cav = cavity_solver(TimeScheme::Bdf2);
    for _ in 0..50 {
        cav.step(0.04);
    }
    println!(
        "D0 fingerprints: fractional {:016x}, piso {:016x}",
        fnv(&state_bits(&fr)),
        fnv(&state_bits(&cav))
    );
}
