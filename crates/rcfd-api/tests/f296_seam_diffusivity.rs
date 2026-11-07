//! ADR 0212 (F-296), case-tree checks of `turbulence.periodic_seam.diffusivity` for the other models and the viscosity law: S1'
//! scope and located errors, S2' the settings hash, T4 / T4r the solver's momentum face viscosity on the seam (k-epsilon,
//! Spalart-Allmaras, mixing length, Smagorinsky, WALE, the Reynolds stress model, a power-law fluid), T5 the laminar rule across the
//! seam with k-epsilon, F2r / G2r the face values and the global diffusion budget of the viscosity law, and V0c' the explicit
//! default and the mesh without a seam. The limits are those registered in ADR 0212 before this code ran.

mod f295_common;
mod f296_common;

use f296_common::{
    chx, chxr, lamx, npk, record, seam, sim_of, tgs, v0_cases, KE, ML, NU_CH, PIMPLE, PISO, RSM, SA, SMAG, V0B, WALE,
};
use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_3d_sector, box_2d, box_2d_graded, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

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

fn ok(id: &str, text: &str) {
    let p = paths(text);
    assert!(p.is_empty(), "S1' {id}: expected no diagnostic, got {p:?}\n{text}");
}

/// The static diagnostics of a text (ADR 0205 amendment 1): the error of `Case::parse_toml`, which runs the semantic checks,
/// or the `semantic_errors` of a text that parses.
fn static_errors(text: &str) -> Vec<(String, String)> {
    let d = match Case::parse_toml(text) {
        Err(d) => d,
        Ok(c) => c.semantic_errors(),
    };
    d.iter().map(|x| (x.path.clone(), x.message.clone())).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// S1', S2'
// ---------------------------------------------------------------------------------------------------------------

/// S1': what the case tree accepts with `pair` and where it refuses.
#[test]
fn s1_scope_of_the_seam_diffusivity_for_every_model() {
    use f295_common::{ch, np, CH_SST, SIMPLEC};
    let with_model = |text: &str, model: &str| {
        assert!(text.contains(CH_SST), "the CH text changed");
        text.replace(CH_SST, model)
    };
    let pair = seam("pair");
    let pimple = "algorithm = \"pimple\"\ntime_scheme = \"euler\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n";
    let accepted = [
        ("CHX k_epsilon pair", chx(KE, &pair)),
        ("CHX spalart_allmaras pair", chx(SA, &pair)),
        ("CHX mixing_length pair", chx(ML, &pair)),
        ("CHXR pair", chxr(&pair)),
        ("TGS smagorinsky pair", tgs(SMAG, &pair, PISO)),
        ("TGS wale pair", tgs(WALE, &pair, PIMPLE)),
        ("TGS rsm pair", tgs(RSM, &pair, PISO)),
        ("CH SST SIMPLEC pair", ch("", &pair, SIMPLEC)),
        ("CH SST pimple pair", ch("", &pair, pimple)),
        ("NP pair", np(&pair)),
        ("CH SST owner", ch("", &seam("owner"), SIMPLEC)),
        ("CH k_epsilon owner", with_model(&ch("", &seam("owner"), SIMPLEC), KE)),
        (
            "CH laminar owner",
            with_model(&ch("", &seam("owner"), SIMPLEC), "model = \"laminar\"\n"),
        ),
    ];
    for (id, text) in &accepted {
        ok(id, text);
    }
    println!(
        "S1': accepted: {}",
        accepted.iter().map(|a| a.0).collect::<Vec<_>>().join(", ")
    );
    let refused = [
        (
            "CH laminar pair",
            with_model(&ch("", &pair, SIMPLEC), "model = \"laminar\"\n"),
            "rheology",
        ),
        (
            "CH plugin pair",
            with_model(&ch("", &pair, SIMPLEC), "model = \"plugin\"\nplugin = \"k_epsilon\"\n"),
            "plugin",
        ),
    ];
    for (id, text, word) in &refused {
        let e = static_errors(text);
        let Some((_, msg)) = e.iter().find(|(p, _)| p == KEY) else {
            panic!("S1' {id}: pair not refused statically at {KEY}: {e:?}");
        };
        assert!(msg.contains(word), "S1' {id}: the message does not name {word}: {msg}");
        println!("S1': {id} located at {KEY}: {msg}");
    }
    let bad = ch("", &seam("harmonic"), SIMPLEC);
    let p = paths(&bad);
    assert!(p.iter().any(|x| x == KEY), "S1' harmonic: {p:?}");
    println!("S1': diffusivity = \"harmonic\" located at {p:?}");
}

/// S2': the explicit default keeps the hash of a k-epsilon case, `pair` changes it.
#[test]
fn s2_hash_with_k_epsilon() {
    let parse = |t: &str| Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"));
    let h = |t: &str| parse(t).tree.content_hash();
    let (h0, h1, h2) = (h(&chx(KE, "")), h(&chx(KE, &seam("owner"))), h(&chx(KE, &seam("pair"))));
    println!("S2': CHX k_epsilon hashes without the key {h0:016x}, owner {h1:016x}, pair {h2:016x}");
    assert_eq!(h0, h1, "S2': the explicit default changed the hash");
    assert_ne!(h0, h2, "S2': pair has the hash of the default");
}

// ---------------------------------------------------------------------------------------------------------------
// T4, T4r, T5
// ---------------------------------------------------------------------------------------------------------------

/// The seam pairs `(f, g)`, `f < g`, from the partner maps.
fn seam_faces(geom: &FvGeometry) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, p) in geom.mesh.patches().iter().enumerate() {
        if let Some(map) = &geom.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                if f < map[local] {
                    out.push((f, map[local]));
                }
            }
        }
    }
    out
}

