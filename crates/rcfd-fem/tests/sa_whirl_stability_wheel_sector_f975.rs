//! F-975 (ADR 0806): `RotatingSector::whirl_stability` of the wheel sector W of ADR 0595 converges. Checks A1, A2, V1 to V4,
//! E-W2 and B-W of the registration (DG1 to DG3 are the unit test `rotating::whirl::state::f975_diagnosis`; B-F, B1 to B3
//! the existing tests and `baseline_hashes`). E-W2 runs the set-up of the known miss
//! `e_w_whirl_paths_of_the_beam_sector_without_the_options_known_miss` of `sa_beam_opt_ins_default_f672.rs` (ADR 0799) in a
//! new test; the record is not touched. The builders, `whirl_texts`, `same_texts` and the hash are those of
//! `sa_beam_opt_ins_default_f672.rs` and `sa_beam_opt_ins_default_f672_bits.rs`, copied.

use rcfd_fem::cyclic::modal_analysis_harmonic;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingSector, SparseStateOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// Ring elements per sector.
const NR: usize = 4;
/// The loaded sector nodes of ADR 0595 (their node sets are kept, the builders being copies).
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a * x[k] + b * y[k])
}

/// `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`.
fn p(phi: f64, r: f64, z: f64) -> Vec3 {
    let (n, a, b) = (axis(), ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn ring_section() -> BeamSection {
    BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap()
}

fn spoke_section() -> BeamSection {
    BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap()
}

fn v_ring() -> Vec3 {
    comb(0.8, axis(), 0.6, ea())
}

fn v_spoke() -> Vec3 {
    comb(0.6, eb(), 0.8, axis())
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn rotating_options(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(modal_options(modes))
}

/// Option set (a) of ADR 0799: the shells option alone (`default_options` of the ADR 0744 test).
fn options_a() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

/// Option set (b): with the beams option (`beam_options` of the ADR 0595 tests).
fn options_b() -> CyclicOptions {
    options_a().beams(true)
}

/// Option set (c): with the full-rotor option as well (`full_options` of the ADR 0601 test).
fn options_c() -> CyclicOptions {
    options_b().full_rotor_beams(true)
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
}

fn wheel_mesh() -> SolidMesh {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    nodes.push(p(a / 2.0, 0.65, 0.125));
    nodes.push(p(a / 2.0, 0.3, 0.25));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(ring_section(), v_ring()).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(spoke_section(), v_spoke()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..6).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![NR]).unwrap();
    mesh.add_node_set("hub", vec![6]).unwrap();
    for q in LOADED {
        mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
    }
    mesh
}

/// The wheel sector W of ADR 0595 with the cyclic constraint of `options`.
fn sector(options: CyclicOptions) -> SolidModel {
    let mut m = SolidModel::new(wheel_mesh());
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The whirl paths P1 to P8 of ADR 0799 on a sector model: (name, `Debug` text of the result).
fn whirl_texts(m: &SolidModel) -> Vec<(String, String)> {
    let o = RotatingOptions::new(4).modal(modal_options(4)).coriolis(true);
    let sec = match RotatingSector::new(m, &o) {
        Ok(s) => s,
        Err(e) => return vec![("RotatingSector::new".to_string(), format!("Err({e:?})"))],
    };
    let ks = [0usize, 1, 2, 3];
    let speeds = [0.0, 30.0, 60.0];
    let mut out = vec![("P1 coriolis".to_string(), format!("{:?}", sec.part().coriolis()))];
    for k in ks {
        out.push((
            format!("P2 whirl_modes k = {k}"),
            format!("{:?}", sec.whirl_modes(k, 30.0)),
        ));
    }
    out.push((
        "P3 whirl_campbell".to_string(),
        format!("{:?}", sec.whirl_campbell(&ks, &speeds)),
    ));
    for k in ks {
        out.push((
            format!("P4 split_modes k = {k}"),
            format!("{:?}", sec.split_modes(k, 30.0)),
        ));
    }
    out.push((
        "P5 split_campbell".to_string(),
        format!("{:?}", sec.split_campbell(&ks, &speeds)),
    ));
    for k in ks {
        out.push((
            format!("P6 whirl_state k = {k}"),
            format!("{:?}", sec.whirl_state(k, 30.0)),
        ));
    }
    out.push((
        "P7 whirl_stability".to_string(),
        format!("{:?}", sec.whirl_stability(&ks, &speeds)),
    ));
    let sparse = sec.whirl_state_sparse(1, 30.0, &SparseStateOptions::new(2.0e4, 4));
    out.push(("P8 whirl_state_sparse k = 1".to_string(), format!("{sparse:?}")));
    out
}

/// One row: the texts of (a), (b), (c) path by path. `ok`: every result must also be other than an error. Returns the
/// failed paths.
fn same_texts(row: &str, sets: [&[(String, String)]; 3], ok: bool) -> Vec<String> {
    let mut fails = Vec::new();
    let [a, b, c] = sets;
    if a.len() != b.len() || a.len() != c.len() {
        println!("{row}: path counts {} {} {} MISSED", a.len(), b.len(), c.len());
        return vec![format!("{row}: path counts")];
    }
    for i in 0..a.len() {
        let (name, ta) = &a[i];
        let ab = b[i] == a[i];
        let ac = c[i] == a[i];
        let not_err = !ta.starts_with("Err(") && !b[i].1.starts_with("Err(") && !c[i].1.starts_with("Err(");
        let good = ab && ac && (!ok || not_err);
        let head: String = ta.chars().take(60).collect();
        println!(
            "{row} {name}: (a) = (b) {ab}, (a) = (c) {ac}, not an error {not_err}, {} characters {}; (a) {head}...",
            ta.len(),
            met(good)
        );
        if ta.starts_with("Err(") {
            println!("    (a) in full: {ta}");
        }
        if !good {
            fails.push(format!("{row} {name}"));
        }
    }
    fails
}

/// 64-bit FNV-1a over the texts, as `fnv` of `sa_beam_opt_ins_default_f672_bits.rs`.
fn fnv(texts: &[(String, String)]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for (name, text) in texts {
        for b in name.bytes().chain(*b"\n").chain(text.bytes()).chain(*b"\n") {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

const KS: [usize; 4] = [0, 1, 2, 3];
const SPEEDS: [f64; 3] = [0.0, 30.0, 60.0];

fn wheel_sector(m: &SolidModel) -> RotatingSector<'_> {
    RotatingSector::new(m, &rotating_options(4).coriolis(true)).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

// ---------------------------------------------------------------------------------------------------------------- A1, A2

#[test]
fn a1_a2_whirl_stability_of_w_and_whirl_state() {
    let m = sector(options_a());
    let sec = wheel_sector(&m);
    let r = sec.whirl_stability(&KS, &SPEEDS);
    let a1 = r.is_ok();
    println!("A1 whirl_stability Ok {a1}: {}", met(a1));
    if let Err(e) = &r {
        println!("    {e:?}");
    }
    let mut a2 = a1;
    if let Ok(r) = &r {
        for (h, &k) in KS.iter().enumerate() {
            for (j, &w) in SPEEDS.iter().enumerate() {
                let one = sec.whirl_state(k, w);
                let same = match &one {
                    Ok(x) => format!("{x:?}") == format!("{:?}", r.results[h][j]),
                    Err(_) => false,
                };
                let ev = one
                    .as_ref()
                    .map(|x| x.state.eigenvalues == r.results[h][j].state.eigenvalues);
                println!(
                    "A2 k = {k}, speed {w}: texts equal {same}, eigenvalues equal {ev:?} ({} eigenvalues): {}",
                    r.results[h][j].state.eigenvalues.len(),
                    met(same)
                );
                a2 &= same;
            }
        }
    }
    println!("A2: {}", met(a2));
    assert!(a1, "A1");
    assert!(a2, "A2");
}

// ---------------------------------------------------------------------------------------------------------- V1 to V4

#[test]
fn v1_v4_frequencies_flags_and_residuals_against_the_other_routes() {
    let m = sector(options_a());
    let sec = wheel_sector(&m);
    let r = sec.whirl_stability(&KS, &SPEEDS).expect("whirl_stability (A1)");
    let (mut v1, mut v2, mut v3, mut v4) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut v1_ok, mut v2_ok) = (true, true);
    for (h, &k) in KS.iter().enumerate() {
        for (j, &w) in SPEEDS.iter().enumerate() {
            let x = &r.results[h][j];
            let st = &x.state;
            let omegas: Vec<f64> = st.modes.iter().map(|q| q.omega).collect();
            let reference: Vec<f64> = if w == 0.0 {
                let hm = modal_analysis_harmonic(&m, &[], k, &modal_options(4)).unwrap();
                let base: Vec<f64> = hm.eigenvalues.iter().map(|l| l.sqrt()).collect();
                base.iter()
                    .flat_map(|&o| std::iter::repeat(o).take(x.multiplicity))
                    .collect()
            } else {
                match sec.whirl_modes(k, w).unwrap().modes() {
                    Some(wm) => wm.modes.iter().map(|q| q.omega).collect(),
                    None => Vec::new(),
                }
            };
            let worst = if omegas.len() == reference.len() && !omegas.is_empty() {
                omegas
                    .iter()
                    .zip(&reference)
                    .map(|(a, b)| rel(*a, *b))
                    .fold(0.0f64, f64::max)
            } else {
                f64::INFINITY
            };
            let stable = st.stability.is_stable() && st.real_defect <= 1e-8;
            let res = st.modes.iter().map(|q| q.residual).fold(0.0f64, f64::max);
            v3 = v3.max(if stable { st.real_defect } else { f64::INFINITY });
            v4 = v4.max(res);
            if w == 0.0 {
                v1 = v1.max(worst);
                v1_ok &= worst <= 1e-8;
            } else {
                v2 = v2.max(worst);
                v2_ok &= worst <= 1e-8;
            }
            println!(
                "{} k = {k}, speed {w}: {} frequencies against {}, largest relative difference {worst:.3e}; {:?}, real_defect \
                 {:.3e}; largest residual {res:.3e}",
                if w == 0.0 { "V1" } else { "V2" },
                omegas.len(),
                reference.len(),
                st.stability,
                st.real_defect
            );
            println!("    omegas {omegas:?}");
            println!("    reference {reference:?}");
        }
    }
    let v3_ok = v3 <= 1e-8;
    let v4_ok = v4 <= 1e-10;
    println!("V1: largest {v1:.3e} (limit 1e-8): {}", met(v1_ok));
    println!("V2: largest {v2:.3e} (limit 1e-8): {}", met(v2_ok));
    println!(
        "V3: every index and speed Stable, largest real_defect {v3:.3e} (limit 1e-8): {}",
        met(v3_ok)
    );
    println!("V4: largest residual {v4:.3e} (limit 1e-10): {}", met(v4_ok));
    assert!(v1_ok, "V1");
    assert!(v2_ok, "V2");
    assert!(v3_ok, "V3");
    assert!(v4_ok, "V4");
}

// ---------------------------------------------------------------------------------------------------------------- E-W2

#[test]
fn e_w2_the_known_miss_of_adr_0799_with_its_set_up() {
    let texts = [options_a(), options_b(), options_c()].map(|o| whirl_texts(&sector(o)));
    let fails = same_texts("E-W2", [&texts[0], &texts[1], &texts[2]], true);
    println!("E-W2: {}", met(fails.is_empty()));
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------------------------- B-W

/// The Windows x86_64 record `H_W` of `sa_beam_opt_ins_default_f672_bits.rs` (ADR 0799).
const H_W_WINDOWS: u64 = 0xe314b82213528f82;

#[test]
fn b_w_the_other_paths_of_w_keep_the_record_of_main() {
    let mut w = whirl_texts(&sector(options_b()));
    let main_p7: Result<(), FemError> = Err(FemError::Solver {
        solver: "gyroscopic_state_eigen",
        reason: "the QR iteration did not converge on rows 116..=119 in 30 steps".to_string(),
    });
    let mut replaced = 0;
    for (name, text) in w.iter_mut() {
        if name == "P7 whirl_stability" {
            println!("B-W P7 now: {} characters, Ok {}", text.len(), text.starts_with("Ok("));
            *text = format!("{main_p7:?}");
            println!("B-W P7 replaced by {text}");
            replaced += 1;
        }
    }
    let got = fnv(&w);
    println!(
        "B-W hash with main's P7 = {got:#018x} on {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert_eq!(replaced, 1, "B-W: P7 not found");
    if cfg!(all(windows, target_arch = "x86_64")) {
        println!(
            "B-W record {H_W_WINDOWS:#018x}: equal {}: {}",
            got == H_W_WINDOWS,
            met(got == H_W_WINDOWS)
        );
        assert_eq!(got, H_W_WINDOWS, "B-W");
    } else {
        println!("B-W: no record of main's P7 text for this platform; printed, not compared");
    }
}
