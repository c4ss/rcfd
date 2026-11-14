//! F-506 (ADR 0427): the sparse whirl solver (shift-invert skew Lanczos on the state form of ADR 0411 with a sparse factor of
//! K_eff, deflated restarts). Checks A2, E1 to E6, Q1, U1, M1 to M4, T1 to T3, R1 to R2 and the negative controls N1, N2 of
//! the registration. The large part (T1 to T3) is an ignored heavy record; the medium part covers the same code path.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::full_rotor;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    gyroscopic_eigen_dense, gyroscopic_eigen_sparse, gyroscopic_eigen_sparse_with, HarmonicWhirlSpeedModes,
    LanczosOptions, RotatingOptions, RotatingPart, RotatingSector, SpeedModes, WhirlModes, WhirlSolver,
    WhirlSpeedModes,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::time::Instant;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn lt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{id}: {value:.3e} (limit < {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} >= {limit:e}"));
        }
    }
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{id}: {value:.3e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const W0: f64 = 1000.0;
const H: f64 = 0.1;
const N: usize = 6;
const FACTORS: [f64; 5] = [0.1, 0.3, 0.5, 0.7, 0.9];

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn bits(r: &WhirlModes) -> Vec<u64> {
    let mut v = Vec::new();
    for m in &r.modes {
        v.push(m.omega.to_bits());
        v.push(m.whirl.to_bits());
        v.push(m.residual.to_bits());
        v.extend(m.re.iter().map(|x| x.to_bits()));
        v.extend(m.im.iter().map(|x| x.to_bits()));
    }
    v.extend(r.frequencies_all.iter().map(|x| x.to_bits()));
    v
}

fn whirl_of(r: WhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        WhirlSpeedModes::Modes(m) => m,
        WhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

fn harmonic_whirl(r: HarmonicWhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

/// `max |omega_s / omega_d - 1|` mode by mode.
fn freq_dev(s: &WhirlModes, d: &WhirlModes) -> f64 {
    assert_eq!(s.modes.len(), d.modes.len(), "mode counts");
    s.modes
        .iter()
        .zip(&d.modes)
        .map(|(a, b)| (a.omega / b.omega - 1.0).abs())
        .fold(0.0f64, f64::max)
}

/// `max |kappa_s - kappa_d|` over the modes whose relative gap to their neighbours in `spectrum` exceeds 1e-6, and the
/// number of excluded modes.
fn kappa_dev(s: &[(f64, f64)], d: &[(f64, f64)], spectrum: &[f64]) -> (f64, usize) {
    let mut worst: f64 = 0.0;
    let mut excluded = 0;
    for (a, b) in s.iter().zip(d) {
        let w = b.0;
        let gap = spectrum
            .iter()
            .filter(|&&x| x != w)
            .map(|&x| (x - w).abs() / w)
            .fold(f64::INFINITY, f64::min);
        let dup = spectrum.iter().filter(|&&x| x == w).count() > 1;
        if gap > 1e-6 && !dup {
            worst = worst.max((a.1 - b.1).abs());
        } else {
            excluded += 1;
        }
    }
    (worst, excluded)
}

fn pairs(r: &WhirlModes) -> Vec<(f64, f64)> {
    r.modes.iter().map(|m| (m.omega, m.whirl)).collect()
}

fn max_residual(r: &WhirlModes) -> f64 {
    r.modes.iter().map(|m| m.residual).fold(0.0f64, f64::max)
}

// ------------------------------------------------------------------------------------------------------ algebraic

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
}

fn csr(a: &[f64], n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if a[i * n + j] != 0.0 {
                t.push((i, j, a[i * n + j]));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

/// Random `K`, `M` symmetric positive definite and `G` skew, dense row-major.
fn random_problem(n: usize, seed: u64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut s = seed;
    let mut spd = |scale: f64| {
        let b: Vec<f64> = (0..n * n).map(|_| lcg(&mut s)).collect();
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut v = 0.0;
                for p in 0..n {
                    v += b[i * n + p] * b[j * n + p];
                }
                a[i * n + j] = scale * (v + if i == j { 0.5 * n as f64 } else { 0.0 });
            }
        }
        a
    };
    let k = spd(1.0e6);
    let m = spd(1.0);
    let mut s2 = seed ^ 0x9e37;
    let mut g = vec![0.0; n * n];
    for i in 0..n {
        for j in i + 1..n {
            let v = 300.0 * lcg(&mut s2);
            g[i * n + j] = v;
            g[j * n + i] = -v;
        }
    }
    (k, m, g)
}

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn u1_r1_n1_algebraic() {
    let mut ch = Checks::new();
    let mut u1: f64 = 0.0;
    let mut q1: f64 = 0.0;
    for (n, count) in [(1, 1), (2, 2), (5, 5), (12, 12), (30, 8)] {
        let (k, m, g) = random_problem(n, 17 + n as u64);
        let d = gyroscopic_eigen_dense(&k, &m, &g, n).unwrap();
        let s = gyroscopic_eigen_sparse(&csr(&k, n), &csr(&m, n), &csr(&g, n), count).unwrap();
        let dev = s
            .omegas
            .iter()
            .zip(&d.omegas)
            .map(|(a, b)| (a / b - 1.0).abs())
            .fold(0.0f64, f64::max);
        println!(
            "U1 n = {n}: {} runs, {} steps, deviation {dev:.2e}, residuals {:?}",
            s.runs, s.steps, s.residuals
        );
        u1 = u1.max(dev);
        q1 = s.residuals.iter().fold(q1, |a, &x| a.max(x));
    }
    ch.le("U1 sparse against dense on random matrices", u1, 1e-9);
    ch.le("Q1 random matrices residual", q1, 1e-9);
    // R1.
    let (k, m, g) = random_problem(5, 3);
    let (kc, mc, gc) = (csr(&k, 5), csr(&m, 5), csr(&g, 5));
    let (k6, _, _) = random_problem(6, 3);
    let k6 = csr(&k6, 6);
    ch.holds(
        "R1 size",
        key_of(gyroscopic_eigen_sparse(&kc, &k6, &gc, 2).unwrap_err()) == "matrix",
        "refused with \"matrix\"",
    );
    ch.holds(
        "R1 count 0",
        key_of(gyroscopic_eigen_sparse(&kc, &mc, &gc, 0).unwrap_err()) == "modes",
        "refused with \"modes\"",
    );
    ch.holds(
        "R1 count n + 1",
        key_of(gyroscopic_eigen_sparse(&kc, &mc, &gc, 6).unwrap_err()) == "modes",
        "refused with \"modes\"",
    );
    let mut kneg = k.clone();
    kneg[0] = -kneg[0];
    ch.holds(
        "R1 indefinite K",
        key_of(gyroscopic_eigen_sparse(&csr(&kneg, 5), &mc, &gc, 2).unwrap_err()) == "stiffness",
        "refused with \"stiffness\"",
    );
    let mut mneg = m.clone();
    mneg[0] = -mneg[0];
    ch.holds(
        "R1 indefinite M",
        key_of(gyroscopic_eigen_sparse(&kc, &csr(&mneg, 5), &gc, 2).unwrap_err()) == "mass",
        "refused with \"mass\"",
    );
    // N1: one run on exactly double frequencies.
    let n = 20;
    let mut kd = vec![0.0; n * n];
    let mut md = vec![0.0; n * n];
    for i in 0..n {
        let j = (i / 2 + 1) as f64;
        kd[i * n + i] = j * j;
        md[i * n + i] = 1.0;
    }
    let gd = vec![0.0; n * n];
    let dense = gyroscopic_eigen_dense(&kd, &md, &gd, n).unwrap();
    let one = gyroscopic_eigen_sparse_with(
        &csr(&kd, n),
        &csr(&md, n),
        &csr(&gd, n),
        6,
        &LanczosOptions::default().max_runs(1),
    )
    .unwrap();
    println!("N1: one run {:?}, dense {:?}", one.omegas, &dense.omegas[..6]);
    ch.gt(
        "N1 one run against dense, second frequency",
        (one.omegas[1] / dense.omegas[1] - 1.0).abs(),
        0.1,
    );
    let full = gyroscopic_eigen_sparse(&csr(&kd, n), &csr(&md, n), &csr(&gd, n), 6).unwrap();
    let dev = full
        .omegas
        .iter()
        .zip(&dense.omegas)
        .map(|(a, b)| (a / b - 1.0).abs())
        .fold(0.0f64, f64::max);
    println!("N1 companion: with the restarts {:?} ({} runs), deviation {dev:.2e}", full.omegas, full.runs);
    ch.le("U1 double frequencies with the restarts", dev, 1e-9);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ CB: the cube

fn cube() -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let kn = RHO * H * H * H * W0 * W0 / 8.0;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, kn)).unwrap();
    m
}