/// After `n` steps: (pairs with unequal bits, pairs off the D1 bits from the field `nut` (`None` without one), largest
/// `abs(nu_f - nu_g) / max nu_f`, pairs).
fn seam_state(text: &str, n: usize, nu: f64) -> (usize, Option<usize>, f64, usize) {
    let mut sim = sim_of(text);
    for k in 1..=n {
        sim.step();
        assert!(sim.failure().is_none(), "step {k}: {:?}", sim.failure());
    }
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let nut = sim.field("nut").ok();
    let g = &sim.geom;
    let own = g.mesh.owner();
    let pairs = seam_faces(g);
    let (mut unequal, mut off) = (0, 0);
    let (mut diff, mut big) = (0.0_f64, 0.0_f64);
    for &(f, q) in &pairs {
        if face[f].to_bits() != face[q].to_bits() {
            unequal += 1;
        }
        if let Some(nut) = &nut {
            let (dn_f, dn_q) = (g.dn[f - g.n_internal], g.dn[q - g.n_internal]);
            let w = dn_q / (dn_f + dn_q);
            let d1 = nu + w * nut[own[f]] + (1.0 - w) * nut[own[q]];
            if face[f].to_bits() != d1.to_bits() {
                off += 1;
            }
        }
        diff = diff.max((face[f] - face[q]).abs());
        big = big.max(face[f]).max(face[q]);
    }
    (unequal, nut.map(|_| off), diff / big, pairs.len())
}

/// T4 / T4r of one text: `pair` exact (the D1 bits where the run has `nut`), `owner` at least `limit`.
fn t4(id: &str, pair_text: &str, owner_text: &str, n: usize, nu: f64, limit: f64, d1: bool) {
    let (unequal, off, _, pairs) = seam_state(pair_text, n, nu);
    let (_, _, owner_rel, _) = seam_state(owner_text, n, nu);
    println!(
        "{id}: {pairs} seam pairs after {n}; pair: {unequal} with unequal bits, {off:?} off the D1 bits; owner: largest relative pair difference {owner_rel:.4e} (must be >= {limit:e})"
    );
    assert!(pairs > 0, "{id}: no seam");
    assert_eq!(unequal, 0, "{id}: pairs with unequal bits");
    if d1 {
        assert_eq!(off, Some(0), "{id}: pairs off the D1 bits");
    }
    assert!(
        owner_rel >= limit,
        "{id}: the owner control did not fire, {owner_rel:.3e}"
    );
}

