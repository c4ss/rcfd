//! F-499 (ADR 0411): Coriolis coupling of a rotating part: the gyroscopic matrix, whirl modes by a symmetric linearisation,
//! forward and backward whirl in the Campbell table, on the whole part and on a cyclic sector. Checks G1 to R2 and the
//! negative controls N1 to N3 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic, FullRotor, SectorMode};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::rotating::{
    coriolis_matrix, gyroscopic_eigen_dense, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector,
    SpeedModes, Whirl, WhirlCampbell, WhirlMode, WhirlModes, WhirlSpeedModes,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

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

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
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

fn cube_options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(3).coriolis(coriolis)
}

fn uniform(model: &SolidModel, c: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut v = vec![0.0; dofs.n_equations()];
    for node in 0..model.mesh().nodes().len() {
        v[dofs.equation(node, c).unwrap()] = 1.0;
    }
    v
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

fn nrm(x: &[f64]) -> f64 {
    dot(x, x).sqrt()
}

/// The complex share of a mode in `span{1_x, 1_y}` (M-orthogonal, M diagonal in the components).
fn translation_share(model: &SolidModel, m: &CsrMatrix, w: &WhirlMode) -> f64 {
    let mut s = 0.0;
    for c in 0..2 {
        let u = uniform(model, c);
        let mu = matvec(m, &u);
        let (a, b) = (dot(&mu, &w.re), dot(&mu, &w.im));
        s += (a * a + b * b) / dot(&u, &mu);
    }
    s / (dot(&w.re, &matvec(m, &w.re)) + dot(&w.im, &matvec(m, &w.im)))
}

fn whirl_of(r: WhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        WhirlSpeedModes::Modes(m) => m,
        WhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

/// The translation pair of a cube result (share >= 1 - 1e-8), lower frequency first.
fn pair_of<'m>(model: &SolidModel, m: &CsrMatrix, r: &'m WhirlModes) -> Vec<&'m WhirlMode> {
    let mut v: Vec<&WhirlMode> = r
        .modes
        .iter()
        .filter(|w| translation_share(model, m, w) >= 1.0 - 1e-8)
        .collect();
    v.sort_by(|a, b| a.omega.total_cmp(&b.omega));
    v
}

// ------------------------------------------------------------------------------------------------------ G1, G2

fn skew_defect(c: &CsrMatrix) -> f64 {
    let mut worst: f64 = 0.0;
    let mut big: f64 = 0.0;
    for r in 0..c.n {
        for p in c.row_ptr[r]..c.row_ptr[r + 1] {
            let col = c.col[p];
            big = big.max(c.val[p].abs());
            let q = c.row_ptr[col] + c.col[c.row_ptr[col]..c.row_ptr[col + 1]].binary_search(&r).unwrap();
            worst = worst.max((c.val[p] + c.val[q]).abs());
        }
    }
    worst / big
}