#[test]
fn e1_e2_n2_r2_cube() {
    let mut ch = Checks::new();
    let cb = cube();
    let opts = RotatingOptions::new(3).coriolis(true);
    let dense = RotatingPart::new(&cb, AXIS, ORIGIN, &opts).unwrap();
    let sparse = RotatingPart::new(&cb, AXIS, ORIGIN, &opts.clone().whirl_solver(WhirlSolver::Sparse)).unwrap();
    let (mut e1, mut e2, mut q1): (f64, f64, f64) = (0.0, 0.0, 0.0);
    let mut excluded = 0;
    for f in FACTORS {
        let w = f * W0;
        let d = whirl_of(dense.whirl_modes(w).unwrap(), "E1 dense");
        let s = whirl_of(sparse.whirl_modes(w).unwrap(), "E1 sparse");
        println!(
            "CB {f}: sparse {:?} kappa {:?}; dense {:?} kappa {:?}",
            s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            s.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            d.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            d.modes.iter().map(|x| x.whirl).collect::<Vec<_>>()
        );
        e1 = e1.max(freq_dev(&s, &d));
        let (k, x) = kappa_dev(&pairs(&s), &pairs(&d), &d.frequencies_all);
        e2 = e2.max(k);
        excluded += x;
        q1 = q1.max(max_residual(&s));
    }
    ch.le("E1 cube frequencies sparse against dense", e1, 1e-9);
    println!("E2 cube: {excluded} modes excluded (gap <= 1e-6)");
    ch.le("E2 cube whirl numbers sparse against dense", e2, 1e-8);
    ch.le("Q1 cube sparse residual", q1, 1e-9);
    // N2: the sparse solver with Coriolis off against the dense one with it.
    let off = RotatingPart::new(
        &cb,
        AXIS,
        ORIGIN,
        &RotatingOptions::new(3).coriolis(false).whirl_solver(WhirlSolver::Sparse),
    )
    .unwrap();
    let w = 0.5 * W0;
    let s_off = whirl_of(off.whirl_modes(w).unwrap(), "N2");
    let d_on = whirl_of(dense.whirl_modes(w).unwrap(), "N2");
    ch.gt("N2 cube sparse off against dense on", freq_dev(&s_off, &d_on), 1e-2);
    // R2.
    ch.holds(
        "R2 default",
        RotatingOptions::new(3).whirl_solver == WhirlSolver::Dense,
        "RotatingOptions::new(m).whirl_solver == Dense",
    );
    let explicit = RotatingPart::new(&cb, AXIS, ORIGIN, &opts.clone().whirl_solver(WhirlSolver::Dense)).unwrap();
    let auto = RotatingPart::new(&cb, AXIS, ORIGIN, &opts.clone().whirl_solver(WhirlSolver::Auto)).unwrap();
    let a = bits(&whirl_of(dense.whirl_modes(w).unwrap(), "R2"));
    let b = bits(&whirl_of(explicit.whirl_modes(w).unwrap(), "R2"));
    let c = bits(&whirl_of(auto.whirl_modes(w).unwrap(), "R2"));
    ch.holds("R2 cube Dense", a == b, "the default part and whirl_solver(Dense) give the same bits");
    ch.holds("R2 cube Auto", a == c, "Auto below the cap gives the bits of Dense");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ DS: the sector

fn sector_mesh(sectors: usize, div: [usize; 3]) -> SolidMesh {
    let alpha = 2.0 * PI / sectors as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], div).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn model_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn sector_model(sectors: usize, div: [usize; 3]) -> SolidModel {
    let mut m = model_on(sector_mesh(sectors, div));
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, sectors))
        .unwrap();
    m
}

