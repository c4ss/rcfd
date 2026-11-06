//! ADR 0205 (F-295), case-tree checks of `turbulence.periodic_seam.diffusivity`: S1 scope and located errors, S2 the written
//! text and the settings hash, T1 / T2 the solver's momentum face viscosity on the B2 fan passage (SIMPLEC and `pimple`, a
//! rotational seam), T3 the laminar rule across the seam, and V0c the explicit default and the mesh without a seam. The limits
//! are those registered in ADR 0205 before this code ran.

mod f295_common;

use f295_common::{b2, b2s, ch, lam, np, record, seam, sim_of, CH_SST, NU_CH, PIMPLE_CH, SIMPLEC};
use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const KEY: &str = "turbulence.periodic_seam.diffusivity";

/// Paths of the diagnostics of a case: parse errors, then static errors, then the errors of `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
}

fn ok(text: &str) {
    let p = paths(text);
    assert!(p.is_empty(), "expected no diagnostic, got {p:?}\n{text}");
}

/// The static diagnostics (`semantic_errors`) of a text that parses: (path, message).
fn static_errors(text: &str) -> Vec<(String, String)> {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    c.semantic_errors()
        .iter()
        .map(|x| (x.path.clone(), x.message.clone()))
        .collect()
}

fn with_model(text: &str, model: &str) -> String {
    assert!(text.contains(CH_SST), "the CH text changed");
    text.replace(CH_SST, &format!("model = \"{model}\"\n"))
}

// ---------------------------------------------------------------------------------------------------------------
// S1 scope, S2 text and hash
// ---------------------------------------------------------------------------------------------------------------

/// S1: what the case tree accepts and where it refuses.
#[test]
fn s1_scope_of_the_seam_diffusivity() {
    let pimple = "algorithm = \"pimple\"\ntime_scheme = \"euler\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n";
    ok(&ch("", &seam("pair"), SIMPLEC));
    ok(&ch("", &seam("pair"), pimple));
    ok(&ch("", &seam("owner"), SIMPLEC));
    ok(&with_model(&ch("", &seam("owner"), SIMPLEC), "k_epsilon"));
    ok(&with_model(&ch("", &seam("owner"), SIMPLEC), "laminar"));
    ok(&np(&seam("pair")));
    println!("S1: accepted: CH SIMPLEC pair, CH pimple pair, CH owner, k_epsilon owner, laminar owner, NP pair");
    for model in ["laminar", "k_epsilon", "spalart_allmaras"] {
        let text = with_model(&ch("", &seam("pair"), SIMPLEC), model);
        let e = static_errors(&text);
        let hit = e.iter().find(|(p, _)| p == KEY);
        let Some((_, msg)) = hit else {
            panic!("S1 {model}: pair not refused statically at {KEY}: {e:?}");
        };
        assert!(
            msg.contains("k_omega_sst"),
            "S1 {model}: the message does not name k_omega_sst: {msg}"
        );
        println!("S1: {model} with pair located at {KEY}: {msg}");
    }
    let bad = ch("", &seam("harmonic"), SIMPLEC);
    let p = paths(&bad);
    assert!(p.iter().any(|x| x == KEY), "S1 harmonic: {p:?}");
    println!("S1: diffusivity = \"harmonic\" located at {p:?}");
}

/// S2: the absent object keeps the written text, the explicit default keeps the hash, `pair` changes it.
#[test]
fn s2_text_and_hash() {
    let parse = |t: &str| Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"));
    let plain = parse(&ch("", "", SIMPLEC));
    let owner = parse(&ch("", &seam("owner"), SIMPLEC));
    let pair = parse(&ch("", &seam("pair"), SIMPLEC));
    let (h0, h1, h2) = (
        plain.tree.content_hash(),
        owner.tree.content_hash(),
        pair.tree.content_hash(),
    );
    let text = plain.to_toml();
    println!(
        "S2: hashes without the key {h0:016x}, owner {h1:016x}, pair {h2:016x}; the written text holds periodic_seam: {}",
        text.contains("periodic_seam")
    );
    assert_eq!(h0, h1, "S2: the explicit default changed the hash");
    assert!(
        !text.contains("periodic_seam"),
        "S2: the written text lists the absent object"
    );
    assert_ne!(h0, h2, "S2: pair has the hash of the default");
}

// ---------------------------------------------------------------------------------------------------------------
// T1, T2: the B2 passage; T3: the laminar rule
// ---------------------------------------------------------------------------------------------------------------

/// The seam pairs `(f, g)`, `f < g`, from the partner maps.
fn seam_faces(sim: &Simulation) -> Vec<(usize, usize)> {
    let g = &sim.geom;
    let mut out = Vec::new();
    for (pi, p) in g.mesh.patches().iter().enumerate() {
        if let Some(map) = &g.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                if f < map[local] {
                    out.push((f, map[local]));
                }
            }
        }
    }
    out
}