/// T4: k-epsilon, Spalart-Allmaras and the mixing length on CHX (SIMPLEC, 5 iterations): one value per seam pair, the D1 bits;
/// the owner values differ.
#[test]
fn t4_chx_steady_models_seam_face_viscosity() {
    let p = seam("pair");
    t4("T4 CHX k_epsilon", &chx(KE, &p), &chx(KE, ""), 5, NU_CH, 1e-3, true);
    t4(
        "T4 CHX spalart_allmaras",
        &chx(SA, &p),
        &chx(SA, ""),
        5,
        NU_CH,
        1e-3,
        true,
    );
    t4("T4 CHX mixing_length", &chx(ML, &p), &chx(ML, ""), 5, NU_CH, 1e-3, true);
}

/// T4: Smagorinsky (piso) and WALE (pimple) after 3 steps, the Reynolds stress model (piso) after 5, on TGS.
#[test]
fn t4_tgs_transient_models_seam_face_viscosity() {
    let p = seam("pair");
    t4(
        "T4 TGS smagorinsky",
        &tgs(SMAG, &p, PISO),
        &tgs(SMAG, "", PISO),
        3,
        0.05,
        1e-3,
        true,
    );
    t4(
        "T4 TGS wale",
        &tgs(WALE, &p, PIMPLE),
        &tgs(WALE, "", PIMPLE),
        3,
        0.05,
        1e-3,
        true,
    );
    t4(
        "T4 TGS rsm",
        &tgs(RSM, &p, PISO),
        &tgs(RSM, "", PISO),
        5,
        0.05,
        1e-5,
        true,
    );
}

/// T4r: the power-law fluid on CHX (SIMPLEC, 5 iterations): one value per seam pair; the owner values differ.
#[test]
fn t4r_chxr_viscosity_law_seam_face_viscosity() {
    t4("T4r CHXR", &chxr(&seam("pair")), &chxr(""), 5, NU_CH, 1e-3, false);
}

/// The momentum face viscosity of the seam faces of LAMX after 3 iterations: (faces off the bits of nu, largest value, faces).
fn lamx_seam(extra: &str) -> (usize, f64, usize) {
    let mut sim = sim_of(&lamx(extra));
    for k in 1..=3 {
        sim.step();
        assert!(sim.failure().is_none(), "T5 step {k}: {:?}", sim.failure());
    }
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let faces: Vec<usize> = seam_faces(&sim.geom).iter().flat_map(|&(f, g)| [f, g]).collect();
    let off = faces.iter().filter(|&&f| face[f].to_bits() != NU_CH.to_bits()).count();
    let big = faces.iter().map(|&f| face[f]).fold(0.0, f64::max);
    (off, big, faces.len())
}

/// T5: with k-epsilon a pair that touches a laminar cell takes the molecular viscosity on both faces.
#[test]
fn t5_laminar_region_at_the_seam_with_k_epsilon() {
    let (off, _, n) = lamx_seam(&seam("pair"));
    let (_, big_owner, _) = lamx_seam("");
    println!(
        "T5: {n} seam faces; pair: {off} off the bits of nu; owner: largest seam face viscosity {:.4e} = {:.3} nu (must be >= 1.001)",
        big_owner,
        big_owner / NU_CH
    );
    assert!(n > 0, "T5: no seam");
    assert_eq!(off, 0, "T5: pair faces off the molecular value");
    assert!(big_owner >= 1.001 * NU_CH, "T5: the owner control did not fire");
}

// ---------------------------------------------------------------------------------------------------------------
// F2r, G2r: the viscosity law
// ---------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mesh {
    Box,
    Sec,
    GBox,
}