fn full_of(sector: &SolidModel) -> SolidModel {
    let rotor = full_rotor(sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    full
}

fn modal(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn ds_options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(4).modal(modal(4)).coriolis(coriolis)
}

fn omega_ref(rs: &RotatingSector) -> f64 {
    (0..=N / 2)
        .map(|k| harmonic_whirl(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn e3_e4_e5_e6_r2_sector() {
    let mut ch = Checks::new();
    let sector = sector_model(N, [2, 1, 1]);
    let full = full_of(&sector);
    let rd = RotatingSector::new(&sector, &ds_options(true)).unwrap();
    let rs = RotatingSector::new(&sector, &ds_options(true).whirl_solver(WhirlSolver::Sparse)).unwrap();
    let w_ref = omega_ref(&rd);
    println!("omega_ref = {w_ref:.9e} rad/s");
    let fd = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(true)).unwrap();
    let fs = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(true).whirl_solver(WhirlSolver::Sparse)).unwrap();
    let (mut e3, mut e4, mut e5f, mut e5k, mut q1): (f64, f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut x4, mut x5) = (0, 0);
    for factor in [0.4, 0.8] {
        let w = factor * w_ref;
        for k in 0..=N / 2 {
            let d = harmonic_whirl(rd.whirl_modes(k, w).unwrap(), "E3 dense");
            let s = harmonic_whirl(rs.whirl_modes(k, w).unwrap(), "E3 sparse");
            println!(
                "DS {factor} k = {k}: sparse {:?} kappa {:?}; dense {:?} kappa {:?}",
                s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
                s.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
                d.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
                d.modes.iter().map(|x| x.whirl).collect::<Vec<_>>()
            );
            e3 = e3.max(freq_dev(&s, &d));
            let (kd, x) = kappa_dev(&pairs(&s), &pairs(&d), &d.frequencies_all);
            e4 = e4.max(kd);
            x4 += x;
            q1 = q1.max(max_residual(&s));
        }
        let d = whirl_of(fd.whirl_modes(w).unwrap(), "E5 dense");
        let s = whirl_of(fs.whirl_modes(w).unwrap(), "E5 sparse");
        println!(
            "DF {factor}: sparse {:?} kappa {:?}; dense {:?} kappa {:?}",
            s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            s.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            d.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            d.modes.iter().map(|x| x.whirl).collect::<Vec<_>>()
        );
        e5f = e5f.max(freq_dev(&s, &d));
        let (kd, x) = kappa_dev(&pairs(&s), &pairs(&d), &d.frequencies_all);
        e5k = e5k.max(kd);
        x5 += x;
        q1 = q1.max(max_residual(&s));
    }
    ch.le("E3 sector frequencies sparse against dense", e3, 1e-9);
    println!("E4 sector: {x4} modes excluded (gap <= 1e-6)");
    ch.le("E4 sector whirl numbers sparse against dense", e4, 1e-8);
    ch.le("E5 full rotor frequencies sparse against dense", e5f, 1e-9);
    println!("E5 full rotor: {x5} modes excluded (gap <= 1e-6)");
    ch.le("E5 full rotor whirl numbers sparse against dense", e5k, 1e-8);
    // E6: Coriolis off, every pair frequency double.
    let w = 0.4 * w_ref;
    let rd_off = RotatingSector::new(&sector, &ds_options(false)).unwrap();
    let rs_off = RotatingSector::new(&sector, &ds_options(false).whirl_solver(WhirlSolver::Sparse)).unwrap();
    let d = harmonic_whirl(rd_off.whirl_modes(1, w).unwrap(), "E6");
    let s = harmonic_whirl(rs_off.whirl_modes(1, w).unwrap(), "E6");
    println!(
        "E6 DS k = 1 off: sparse {:?}; dense {:?}",
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
        d.modes.iter().map(|x| x.omega).collect::<Vec<_>>()
    );
    let e6s = freq_dev(&s, &d);
    q1 = q1.max(max_residual(&s));
    let fd_off = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(false)).unwrap();
    let fs_off = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(false).whirl_solver(WhirlSolver::Sparse)).unwrap();
    let d = whirl_of(fd_off.whirl_modes(w).unwrap(), "E6 full");
    let s = whirl_of(fs_off.whirl_modes(w).unwrap(), "E6 full");
    println!(
        "E6 DF off: sparse {:?}; dense {:?}",
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
        d.modes.iter().map(|x| x.omega).collect::<Vec<_>>()
    );
    let e6f = freq_dev(&s, &d);
    q1 = q1.max(max_residual(&s));
    ch.le("E6 sector index 1 Coriolis off sparse against dense", e6s, 1e-9);
    ch.le("E6 full rotor Coriolis off sparse against dense", e6f, 1e-9);
    ch.le("Q1 sector and full rotor sparse residual", q1, 1e-9);
    // R2 on the sector.
    let explicit = RotatingSector::new(&sector, &ds_options(true).whirl_solver(WhirlSolver::Dense)).unwrap();
    let auto = RotatingSector::new(&sector, &ds_options(true).whirl_solver(WhirlSolver::Auto)).unwrap();
    let a = bits(&harmonic_whirl(rd.whirl_modes(1, w).unwrap(), "R2"));
    let b = bits(&harmonic_whirl(explicit.whirl_modes(1, w).unwrap(), "R2"));
    let c = bits(&harmonic_whirl(auto.whirl_modes(1, w).unwrap(), "R2"));
    ch.holds("R2 sector Dense", a == b, "the default sector and whirl_solver(Dense) give the same bits");
    ch.holds("R2 sector Auto", a == c, "Auto below the cap gives the bits of Dense");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ MP and LP

const RING_DIV: [usize; 3] = [4, 1, 2];
const RING_MODES: usize = 10;

fn ring_options(coriolis: bool, solver: WhirlSolver) -> RotatingOptions {
    RotatingOptions::new(RING_MODES)
        .modal(modal(RING_MODES))
        .coriolis(coriolis)
        .whirl_solver(solver)
}

/// The lowest `RING_MODES` of the union over every index of the dense sector solutions (4 modes per index) as `(omega,
/// kappa)`, the full sorted union, and whether the union is complete for them (every index's highest returned frequency at or
/// above the union's `RING_MODES`-th).
fn sector_union(sector: &SolidModel, sectors: usize, w: f64) -> (Vec<(f64, f64)>, Vec<f64>, bool) {
    let rd = RotatingSector::new(sector, &ds_options(true)).unwrap();
    let mut all: Vec<(f64, f64)> = Vec::new();
    let mut highest = Vec::new();
    for k in 0..=sectors / 2 {
        let h = harmonic_whirl(rd.whirl_modes(k, w).unwrap(), "sector union");
        highest.push(h.modes.last().unwrap().omega);
        all.extend(h.modes.iter().map(|m| (m.omega, m.whirl)));
    }
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let tenth = all[RING_MODES - 1].0;
    let valid = highest.iter().all(|&h| h >= tenth);
    let spectrum: Vec<f64> = all.iter().map(|p| p.0).collect();
    all.truncate(RING_MODES);
    (all, spectrum, valid)
}

fn subspace_dev(s: &WhirlModes, part: &RotatingPart, w: f64) -> f64 {
    let sub = match part.modes(w).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("defective set-up: {l:?}"),
    };
    s.modes
        .iter()
        .zip(&sub.eigenvalues)
        .map(|(x, l)| (x.omega / l.sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max)
}

fn union_devs(s: &WhirlModes, union: &[(f64, f64)], spectrum: &[f64]) -> (f64, f64, usize) {
    let f = s
        .modes
        .iter()
        .zip(union)
        .map(|(a, b)| (a.omega / b.0 - 1.0).abs())
        .fold(0.0f64, f64::max);
    let (k, x) = kappa_dev(&pairs(s), union, spectrum);
    (f, k, x)
}

#[test]
fn a2_m1_m2_m3_m4_medium_part() {
    let mut ch = Checks::new();
    let sectors = 16;
    let sector = sector_model(sectors, RING_DIV);
    let full = full_of(&sector);
    let dense = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(true, WhirlSolver::Dense)).unwrap();
    let rest = whirl_of(dense.whirl_modes(0.0).unwrap(), "MP rest");
    let w = 0.4 * rest.modes[0].omega;
    println!("MP: {} reduced unknowns, omega_mp = {:.9e}, speed {w:.6e}", rest.unknowns, rest.modes[0].omega);
    let refused = dense.whirl_modes(w).map(|_| ()).unwrap_err();
    let sparse = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(true, WhirlSolver::Sparse)).unwrap();
    let t = Instant::now();
    let s = whirl_of(sparse.whirl_modes(w).unwrap(), "MP sparse");
    println!("MP sparse solve {:.2} s", t.elapsed().as_secs_f64());
    println!(
        "MP sparse: {:?} kappa {:?} residuals {:?}",
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
        s.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
        s.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
    );
    ch.holds(
        "A2 MP",
        rest.unknowns > 1500 && key_of(refused) == "unknowns" && s.modes.len() == RING_MODES,
        "above the cap: Dense refused with \"unknowns\", Sparse returns 10 modes",
    );
    let auto = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(true, WhirlSolver::Auto)).unwrap();
    let a = whirl_of(auto.whirl_modes(w).unwrap(), "M1");
    ch.holds("M1 MP Auto", bits(&a) == bits(&s), "Auto above the cap gives the bits of Sparse");
    let (union, spectrum, valid) = sector_union(&sector, sectors, w);
    ch.holds("M2 validity", valid, "every index's highest returned frequency at or above the union's 10th");
    let (f, k, x) = union_devs(&s, &union, &spectrum);
    ch.le("M2 MP frequencies against the sector union", f, 1e-8);
    println!("M3 MP: {x} modes excluded (gap <= 1e-6)");
    ch.le("M3 MP whirl numbers against the sector union", k, 1e-6);
    let off = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(false, WhirlSolver::Sparse)).unwrap();
    let so = whirl_of(off.whirl_modes(w).unwrap(), "M4");
    println!("M4 MP off: {:?}", so.modes.iter().map(|x| x.omega).collect::<Vec<_>>());
    ch.le("M4 MP Coriolis off against the subspace modes", subspace_dev(&so, &off, w), 1e-8);
    ch.le("Q1 MP residual", max_residual(&s).max(max_residual(&so)), 1e-9);
    ch.finish();
}