/// After `n` steps: (pairs with unequal bits, pairs off the D1 bits, largest `abs(nu_f - nu_g) / max nu_f`, pairs).
fn seam_state(text: &str, n: usize, nu: f64) -> (usize, usize, f64, usize) {
    let mut sim = sim_of(text);
    for k in 1..=n {
        sim.step();
        assert!(sim.failure().is_none(), "step {k}: {:?}", sim.failure());
    }
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let nut = sim.field("nut").unwrap();
    let g = &sim.geom;
    let own = g.mesh.owner();
    let pairs = seam_faces(&sim);
    let (mut unequal, mut off) = (0, 0);
    let (mut diff, mut big) = (0.0_f64, 0.0_f64);
    for &(f, q) in &pairs {
        if face[f].to_bits() != face[q].to_bits() {
            unequal += 1;
        }
        let (dn_f, dn_q) = (g.dn[f - g.n_internal], g.dn[q - g.n_internal]);
        let w = dn_q / (dn_f + dn_q);
        let d1 = nu + w * nut[own[f]] + (1.0 - w) * nut[own[q]];
        if face[f].to_bits() != d1.to_bits() {
            off += 1;
        }
        diff = diff.max((face[f] - face[q]).abs());
        big = big.max(face[f]).max(face[q]);
    }
    (unequal, off, diff / big, pairs.len())
}

fn t_b2(id: &str, pair_text: &str, owner_text: &str, n: usize) {
    let (unequal, off, _, pairs) = seam_state(pair_text, n, 1.5e-5);
    let (_, _, owner_rel, _) = seam_state(owner_text, n, 1.5e-5);
    println!(
        "{id}: {pairs} seam pairs after {n}; pair: {unequal} with unequal bits, {off} off the D1 bits; owner: largest relative pair difference {owner_rel:.4e} (must be >= 1e-2)"
    );
    assert!(pairs > 0, "{id}: no seam");
    assert_eq!(unequal, 0, "{id}: pairs with unequal bits");
    assert_eq!(off, 0, "{id}: pairs off the D1 bits");
    assert!(
        owner_rel >= 1e-2,
        "{id}: the owner control did not fire, {owner_rel:.3e}"
    );
}

/// T1: B2 on SIMPLEC, 5 iterations: one value per seam pair, the D1 bits; the owner values differ.
#[test]
fn t1_b2_simplec_seam_face_viscosity() {
    t_b2("T1", &b2s(&seam("pair")), &b2s(""), 5)
}

/// T2: B2 on pimple, 3 steps, as T1.
#[test]
fn t2_b2_pimple_seam_face_viscosity() {
    t_b2("T2", &b2(&seam("pair")), &b2(""), 3)
}

/// The momentum face viscosity of the seam faces of LAM after 3 iterations: (faces off the bits of nu, largest value).
fn lam_seam(extra: &str) -> (usize, f64, usize) {
    let mut sim = sim_of(&lam(extra));
    for k in 1..=3 {
        sim.step();
        assert!(sim.failure().is_none(), "T3 step {k}: {:?}", sim.failure());
    }
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let pairs = seam_faces(&sim);
    let faces: Vec<usize> = pairs.iter().flat_map(|&(f, g)| [f, g]).collect();
    let off = faces.iter().filter(|&&f| face[f].to_bits() != NU_CH.to_bits()).count();
    let big = faces.iter().map(|&f| face[f]).fold(0.0, f64::max);
    (off, big, faces.len())
}

/// T3: a pair that touches a laminar cell takes the molecular viscosity on both faces.
#[test]
fn t3_laminar_region_at_the_seam() {
    let (off, _, n) = lam_seam(&seam("pair"));
    let (_, big_owner, _) = lam_seam("");
    println!(
        "T3: {n} seam faces; pair: {off} off the bits of nu; owner: largest seam face viscosity {:.4e} = {:.3} nu (must be >= 1.001)",
        big_owner,
        big_owner / NU_CH
    );
    assert!(n > 0, "T3: no seam");
    assert_eq!(off, 0, "T3: pair faces off the molecular value");
    assert!(big_owner >= 1.001 * NU_CH, "T3: the owner control did not fire");
}

// ---------------------------------------------------------------------------------------------------------------
// V0c: the explicit default, and pair without a seam
// ---------------------------------------------------------------------------------------------------------------

/// V0c: the CH pimple record of V0b with `owner` written; NP with `pair` equals NP without the key.
#[test]
fn v0c_explicit_owner_and_no_seam() {
    let c1 = record(&ch("", &seam("owner"), PIMPLE_CH), 20);
    println!(
        "V0c1: CH pimple with owner written: ({}, {}, 0x{:016x})",
        c1.0, c1.1, c1.2
    );
    assert_eq!(c1, (20, 7, 0xdada_2826_3c54_03a0), "V0c1: not the V0b record");
    let (a, b) = (record(&np(&seam("pair")), 100), record(&np(""), 100));
    println!(
        "V0c2: NP with pair ({}, {}, 0x{:016x}), without the key ({}, {}, 0x{:016x})",
        a.0, a.1, a.2, b.0, b.1, b.2
    );
    assert_eq!(a, b, "V0c2: pair changed a mesh without a seam");
}
