//! F-507 (ADR 0419): stationary-frame frequencies of a cyclic whirl table, `omega + tau k Omega`, checked on the disc sector
//! of ADR 0411 against the full rotor's whirl modes, whose nodal-diameter index and sense are read from their own
//! eigenvectors (the discrete Fourier content over the copies). Checks T1 to T6 and the negative controls N1 to N3 of the
//! registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::cyclic::{boundary, full_rotor, rotation_matrix, the_cyclic_constraint, FullRotor};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    stationary_frequency, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector, WhirlModes,
    WhirlSpeedModes,
};
use rcfd_fem::{Constraint, Material, SolidMesh, SolidModel};
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
const N: usize = 6;

// ------------------------------------------------------------------ DS of ADR 0411 (helpers as in si_rotating_whirl.rs)

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

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

fn modal(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn ds_options(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(modal(modes)).coriolis(true)
}

fn omega_ref(rs: &RotatingSector) -> f64 {
    (0..=N / 2)
        .map(|k| match rs.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: rest {l:?}"),
        })
        .fold(f64::INFINITY, f64::min)
}

fn whirl_of(r: WhirlSpeedModes, what: &str) -> WhirlModes {
    match r {
        WhirlSpeedModes::Modes(m) => m,
        WhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

// ------------------------------------------------------------------ D3: the content of a full mode over the copies

/// The content of a full mode over the discrete circumferential indices `q = 0 .. N - 1` (ADR 0419 D3): copy-local vectors
/// `(R^j)^T phi_n` at the sector nodes that are not high nodes, the coefficients `(1/N) sum_j u_j e^{sign i q theta_j}`
/// (`sign = +1` registered, `-1` the control N3), Euclidean content per `q`. Returns `(q*, fraction)`, `q*` signed in
/// `(-N/2, N/2]`.
struct Content {
    orbits: Vec<Vec<usize>>,
    rot: Vec<[[f64; 3]; 3]>,
}

impl Content {
    fn new(sector: &SolidModel, rotor: &FullRotor) -> Content {
        let ci = the_cyclic_constraint(sector).unwrap();
        let bd = boundary(sector, ci).unwrap();
        let nn = sector.mesh().nodes().len();
        let mut high = vec![false; nn];
        for &(_, h) in bd.pairs() {
            high[h] = true;
        }
        let orbits: Vec<Vec<usize>> = (0..nn)
            .filter(|&p| !high[p])
            .map(|p| (0..N).map(|j| rotor.node(j, p)).collect())
            .collect();
        let rot = (0..N)
            .map(|j| rotation_matrix(AXIS, 2.0 * PI * j as f64 / N as f64))
            .collect();
        Content { orbits, rot }
    }

    fn dominant(&self, full: &SolidModel, re: &[f64], im: &[f64], sign: f64) -> (i64, f64) {
        let dofs = full.dof_map();
        let mut e = [0.0f64; N];
        for orbit in &self.orbits {
            // Copy-local complex vectors.
            let mut loc = [[(0.0f64, 0.0f64); 3]; N];
            for (j, &node) in orbit.iter().enumerate() {
                let g: Vec<(f64, f64)> = (0..3)
                    .map(|i| {
                        let eq = dofs.equation(node, i).unwrap();
                        (re[eq], im[eq])
                    })
                    .collect();
                let r = &self.rot[j];
                for (c, l) in loc[j].iter_mut().enumerate() {
                    // (R^T g)_c = sum_i R[i][c] g_i
                    *l = (0..3).fold((0.0, 0.0), |a, i| (a.0 + r[i][c] * g[i].0, a.1 + r[i][c] * g[i].1));
                }
            }
            for (q, eq) in e.iter_mut().enumerate() {
                for c in 0..3 {
                    let (mut sr, mut si) = (0.0, 0.0);
                    for (j, l) in loc.iter().enumerate() {
                        let t = sign * 2.0 * PI * (q * j) as f64 / N as f64;
                        let (ct, st) = (t.cos(), t.sin());
                        let (a, b) = l[c];
                        sr += a * ct - b * st;
                        si += a * st + b * ct;
                    }
                    *eq += (sr * sr + si * si) / (N * N) as f64;
                }
            }
        }
        let total: f64 = e.iter().sum();
        let (qmax, emax) = e
            .iter()
            .enumerate()
            .fold((0usize, -1.0f64), |a, (q, &v)| if v > a.1 { (q, v) } else { a });
        let signed = if 2 * qmax <= N {
            qmax as i64
        } else {
            qmax as i64 - N as i64
        };
        (signed, emax / total)
    }
}

// ------------------------------------------------------------------ T6

#[test]
fn t6_stationary_frequency() {
    let mut ch = Checks::new();
    let cases = [
        ((10.0, 2, Some(1.0), 3.0), 16.0f64),
        ((10.0, 2, Some(-1.0), 3.0), 4.0),
        ((10.0, 2, None, 3.0), 10.0),
        ((10.0, 0, Some(1.0), 3.0), 10.0),
        ((10.0, 1, Some(0.5), 4.0), 12.0),
    ];
    for ((w, k, tau, om), want) in cases {
        let got = stationary_frequency(w, k, tau, om);
        ch.holds(
            "T6",
            got.to_bits() == want.to_bits(),
            &format!("stationary_frequency({w}, {k}, {tau:?}, {om}) = {got} (want {want})"),
        );
    }
    ch.finish();
}

// ------------------------------------------------------------------ T1 to T5, N1 to N3

#[test]
fn t1_t5_stationary_column_against_the_full_rotor() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(4)).unwrap();
    let w_ref = omega_ref(&rs);
    println!("omega_ref = {w_ref:.9e} rad/s");
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let c = rs.whirl_campbell(&[0, 1, 2, 3], &speeds).unwrap();

    // T1: the column.
    let mut t1 = c.stationary.len() == c.families.len() && c.families.len() == 4;
    for (f, fam) in c.families.iter().enumerate() {
        let k = c.harmonics[f];
        let col = &c.stationary[f];
        t1 &= col.len() == fam.table.tracks.len();
        for (t, tr) in fam.table.tracks.iter().enumerate() {
            t1 &= col[t].len() == speeds.len();
            for (j, &om) in fam.table.speeds.iter().enumerate() {
                match (tr.frequencies[j], col[t][j]) {
                    (Some(fr), Some(s)) => {
                        let tau = fam.waves[t][j].unwrap_or(0.0);
                        let want = fr + tau * k as f64 * om / (2.0 * PI);
                        t1 &= s.to_bits() == want.to_bits();
                        if om == 0.0 || k == 0 || 2 * k == N {
                            t1 &= s.to_bits() == fr.to_bits();
                        }
                    }
                    (None, None) => {}
                    _ => t1 = false,
                }
            }
        }
    }
    ch.holds(
        "T1",
        t1,
        "stationary: shape of the table, Some where frequencies, bits of f + tau k Omega / (2 pi), f at rest and on k = 0, 3",
    );

    // T2: the CSV.
    let csv = c.stationary_csv();
    let lines: Vec<&str> = csv.lines().collect();
    let mut header = String::from("harmonic,speed_rad_s,speed_rpm");
    for t in 0..8 {
        header.push_str(&format!(",track_{t}_stationary_hz"));
    }
    let mut t2 = lines.len() == 1 + 4 * speeds.len() && lines[0] == header;
    for (f, fam) in c.families.iter().enumerate() {
        for j in 0..speeds.len() {
            let Some(line) = lines.get(1 + f * speeds.len() + j) else {
                t2 = false;
                continue;
            };
            let fields: Vec<&str> = line.split(',').collect();
            t2 &= fields.len() == 3 + 8;
            t2 &= fields[0] == c.harmonics[f].to_string();
            t2 &= fields[1].parse::<f64>().map(f64::to_bits) == Ok(fam.table.speeds[j].to_bits());
            for t in 0..8 {
                let want = c.stationary[f].get(t).and_then(|col| col[j]);
                t2 &= match want {
                    Some(s) => fields[3 + t].parse::<f64>().map(f64::to_bits) == Ok(s.to_bits()),
                    None => fields[3 + t].is_empty(),
                };
            }
        }
    }
    ch.holds(
        "T2",
        t2,
        "stationary_csv: 21 lines, header, values round-trip, empty at None and padding",
    );

    // The full rotor at speeds[2] and speeds[4].
    let (rotor, full) = full_of(&sector);
    let fdofs = full.dof_map();
    let n_fixed = prescribed_equations(&full, &[]).unwrap().len();
    let n_free = fdofs.n_equations() - n_fixed;
    println!("full rotor: {} equations, {n_free} free", fdofs.n_equations());
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(n_free - 1)).unwrap();
    let content = Content::new(&sector, &rotor);
    let (mut t3, mut t4f, mut t4w) = (0.0f64, 1.0f64, 0.0f64);
    let (mut n1, mut n2) = (f64::INFINITY, f64::INFINITY);
    let (mut count, mut disagree, mut disagree_n3) = (0usize, 0usize, 0usize);
    let mut twice = false;
    for j in [2usize, 4] {
        let om = speeds[j];
        let fr = whirl_of(fp.whirl_modes(om).unwrap(), "full rotor");
        // The content of every returned full mode, read from the eigenvector alone.
        let ids: Vec<(i64, f64)> = fr
            .modes
            .iter()
            .map(|x| content.dominant(&full, &x.re, &x.im, 1.0))
            .collect();
        for (f, fam) in c.families.iter().enumerate() {
            let k = c.harmonics[f];
            if k != 1 && k != 2 {
                continue;
            }
            let mut used = vec![false; fr.modes.len()];
            for t in 0..fam.table.tracks.len() {
                let (Some(l), Some(tau), Some(s_hz)) = (
                    fam.table.tracks[t].eigenvalues[j],
                    fam.waves[t][j],
                    c.stationary[f][t][j],
                ) else {
                    continue;
                };
                if (tau.abs() - 1.0).abs() > 1e-6 {
                    continue;
                }
                count += 1;
                let w_s = l.sqrt();
                let s_sector = 2.0 * PI * s_hz;
                // Pairing: |q*| = k, nearest rotating frequency (the sense is not used).
                let best = fr
                    .modes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| ids[*i].0.unsigned_abs() as usize == k)
                    .min_by(|a, b| (a.1.omega - w_s).abs().total_cmp(&(b.1.omega - w_s).abs()));
                let Some((i, x)) = best else {
                    ch.holds("T4", false, &format!("no full mode with |q| = {k} at speed {j}"));
                    continue;
                };
                twice |= used[i];
                used[i] = true;
                let (q, frac) = ids[i];
                t4f = t4f.min(frac);
                t4w = t4w.max((x.omega / w_s - 1.0).abs());
                let s_full = x.omega + q as f64 * om;
                let den = s_full.abs().max(1e-3 * x.omega);
                let dev = (s_sector - s_full).abs() / den;
                t3 = t3.max(dev);
                n1 = n1.min(((w_s - tau * k as f64 * om) - s_full).abs() / den);
                n2 = n2.min((w_s - s_full).abs() / den);
                if (q > 0) != (tau > 0.0) {
                    disagree += 1;
                }
                let (q3, _) = content.dominant(&full, &x.re, &x.im, -1.0);
                if (q3 > 0) != (tau > 0.0) {
                    disagree_n3 += 1;
                }
                println!(
                    "speed {j} k = {k} track {t}: omega_s {w_s:.9e}, tau {tau:+.0}, s_sector {s_sector:.12e}; full mode {i}: \
                     omega_f {:.9e}, q* {q:+}, fraction 1 - {:.1e}, s_full {s_full:.12e}, deviation {dev:.2e}",
                    x.omega,
                    1.0 - frac
                );
            }
        }
    }
    println!("travelling tracks: {count}");
    ch.le("T3 stationary sector against full rotor", t3, 1e-8);
    ch.holds("T3 count", count >= 24, &format!("{count} travelling tracks (>= 24)"));
    ch.le("T4 content defect 1 - fraction", 1.0 - t4f, 1e-8);
    ch.le("T4 rotating frequency of the pairs", t4w, 1e-8);
    ch.holds("T4 pairing", !twice, "no full mode paired twice within one (k, speed)");
    ch.holds(
        "T5",
        disagree == 0,
        &format!("{disagree} sense disagreements of {count}"),
    );
    ch.gt("N1 opposite sense", n1, 1e-2);
    ch.gt("N2 no transformation", n2, 1e-2);
    ch.holds(
        "N3",
        count > 0 && disagree_n3 == count,
        &format!("opposite exponent: {disagree_n3} of {count} disagree (expected all)"),
    );
    ch.finish();
}