#[test]
#[ignore = "heavy record (ADR 0427 T1 to T3): a ring of 20 064 reduced unknowns, run once in release"]
fn t1_t2_t3_large_part() {
    let mut ch = Checks::new();
    let sectors = 152;
    let sector = sector_model(sectors, RING_DIV);
    let full = full_of(&sector);
    // The rest frequency (outside the timed span) on a part of its own.
    let probe = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(true, WhirlSolver::Sparse)).unwrap();
    let rest = whirl_of(probe.whirl_modes(0.0).unwrap(), "LP rest");
    drop(probe);
    let w = 0.4 * rest.modes[0].omega;
    println!("LP: {} reduced unknowns, omega_lp = {:.9e}, speed {w:.6e}", rest.unknowns, rest.modes[0].omega);
    let t = Instant::now();
    let part = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(true, WhirlSolver::Sparse)).unwrap();
    let t_new = t.elapsed().as_secs_f64();
    let s = whirl_of(part.whirl_modes(w).unwrap(), "LP sparse");
    let total = t.elapsed().as_secs_f64();
    println!("T1 LP: new {t_new:.2} s, whirl_modes {:.2} s, total {total:.2} s", total - t_new);
    println!(
        "LP sparse: {:?} kappa {:?} residuals {:?}",
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
        s.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
        s.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
    );
    ch.holds("T1 LP unknowns", s.unknowns >= 20_000, &format!("{} >= 20 000", s.unknowns));
    ch.lt("T1 LP wall time of new + whirl_modes, s", total, 120.0);
    drop(part);
    ch.le("Q1 LP residual", max_residual(&s), 1e-9);
    let (union, spectrum, valid) = sector_union(&sector, sectors, w);
    ch.holds("T2 validity", valid, "every index's highest returned frequency at or above the union's 10th");
    let (f, k, x) = union_devs(&s, &union, &spectrum);
    ch.le("T2 LP frequencies against the sector union", f, 1e-8);
    println!("T2 LP whirl numbers against the sector union (recorded): {k:.3e}, {x} excluded");
    let off = RotatingPart::new(&full, AXIS, ORIGIN, &ring_options(false, WhirlSolver::Sparse)).unwrap();
    let so = whirl_of(off.whirl_modes(w).unwrap(), "T3");
    println!("T3 LP off: {:?}", so.modes.iter().map(|x| x.omega).collect::<Vec<_>>());
    ch.le("T3 LP Coriolis off against the subspace modes", subspace_dev(&so, &off, w), 1e-8);
    ch.le("Q1 LP Coriolis off residual", max_residual(&so), 1e-9);
    ch.finish();
}