#[test]
fn g1_g2_coriolis_matrix_is_skew() {
    let mut ch = Checks::new();
    let cb = cube();
    let c = coriolis_matrix(&cb, AXIS, None).unwrap();
    ch.le("G1 cube", skew_defect(&c), 1e-14);
    let sector = ds_sector();
    ch.le(
        "G1 sector",
        skew_defect(&coriolis_matrix(&sector, AXIS, None).unwrap()),
        1e-14,
    );
    let (_, full) = full_of(&sector);
    ch.le(
        "G1 full rotor",
        skew_defect(&coriolis_matrix(&full, AXIS, None).unwrap()),
        1e-14,
    );
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &cube_options(true)).unwrap();
    let m = part.mass();
    let (ux, uy) = (uniform(&cb, 0), uniform(&cb, 1));
    let (cx, cy) = (matvec(&c, &ux), matvec(&c, &uy));
    let (mx, my) = (matvec(m, &ux), matvec(m, &uy));
    let d1: Vec<f64> = cx.iter().zip(&my).map(|(a, b)| a - b).collect();
    let d2: Vec<f64> = cy.iter().zip(&mx).map(|(a, b)| a + b).collect();
    ch.le("G2 C 1_x = M 1_y", nrm(&d1) / nrm(&my), 1e-14);
    ch.le("G2 C 1_y = -M 1_x", nrm(&d2) / nrm(&mx), 1e-14);
    ch.holds(
        "G1 part",
        part.coriolis().unwrap() == c,
        "RotatingPart::coriolis equals coriolis_matrix",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ P1, P2: PM

#[test]
fn p1_p2_point_mass_closed_form() {
    let mut ch = Checks::new();
    let m = 7.85;
    let k = m * W0 * W0;
    let (mut p1, mut q1): (f64, f64) = (0.0, 0.0);
    let mut dir_ok = true;
    for f in FACTORS {
        let w = f * W0;
        let ke = vec![k - m * w * w, 0.0, 0.0, k - m * w * w];
        let mm = vec![m, 0.0, 0.0, m];
        let c = [0.0, -m, m, 0.0];
        let g: Vec<f64> = c.iter().map(|v| 2.0 * w * v).collect();
        let r = gyroscopic_eigen_dense(&ke, &mm, &g, 2).unwrap();
        assert_eq!(r.omegas.len(), 2);
        let kappa = |i: usize| {
            let (re, im) = (&r.re[i], &r.im[i]);
            let cs = [c[0] * im[0] + c[1] * im[1], c[2] * im[0] + c[3] * im[1]];
            2.0 * (re[0] * cs[0] + re[1] * cs[1])
                / (m * (re[0] * re[0] + re[1] * re[1] + im[0] * im[0] + im[1] * im[1]))
        };
        let (kf, kb) = (kappa(0), kappa(1));
        let ef = (r.omegas[0] - (W0 - w)).abs() / W0;
        let eb = (r.omegas[1] - (W0 + w)).abs() / W0;
        println!(
            "PM {f} omega_0: omega = {:?}, deviations {ef:.2e} {eb:.2e}, kappa {kf:.15} {kb:.15}, residuals {:?}",
            r.omegas, r.residuals
        );
        p1 = p1.max(ef).max(eb);
        q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
        dir_ok &= kf >= 1.0 - 1e-8
            && kb <= -1.0 + 1e-8
            && Whirl::of(kf) == Whirl::Forward
            && Whirl::of(kb) == Whirl::Backward;
    }
    ch.le("P1 point mass", p1, 1e-8);
    ch.le("Q1 point mass residual", q1, 1e-9);
    ch.holds(
        "P2 point mass",
        dir_ok,
        "omega_0 - Omega forward (kappa = 1), omega_0 + Omega backward (kappa = -1)",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ CB single speeds

#[test]
fn p1b_p2_q_z_l_n1_r2_cube_on_springs() {
    let mut ch = Checks::new();
    let cb = cube();
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &cube_options(true)).unwrap();
    let plain = RotatingPart::new(&cb, AXIS, ORIGIN, &cube_options(false)).unwrap();
    let m = part.mass();
    let (mut p1b, mut q1): (f64, f64) = (0.0, 0.0);
    let mut dir_ok = true;
    let mut q3 = true;
    for f in FACTORS {
        let w = f * W0;
        let r = whirl_of(part.whirl_modes(w).unwrap(), "P1b");
        let pair = pair_of(&cb, m, &r);
        assert_eq!(
            pair.len(),
            2,
            "defective set-up: the translation pair is not among the modes at {f} omega_0"
        );
        let ef = (pair[0].omega - (W0 - w)).abs() / W0;
        let eb = (pair[1].omega - (W0 + w)).abs() / W0;
        println!(
            "CB {f} omega_0: omega = {:?}, kappa = {:?}, pair deviations {ef:.2e} {eb:.2e}, residuals {:?}",
            r.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            r.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            r.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
        );
        p1b = p1b.max(ef).max(eb);
        q1 = r.modes.iter().fold(q1, |a, x| a.max(x.residual));
        dir_ok &= pair[0].whirl >= 1.0 - 1e-8
            && pair[0].direction == Whirl::Forward
            && pair[1].whirl <= -1.0 + 1e-8
            && pair[1].direction == Whirl::Backward;
        q3 &= r.frequencies_all.len() == r.unknowns && r.frequencies_all.iter().all(|x| x.is_finite());
    }
    ch.le("P1b cube translation pair", p1b, 1e-8);
    ch.holds(
        "P2 cube",
        dir_ok,
        "omega_0 - Omega forward, omega_0 + Omega backward, |kappa| = 1",
    );
    ch.le("Q1 cube residual", q1, 1e-9);
    ch.holds("Q3 cube", q3, "n positive finite frequencies");
    // Q2: Coriolis off against the subspace solution of F-396.
    let w = 0.5 * W0;
    let off = whirl_of(plain.whirl_modes(w).unwrap(), "Q2");
    let sub = match plain.modes(w).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("defective set-up: {l:?}"),
    };
    let q2 = off
        .modes
        .iter()
        .zip(&sub.eigenvalues)
        .map(|(x, l)| (x.omega / l.sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.le("Q2 cube Coriolis off against modes", q2, 1e-9);
    // N1: Coriolis off misses the closed form.
    let pair = pair_of(&cb, m, &off);
    let n1 = pair
        .iter()
        .zip([W0 - w, W0 + w])
        .map(|(x, e)| (x.omega - e).abs() / W0)
        .fold(0.0f64, f64::max);
    ch.gt("N1 Coriolis off against omega_0 -/+ Omega", n1, 1e-2);
    // Z1: the bits at rest.
    let z = whirl_of(part.whirl_modes(0.0).unwrap(), "Z1");
    let rest = match part.modes(0.0).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    let ma = modal_analysis(&cb, &cube_options(true).modal).unwrap();
    let zf: Vec<f64> = z.modes.iter().map(|x| x.frequency).collect();
    let zl: Vec<f64> = z.modes.iter().map(|x| x.eigenvalue).collect();
    ch.holds(
        "Z1 cube",
        same_bits(&zf, &rest.frequencies)
            && same_bits(&zl, &rest.eigenvalues)
            && same_bits(&zf, &ma.frequencies)
            && same_bits(&zl, &ma.eigenvalues),
        "whirl_modes(0) has the bits of modes(0) and of modal_analysis",
    );
    // Z2: continuity at a small speed.
    let small = whirl_of(part.whirl_modes(1e-6 * W0).unwrap(), "Z2");
    let z2 = small
        .modes
        .iter()
        .zip(&z.modes)
        .map(|(a, b)| (a.omega / b.omega - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.le("Z2 cube at 1e-6 omega_0", z2, 2e-6);
    // L1: a loss of stiffness above omega_0.
    let l1 = part.whirl_modes(1.2 * W0).unwrap();
    println!("L1: {l1:?}");
    ch.holds(
        "L1",
        matches!(l1, WhirlSpeedModes::LossOfStiffness(l) if l.negative == Some(2) && l.speed == 1.2 * W0),
        "LossOfStiffness with negative == Some(2) at 1.2 omega_0",
    );
    // R2: the default and the F-396 functions untouched by the option.
    ch.holds(
        "R2 default",
        !RotatingOptions::new(3).coriolis,
        "RotatingOptions::new(m).coriolis == false",
    );
    let a = match part.modes(w).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    let speeds = [0.0, 0.25 * W0, 0.5 * W0];
    let ca = part.campbell(&speeds).unwrap();
    let cb2 = plain.campbell(&speeds).unwrap();
    ch.holds(
        "R2 modes and campbell",
        same_bits(&a.eigenvalues, &sub.eigenvalues)
            && a.modes.iter().zip(&sub.modes).all(|(x, y)| same_bits(x, y))
            && ca == cb2,
        "modes and campbell with coriolis(true) have the bits of the default part",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ P3, P4, P5

fn campbell_speeds() -> Vec<f64> {
    (0..13).map(|j| 0.07 * j as f64 * W0).collect()
}

#[test]
fn p3_p4_p5_cube_whirl_campbell() {
    let mut ch = Checks::new();
    let cb = cube();
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &cube_options(true)).unwrap();
    let speeds = campbell_speeds();
    let t: WhirlCampbell = part.whirl_campbell(&speeds).unwrap();
    let tracks = &t.table.tracks;
    ch.holds("P3 tracks", tracks.len() == 3, &format!("{} tracks", tracks.len()));
    let (mut dev, mut mac_min, mut q1): (f64, f64, f64) = (0.0, 1.0, 0.0);
    let mut dirs = true;
    for (j, &w) in speeds.iter().enumerate() {
        let om0 = t.omegas(0)[j].unwrap();
        let om1 = t.omegas(1)[j].unwrap();
        println!(
            "P3 {:.2} omega_0: {:.9e} {:.9e} {:.9e} rad/s, dirs {:?} {:?} {:?}, mac {:?} {:?} {:?}",
            w / W0,
            om0,
            om1,
            t.omegas(2)[j].unwrap(),
            t.directions[0][j],
            t.directions[1][j],
            t.directions[2][j],
            tracks[0].mac[j],
            tracks[1].mac[j],
            tracks[2].mac[j]
        );
        if j == 0 {
            dirs &= t.directions[0][0].is_none() && t.directions[1][0].is_none();
            continue;
        }
        dev = dev.max((om0 - (W0 - w)).abs() / W0).max((om1 - (W0 + w)).abs() / W0);
        dirs &= t.directions[0][j] == Some(Whirl::Forward) && t.directions[1][j] == Some(Whirl::Backward);
        mac_min = mac_min.min(tracks[0].mac[j].unwrap()).min(tracks[1].mac[j].unwrap());
    }
    for &w in &speeds[1..] {
        let r = whirl_of(part.whirl_modes(w).unwrap(), "Q1");
        q1 = r.modes.iter().fold(q1, |a, x| a.max(x.residual));
    }
    ch.le("P3 branches against omega_0 -/+ Omega", dev, 1e-8);
    ch.holds(
        "P3 directions",
        dirs,
        "track 0 forward, track 1 backward at every speed > 0, None at rest",
    );
    ch.holds(
        "P3 MAC",
        mac_min >= 0.9,
        &format!("smallest MAC of tracks 0 and 1: {mac_min:.12}"),
    );
    ch.le("Q1 Campbell speeds residual", q1, 1e-9);
    // P4: crossings linear in Omega.
    let exact = [
        (0, 1, 0.5),
        (0, 2, 1.0 / 3.0),
        (0, 3, 0.25),
        (0, 4, 0.2),
        (1, 3, 0.5),
        (1, 4, 1.0 / 3.0),
    ];
    let mut found = Vec::new();
    for e in 1..=4u32 {
        for c in t.crossings(e) {
            if c.track < 2 {
                found.push(c);
            }
        }
    }
    println!("P4 crossings: {found:?}");
    ch.holds(
        "P4 count",
        found.len() == 6,
        &format!("{} crossings on tracks 0 and 1", found.len()),
    );
    let mut p4: f64 = 0.0;
    let mut n3: f64 = f64::INFINITY;
    for (tr, e, x) in exact {
        let c = found.iter().find(|c| c.track == tr && c.order == e);
        p4 = p4.max(c.map_or(f64::INFINITY, |c| (c.speed / (x * W0) - 1.0).abs()));
        let old = t.table.crossings(e).into_iter().find(|c| c.track == tr);
        n3 = n3.min(old.map_or(f64::INFINITY, |c| (c.speed / (x * W0) - 1.0).abs()));
    }
    ch.le("P4 crossings", p4, 1e-9);
    ch.gt("N3 crossings by the Omega^2 rule (smallest deviation)", n3, 1e-4);
    // P5: CSV and JSON.
    let csv = t.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    let header: Vec<&str> = lines[0].split(',').collect();
    let mut p5 = lines.len() == 14 && header.len() == 2 + 3 + 1 + 3 && header[6] == "track_0_dir";
    for (j, line) in lines[1..].iter().enumerate() {
        let f: Vec<&str> = line.split(',').collect();
        p5 &= f.len() == header.len();
        for tr in 0..3 {
            let v: f64 = f[2 + tr].parse().unwrap();
            p5 &= v.to_bits() == tracks[tr].frequencies[j].unwrap().to_bits();
        }
        let want = if j == 0 { ("", "") } else { ("forward", "backward") };
        p5 &= f[6] == want.0 && f[7] == want.1;
    }
    let json = t.to_json(&[1, 2, 3, 4]);
    p5 &= !json.contains("NaN") && !json.contains("inf");
    p5 &= json.matches("\"direction\"").count() == 3 && json.contains("\"forward\"") && json.contains("\"backward\"");
    println!("{csv}");
    ch.holds(
        "P5",
        p5,
        "CSV rows, direction columns, round-trip bits; JSON finite with directions",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ DS: the sector

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn ds_mesh() -> SolidMesh {
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha(), 0.2], [2, 1, 1]).unwrap();
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

fn ds_sector() -> SolidModel {
    let mut m = model_on(ds_mesh());
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn full_of(sector: &SolidModel) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    (rotor, full)
}

fn ds_modal() -> ModalOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn ds_options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(4).modal(ds_modal()).coriolis(coriolis)
}

fn harmonic_whirl(r: HarmonicWhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

fn omega_ref(rs: &RotatingSector) -> f64 {
    (0..=N / 2)
        .map(|k| harmonic_whirl(rs.whirl_modes(k, 0.0).unwrap(), "rest").modes[0].omega)
        .fold(f64::INFINITY, f64::min)
}

fn worst_relative(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(s, f)| (s / f - 1.0).abs()).fold(0.0f64, f64::max)
}

/// The union over the indices of every positive frequency of the sector, sorted.
fn union(rs: &RotatingSector, w: f64) -> (Vec<f64>, Vec<WhirlModes>) {
    let hs: Vec<WhirlModes> = (0..=N / 2)
        .map(|k| harmonic_whirl(rs.whirl_modes(k, w).unwrap(), "S1"))
        .collect();
    let mut all: Vec<f64> = hs.iter().flat_map(|h| h.frequencies_all.iter().copied()).collect();
    all.sort_by(f64::total_cmp);
    (all, hs)
}

#[test]
fn s1_s2_s3_s4_sector_against_the_full_rotor() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(true)).unwrap();
    let rs_off = RotatingSector::new(&sector, &ds_options(false)).unwrap();
    let w_ref = omega_ref(&rs);
    println!("omega_ref = {w_ref:.9e} rad/s");
    let (rotor, full) = full_of(&sector);
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(true)).unwrap();
    let sdofs = sector.dof_map();
    let fdofs = full.dof_map();
    let fixed: Vec<bool> = {
        let mut f = vec![false; fdofs.n_equations()];
        for (eq, _) in prescribed_equations(&full, &[]).unwrap() {
            f[eq] = true;
        }
        f
    };
    let c_full = fp.coriolis().unwrap();
    let n = sdofs.n_equations();
    let (mut q1, mut q3) = (0.0f64, true);
    for factor in [0.4, 0.8] {
        let w = factor * w_ref;
        let (all, hs) = union(&rs, w);
        let fr = whirl_of(fp.whirl_modes(w).unwrap(), "S1 full rotor");
        println!(
            "{factor} omega_ref: sector {} frequencies (unknowns {:?}), full {} (unknowns {})",
            all.len(),
            hs.iter().map(|h| h.unknowns).collect::<Vec<_>>(),
            fr.frequencies_all.len(),
            fr.unknowns
        );
        ch.holds(
            &format!("S1 {factor} omega_ref count"),
            all.len() == fr.frequencies_all.len(),
            &format!("{} = {}", all.len(), fr.frequencies_all.len()),
        );
        ch.le(
            &format!("S1 {factor} omega_ref frequencies"),
            worst_relative(&all, &fr.frequencies_all),
            1e-8,
        );
        q3 &= fr.frequencies_all.len() == fr.unknowns && fr.frequencies_all.iter().all(|x| x.is_finite());
        q1 = fr.modes.iter().fold(q1, |a, x| a.max(x.residual));
        // S2: the full residual of each expanded sector mode.
        let keff = fp.operator(w);
        let m = fp.mass();
        let mut s2: f64 = 0.0;
        for (k, h) in hs.iter().enumerate() {
            q3 &= h.frequencies_all.len() == h.unknowns && h.frequencies_all.iter().all(|x| x.is_finite());
            let pair = k != 0 && 2 * k != N;
            for x in &h.modes {
                q1 = q1.max(x.residual);
                let split = |v: &[f64]| SectorMode {
                    cos: v[..n].to_vec(),
                    sin: if pair { v[n..].to_vec() } else { vec![0.0; n] },
                };
                let re = rotor.expand(&sdofs, &fdofs, k, &split(&x.re));
                let im = rotor.expand(&sdofs, &fdofs, k, &split(&x.im));
                let (kr, ki) = (matvec(&keff, &re), matvec(&keff, &im));
                let (mr, mi) = (matvec(m, &re), matvec(m, &im));
                let (gr, gi) = (matvec(&c_full, &re), matvec(&c_full, &im));
                let (om, g2) = (x.omega, 2.0 * w);
                let (mut num, mut nk, mut nm, mut ng) = (0.0, 0.0, 0.0, 0.0);
                for i in 0..re.len() {
                    if fixed[i] {
                        continue;
                    }
                    let a = kr[i] - om * om * mr[i] - om * g2 * gi[i];
                    let b = ki[i] - om * om * mi[i] + om * g2 * gr[i];
                    num += a * a + b * b;
                    nk += kr[i] * kr[i] + ki[i] * ki[i];
                    nm += mr[i] * mr[i] + mi[i] * mi[i];
                    ng += gr[i] * gr[i] + gi[i] * gi[i];
                }
                let rel = num.sqrt() / (nk.sqrt() + om * om * nm.sqrt() + om * g2 * ng.sqrt());
                s2 = s2.max(rel);
            }
            println!(
                "{factor} omega_ref k = {k}: omega {:?}, kappa {:?}, wave {:?}",
                h.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
                h.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
                h.modes.iter().map(|x| x.wave).collect::<Vec<_>>()
            );
        }
        ch.le(&format!("S2 {factor} omega_ref full residual"), s2, 1e-8);
        // S3: the wave sense of separated pair modes.
        let mut s3: f64 = 0.0;
        let mut counted = 0;
        for k in [1, 2] {
            let om: Vec<f64> = hs[k].modes.iter().map(|x| x.omega).collect();
            for (i, x) in hs[k].modes.iter().enumerate() {
                let gap_lo = if i > 0 {
                    (om[i] - om[i - 1]) / om[i]
                } else {
                    f64::INFINITY
                };
                let gap_hi = if i + 1 < om.len() {
                    (om[i + 1] - om[i]) / om[i]
                } else {
                    f64::INFINITY
                };
                if gap_lo.min(gap_hi) > 1e-6 {
                    counted += 1;
                    s3 = s3.max((x.wave.unwrap().abs() - 1.0).abs());
                }
            }
        }
        println!("S3 {factor}: {counted} separated pair modes");
        ch.le(&format!("S3 {factor} omega_ref | |tau| - 1 |"), s3, 1e-6);
        if factor == 0.8 {
            let om: Vec<f64> = hs[1].modes.iter().map(|x| x.omega).collect();
            let split = om
                .chunks(2)
                .map(|p| (p[1] - p[0]).abs() / (0.5 * (p[0] + p[1])))
                .fold(0.0f64, f64::max);
            ch.gt("S4 the Coriolis split of index 1 at 0.8 omega_ref", split, 1e-6);
            // N2: the sector without Coriolis against the full rotor with it.
            let (off, _) = union(&rs_off, w);
            ch.gt(
                "N2 sector without Coriolis",
                worst_relative(&off, &fr.frequencies_all),
                1e-6,
            );
        }
    }
    ch.le("Q1 sector and full rotor residual", q1, 1e-9);
    ch.holds("Q3 sector and full rotor", q3, "n positive finite frequencies");
    // Q2: index 1 at 0.4 omega_ref with Coriolis off against the subspace modes of ADR 0407.
    let w = 0.4 * w_ref;
    let off = harmonic_whirl(rs_off.whirl_modes(1, w).unwrap(), "Q2");
    let sub = rs_off.modes(1, w).unwrap();
    let sub = sub.modes().unwrap();
    let q2 = off
        .modes
        .iter()
        .zip(&sub.eigenvalues)
        .map(|(x, l)| (x.omega / l.sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.le("Q2 sector index 1 Coriolis off against modes", q2, 1e-9);
    // Z1 on the sector.
    let mut z1 = true;
    for k in 0..=N / 2 {
        let z = harmonic_whirl(rs.whirl_modes(k, 0.0).unwrap(), "Z1");
        let h = modal_analysis_harmonic(&sector, &[], k, &ds_modal()).unwrap();
        let zl: Vec<f64> = z.modes.iter().map(|x| x.eigenvalue).collect();
        let zf: Vec<f64> = z.modes.iter().map(|x| x.frequency).collect();
        let hf: Vec<f64> = h.eigenvalues.iter().map(|&l| rcfd_fem::modal::frequency(l)).collect();
        z1 &= same_bits(&zl, &h.eigenvalues) && same_bits(&zf, &hf);
    }
    ch.holds(
        "Z1 sector",
        z1,
        "whirl_modes(k, 0) has the bits of modal_analysis_harmonic, k = 0 .. 3",
    );
    ch.finish();
}

#[test]
fn s5_sector_whirl_campbell() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(true)).unwrap();
    let w_ref = omega_ref(&rs);
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let c = rs.whirl_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let mut ok = c.families.len() == 4 && c.multiplicities == vec![1, 2, 2, 1] && c.harmonics == vec![0, 1, 2, 3];
    let mut mac_min: f64 = 1.0;
    for (f, fam) in c.families.iter().enumerate() {
        let want = 4 * c.multiplicities[f];
        ok &= fam.table.tracks.len() == want;
        for (t, tr) in fam.table.tracks.iter().enumerate() {
            for j in 0..speeds.len() {
                ok &= tr.frequencies[j].is_some_and(f64::is_finite);
                if j > 0 {
                    ok &= fam.directions[t][j].is_some();
                    mac_min = mac_min.min(tr.mac[j].unwrap());
                }
            }
        }
        println!(
            "S5 k = {}: tracks {}, directions at 0.8 {:?}",
            c.harmonics[f],
            fam.table.tracks.len(),
            fam.directions.iter().map(|d| d[4]).collect::<Vec<_>>()
        );
    }
    ch.holds(
        "S5",
        ok,
        "4 families, multiplicities 1 2 2 1, tracks 4 8 8 4, finite, a direction everywhere",
    );
    println!("S5 smallest stored MAC (recorded, no limit): {mac_min:.6}");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ R1

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn r1_refusals() {
    let mut ch = Checks::new();
    let cb = cube();
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &cube_options(true)).unwrap();
    ch.holds(
        "R1 speed -1",
        key_of(part.whirl_modes(-1.0).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 speed NaN",
        key_of(part.whirl_modes(f64::NAN).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 speeds decreasing",
        key_of(part.whirl_campbell(&[0.0, 2.0, 1.0]).unwrap_err()) == "speeds",
        "refused with \"speeds\"",
    );
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(true)).unwrap();
    ch.holds(
        "R1 harmonic 4",
        key_of(rs.whirl_modes(4, 1.0).unwrap_err()) == "harmonic",
        "refused with \"harmonic\"",
    );
    let one = [1.0];
    ch.holds(
        "R1 dense wrong length",
        key_of(gyroscopic_eigen_dense(&one, &[1.0, 0.0, 0.0, 1.0], &[0.0; 4], 2).unwrap_err()) == "matrix",
        "refused with \"matrix\"",
    );
    ch.holds(
        "R1 dense K not positive definite",
        key_of(gyroscopic_eigen_dense(&[-1.0, 0.0, 0.0, 1.0], &[1.0, 0.0, 0.0, 1.0], &[0.0; 4], 2).unwrap_err())
            == "stiffness",
        "refused with \"stiffness\"",
    );
    ch.finish();
}