fn geometry(mesh: Mesh) -> Arc<FvGeometry> {
    let m = match mesh {
        Mesh::Box => box_2d(16, 4, 1.0, 1.0, true, false).unwrap(),
        Mesh::Sec => annulus_3d_sector(8, 6, 4, 0.5, 1.0, 0.5, 40f64.to_radians(), Grading::Uniform).unwrap(),
        Mesh::GBox => box_2d_graded(
            16,
            4,
            1.0,
            1.0,
            [Grading::Expansion(4.0), Grading::Uniform],
            true,
            false,
        )
        .unwrap(),
    };
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

fn s_of(mesh: Mesh, p: [f64; 3]) -> f64 {
    match mesh {
        Mesh::Sec => p[1].atan2(p[0]) / 40f64.to_radians(),
        _ => p[0],
    }
}

/// The velocity of F2r: `(sin(2 pi s) + 0.1 y, 0.5 sin(2 pi s + 1), 0.3 z)`, times `scale`.
fn f2r_velocity(geom: &FvGeometry, mesh: Mesh, scale: f64) -> VectorField {
    VectorField::from_fn("U", geom, |c| {
        let s = s_of(mesh, c);
        [
            scale * ((2.0 * PI * s).sin() + 0.1 * c[1]),
            scale * 0.5 * (2.0 * PI * s + 1.0).sin(),
            scale * 0.3 * c[2],
        ]
    })
}

/// The law of CHXR.
fn law() -> rcfd_api::rheology::Rheology {
    let case = Case::parse_toml(&chxr("")).unwrap_or_else(|e| panic!("{e:?}"));
    rcfd_api::rheology::Rheology::from_case(&case).expect("a viscosity law")
}

/// G of ADR 0205 G1.
fn budget(geom: &FvGeometry, gamma: &[f64], x: &[f64], bc: &[rcfd_fv::FaceCoeffs]) -> f64 {
    let m = laplacian(geom, gamma, bc, None);
    let r = m.residual(x);
    let total: f64 = r.iter().sum();
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut flux = 0.0;
    for f in 0..mesh.n_faces() {
        if f < geom.n_internal {
            flux += (gamma[f] * geom.lap_coeff[f] * (x[nei[f]] - x[own[f]])).abs();
        } else if !geom.empty[f] {
            let co = &bc[f - geom.n_internal];
            flux += (gamma[f] * vec3::norm(mesh.face_area_vectors()[f]) * co.gradient(x[own[f]])).abs();
        }
    }
    total.abs() / flux
}

/// F2r and G2r on one mesh.
fn f2r_g2r(mesh: Mesh) {
    let geom = geometry(mesh);
    let t = 293.15;
    let u = f2r_velocity(&geom, mesh, 1.0);
    let owner = law().nu_faces(&geom, &u, None, t).unwrap();
    let mut lp = law();
    let pairs = geom.periodic_pairs().unwrap();
    lp.set_seam_pairs(Some(pairs.clone()));
    let pair = lp.nu_faces(&geom, &u, None, t).unwrap();
    // the relaxed second call (relax = 0.7) on 1.1 u
    let relaxed = lp.nu_faces(&geom, &f2r_velocity(&geom, mesh, 1.1), None, t).unwrap();
    let shear = rcfd_turb::eddy::strain_rate(&geom, &u);
    let l = law();
    let mu: Vec<f64> = shear.iter().map(|&g| l.viscosity(g, t)).collect();
    let own = geom.mesh.owner();
    let seam = seam_faces(&geom);
    let mut on_seam = vec![false; geom.mesh.n_faces()];
    let (mut unequal, mut not_d1, mut unequal_relaxed) = (0, 0, 0);
    let mut owner_diff: f64 = 0.0;
    for &(f, g) in &seam {
        on_seam[f] = true;
        on_seam[g] = true;
        if pair[f].to_bits() != pair[g].to_bits() {
            unequal += 1;
        }
        if relaxed[f].to_bits() != relaxed[g].to_bits() {
            unequal_relaxed += 1;
        }
        let (dn_f, dn_g) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
        let w = dn_g / (dn_f + dn_g);
        let d1 = (w * mu[own[f]] + (1.0 - w) * mu[own[g]]) / l.rho;
        if pair[f].to_bits() != d1.to_bits() {
            not_d1 += 1;
        }
        owner_diff = owner_diff.max((owner[f] - owner[g]).abs());
    }
    let changed_elsewhere = (0..geom.mesh.n_faces())
        .filter(|&x| !on_seam[x] && pair[x].to_bits() != owner[x].to_bits())
        .count();
    let big = owner.iter().cloned().fold(0.0, f64::max);
    println!(
        "F2r {mesh:?}: {} pairs, relax {}; pair: {unequal} with unequal bits, {not_d1} not the D1 bits, {changed_elsewhere} other faces changed, {unequal_relaxed} unequal after the relaxed call; owner: largest pair difference {:.4e} of the largest face viscosity {big:.4e} (must be >= 1e-3)",
        seam.len(),
        l.relax,
        owner_diff / big
    );
    assert!(!seam.is_empty(), "F2r {mesh:?}: no seam");
    assert_eq!(unequal, 0, "F2r {mesh:?}: pairs with unequal bits");
    assert_eq!(not_d1, 0, "F2r {mesh:?}: pairs off the D1 bits");
    assert_eq!(changed_elsewhere, 0, "F2r {mesh:?}: faces off the seam changed");
    assert_eq!(unequal_relaxed, 0, "F2r {mesh:?}: the relaxed call split a pair");
    assert!(owner_diff >= 1e-3 * big, "F2r {mesh:?}: vacuous");
    // G2r
    let sec = mesh == Mesh::Sec;
    let q = ScalarField::from_fn("q", &geom, |p| {
        1.0 + 0.5 * (2.0 * PI * s_of(mesh, p)).sin() + 0.2 * if sec { p[2] } else { p[1] }
    });
    let mut fields = vec![("q", q.values.clone(), q.boundary_coeffs(&geom))];
    if !sec {
        for (k, name) in [(0usize, "u_x"), (1, "u_y")] {
            fields.push((
                name,
                u.values.iter().map(|v| v[k]).collect(),
                u.boundary_coeffs(&geom, k),
            ));
        }
    }
    for (name, x, bc) in fields {
        let (gp, go) = (budget(&geom, &pair, &x, &bc), budget(&geom, &owner, &x, &bc));
        println!("G2r {mesh:?} {name}: pair {gp:.3e} (limit 1e-13), owner {go:.3e} (must be >= 1e-5)");
        assert!(gp <= 1e-13, "G2r {mesh:?} {name}: pair {gp:.3e}");
        assert!(
            go >= 1e-5,
            "G2r {mesh:?} {name}: the owner control did not fire, {go:.3e}"
        );
    }
}

/// F2r / G2r: one face viscosity per pair for the viscosity law, the D1 bits, kept by the relaxation; the budget closes.
#[test]
fn f2r_g2r_viscosity_law_face_values_and_budget() {
    for mesh in [Mesh::Box, Mesh::Sec, Mesh::GBox] {
        f2r_g2r(mesh);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// V0c': the explicit default, and pair without a seam
// ---------------------------------------------------------------------------------------------------------------

/// V0c': every V0b' run with `owner` written gives its record; NPK with `pair` gives the NPK record.
#[test]
fn v0c_explicit_owner_and_no_seam() {
    let mut bad = Vec::new();
    for (name, text, n, want) in v0_cases(&seam("owner")) {
        let got = record(&text, n);
        println!(
            "V0c1' {name} with owner written: ({}, {}, 0x{:016x})",
            got.0, got.1, got.2
        );
        if got != want {
            bad.push(format!("{name}: {got:?} against {want:?}"));
        }
    }
    let c2 = record(&npk(&seam("pair")), 50);
    println!("V0c2' NPK with pair: ({}, {}, 0x{:016x})", c2.0, c2.1, c2.2);
    if c2 != V0B[9] {
        bad.push(format!("NPK pair: {c2:?} against {:?}", V0B[9]));
    }
    assert!(bad.is_empty(), "V0c': {bad:#?}");
}
