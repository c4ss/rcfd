//! F-672 (ADR 0799): the whirl paths and `full_rotor` take a beam sector with the shells option alone; `CyclicOptions::beams`
//! and `CyclicOptions::full_rotor_beams` are accepted and have no effect. Checks E-W, E-F, E-WS, E-FS, A-T1, N-T1, A-T2,
//! N-T2, A-T3, N-T3 and N-F of the registration (B-M and B-D are `sa_beam_opt_ins_default_f672_bits.rs`; B1 to B3 the
//! existing tests and `baseline_hashes`). The retired records restated here: `a3_nc1_nc2_nc4_restated_and_n_x_n_w` of
//! `sa_cyclic_beams_default_adr0744.rs` (T1), `nc1_nc2_refusals` of `sa_cyclic_beams_full_rotor_f674.rs` (T2) and
//! `nc_refusals_without_the_options` of `sa_rotating_cyclic_shells.rs` (T3, in the module `t3`). The builders are those of
//! `sa_rotating_cyclic_beams_f675.rs`, `sa_cyclic_beams_full_rotor_f674.rs` and `sa_rotating_cyclic_shells.rs`, copied.

use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::rotating::{RotatingOptions, RotatingSector, SparseStateOptions};
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie::{TieMethod, TieOptions};
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
/// The frustum of ADR 0556: columns and rows of the sector.
const NC: usize = 2;
const NA: usize = 3;

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

/// The names of the copied bodies.
fn beam_options() -> CyclicOptions {
    options_b()
}

fn default_options() -> CyclicOptions {
    options_a()
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

/// The ADR 0556 D3 frustum point at column `i` and generator parameter `s`.
fn frustum_point(i: usize, s: f64) -> Vec3 {
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    p(phi, 1.0 - 0.4 * s, 0.8 * s)
}

/// The Shell4 frustum sector of ADR 0556 (no beams) with the cyclic constraint of `options`.
fn shell_sector(options: CyclicOptions) -> SolidModel {
    let id = |i: usize, a: usize| a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..=NC {
            nodes.push(frustum_point(i, a as f64 / NA as f64));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            conn.extend([id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    mesh.add_element_set("all", (0..NA * NC).collect()).unwrap();
    mesh.add_node_set("hub", (0..=NC).map(|i| id(i, 0)).collect()).unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The full-rotor model of ADR 0601: the mesh of a rotor with a node set `r<p>` on every node, the material and the clamp.
fn rotor_model(mesh: &SolidMesh) -> SolidModel {
    let mut mesh = mesh.clone();
    for p in 0..mesh.nodes().len() {
        mesh.add_node_set(&format!("r{p}"), vec![p]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
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

/// The full rotor of a sector model and its expansions at `k = 1`: (name, `Debug` text).
fn rotor_texts(m: &SolidModel) -> Vec<(String, String)> {
    let rotor = match full_rotor(m) {
        Ok(r) => r,
        Err(e) => return vec![("full_rotor".to_string(), format!("Err({e:?})"))],
    };
    let mut out = vec![("full_rotor".to_string(), format!("Ok({rotor:?})"))];
    let fmodel = rotor_model(rotor.mesh());
    let (sd, fd) = (m.dof_map(), fmodel.dof_map());
    let text = match modal_analysis_harmonic(m, &[], 1, &modal_options(4)) {
        Ok(h) => format!("Ok({:?})", rotor.expand(&sd, &fd, 1, &h.modes[0])),
        Err(e) => format!("Err({e:?})"),
    };
    out.push(("expand k = 1, first mode".to_string(), text));
    let n = sd.n_equations();
    let fa: Vec<f64> = (0..n).map(|e| (0.37 * e as f64 + 0.1).sin()).collect();
    let fb: Vec<f64> = (0..n).map(|e| (0.53 * e as f64 + 0.4).cos()).collect();
    out.push((
        "expand_load k = 1".to_string(),
        format!("{:?}", rotor.expand_load(&sd, &fd, 1, &fa, &fb)),
    ));
    out.push((
        "expand_field k = 1".to_string(),
        format!("{:?}", rotor.expand_field(&sd, &fd, 1, &fa, &fb)),
    ));
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

// ---------------------------------------------------------------------------------------------- E-W, E-F, E-WS, E-FS

#[test]
fn e_w_whirl_paths_of_the_beam_sector_without_the_options() {
    let texts = [options_a(), options_b(), options_c()].map(|o| whirl_texts(&sector(o)));
    let fails = same_texts("E-W", [&texts[0], &texts[1], &texts[2]], true);
    assert!(fails.is_empty(), "{fails:?}");
}

#[test]
fn e_f_full_rotor_of_the_beam_sector_without_the_options() {
    let texts = [options_a(), options_b(), options_c()].map(|o| rotor_texts(&sector(o)));
    let fails = same_texts("E-F", [&texts[0], &texts[1], &texts[2]], true);
    assert!(fails.is_empty(), "{fails:?}");
}

#[test]
fn e_ws_e_fs_the_options_on_a_shell_sector_change_no_bit() {
    let w = [options_a(), options_b(), options_c()].map(|o| whirl_texts(&shell_sector(o)));
    let mut fails = same_texts("E-WS", [&w[0], &w[1], &w[2]], false);
    let f = [options_a(), options_b(), options_c()].map(|o| rotor_texts(&shell_sector(o)));
    fails.extend(same_texts("E-FS", [&f[0], &f[1], &f[2]], false));
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- T1 restated

fn names(r: &Result<(), FemError>, adr: &str) -> bool {
    matches!(r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains(adr))
}

/// `a3_nc1_nc2_nc4_restated_and_n_x_n_w` of `sa_cyclic_beams_default_adr0744.rs` restated by ADR 0799: NC1, NC2, A3 and N-X
/// as they were (their bodies copied); NC4 and N-W as acceptance.
#[test]
fn r_t1_nc4_n_w_as_acceptance_nc1_nc2_a3_n_x_as_before() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // NC1 (amended): beams without shells, refused by add_constraint.
    let mut m = SolidModel::new(sector(beam_options()).mesh().clone());
    m.add_material("all", mat()).unwrap();
    let r = m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().beams(true),
    ));
    hold("NC1 add_constraint", names(&r, "ADR 0595"), format!("{r:?}"));
    // NC2: beams with axis nodes or interpolation.
    let m = sector(beam_options().shell_extensions(true).axis_nodes(true));
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold(
        "NC2 axis nodes: solve_static_mpc",
        names(&r, "ADR 0595"),
        format!("{r:?}"),
    );
    let m = sector(
        beam_options()
            .shell_extensions(true)
            .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
    );
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold(
        "NC2 interpolation: solve_static_mpc",
        names(&r, "ADR 0595"),
        format!("{r:?}"),
    );
    // A3 (NC3 restated): the shells option alone is taken.
    let m = sector(default_options());
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("A3 shells only: solve_static_mpc", r.is_ok(), format!("{r:?}"));
    let r = modal_analysis_harmonic(&m, &[], 0, &modal_options(2)).map(|h| h.frequencies);
    hold(
        "A3 shells only: modal_analysis_harmonic k = 0",
        r.is_ok(),
        format!("{r:?}"),
    );
    // NC4 as acceptance (ADR 0799): full_rotor takes Beam2 with the beams option and with the shells option alone, with
    // the bits of the full-rotor option.
    let reference = format!("{:?}", full_rotor(&sector(options_c())));
    for (label, options) in [("beams option", beam_options()), ("shells only", default_options())] {
        let r = full_rotor(&sector(options));
        let same = format!("{r:?}") == reference;
        hold(
            &format!("A-NC4 full_rotor, {label}"),
            r.is_ok() && same,
            format!("Ok {}, equal to (c) {same}", r.is_ok()),
        );
    }
    // N-X: beam ends on extended boundaries stay refused without the beams option (ADR 0744 D2).
    for (label, options) in [
        ("axis nodes", default_options().shell_extensions(true).axis_nodes(true)),
        (
            "interpolation",
            default_options()
                .shell_extensions(true)
                .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
        ),
    ] {
        let m = sector(options);
        let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
        hold(
            &format!("N-X {label}: solve_static_mpc"),
            names(&r, "ADR 0744"),
            format!("{r:?}"),
        );
        let r = modal_analysis_harmonic(&m, &[], 1, &modal_options(2)).map(|_| ());
        hold(
            &format!("N-X {label}: modal_analysis_harmonic k = 1"),
            names(&r, "ADR 0744"),
            format!("{r:?}"),
        );
    }
    // N-W as acceptance (ADR 0799): the whirl paths take the beam sector with the shells option alone.
    let whirl = |options: CyclicOptions| {
        let m = sector(options);
        let sec = RotatingSector::new(&m, &rotating_options(4).coriolis(true)).unwrap();
        sec.whirl_modes(1, 30.0)
    };
    let (ra, rb) = (whirl(default_options()), whirl(beam_options()));
    let same = format!("{ra:?}") == format!("{rb:?}");
    hold(
        "A-NW whirl_modes k = 1, shells only",
        ra.is_ok() && same,
        format!("Ok {}, equal to (b) {same}", ra.is_ok()),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- T2 restated

/// `nc1_nc2_refusals` of `sa_cyclic_beams_full_rotor_f674.rs` restated by ADR 0799: NC1 as acceptance, NC2 as it was.
#[test]
fn r_t2_nc1_as_acceptance_nc2_as_before() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // NC1 as acceptance: beams without full_rotor_beams: full_rotor takes Beam2 with the bits of the option.
    let r = full_rotor(&sector(beam_options()));
    let same = format!("{r:?}") == format!("{:?}", full_rotor(&sector(options_c())));
    hold(
        "A-601 full_rotor without full_rotor_beams",
        r.is_ok() && same,
        format!("Ok {}, equal to (c) {same}", r.is_ok()),
    );
    // NC2: full_rotor_beams without beams, refused by add_constraint.
    let mut m = SolidModel::new(wheel_mesh());
    m.add_material("all", mat()).unwrap();
    let r = m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true).full_rotor_beams(true),
    ));
    hold(
        "NC2 add_constraint",
        matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("ADR 0601")),
        format!("{r:?}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- T3 restated, N-F

/// The builders of `sa_rotating_cyclic_shells.rs` (ADR 0561), copied, and its `nc_refusals_without_the_options` restated.
mod t3 {
    use rcfd_fem::cyclic::{full_rotor, rotation_matrix, FullRotor, Mat3};
    use rcfd_fem::modal::ModalOptions;
    use rcfd_fem::model::CyclicOptions;
    use rcfd_fem::rotating::{coriolis_matrix, RotatingOptions, RotatingPart};
    use rcfd_fem::{
        BeamSection, BlockProperty, Constraint, ElementKind, FaceRef, FemError, Material, SolidMesh, SolidModel,
    };
    use rcfd_linalg::eigen::ResidualTest;
    use std::f64::consts::PI;

    type Vec3 = [f64; 3];

    const E: f64 = 2.0e11;
    const NU: f64 = 0.3;
    const RHO: f64 = 7850.0;
    const N: usize = 6;
    const NC: usize = 2;
    const NA: usize = 3;
    const T: f64 = 0.05;
    const ORIGIN: Vec3 = [0.3, -0.2, 0.5];

    fn axis() -> Vec3 {
        [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
    }

    fn mat() -> Material {
        Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
    }

    fn cross(a: Vec3, b: Vec3) -> Vec3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    /// The point of the ADR 0556 D3 frustum at column `i` (of `N NC` around) and generator parameter `s`, radial offset
    /// `dr`.
    fn point(i: usize, s: f64, dr: f64) -> Vec3 {
        let n = axis();
        let l = 5.0f64.sqrt();
        let ea = [2.0 / l, -1.0 / l, 0.0];
        let eb = cross(n, ea);
        let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
        let r = 1.0 - 0.4 * s + dr;
        let z = 0.8 * s;
        let (sp, cp) = phi.sin_cos();
        [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
    }

    fn modal_options(modes: usize) -> ModalOptions {
        let mut o = ModalOptions::new(modes);
        o.tolerance = 1e-12;
        o.residual_test = ResidualTest::Backward;
        o
    }

    fn met(ok: bool) -> &'static str {
        if ok {
            "met"
        } else {
            "MISSED"
        }
    }

    struct Record {
        fails: Vec<String>,
    }

    impl Record {
        fn new() -> Record {
            Record { fails: Vec::new() }
        }
        fn hold(&mut self, name: &str, ok: bool, detail: String) {
            println!("{name}: {detail} {}", met(ok));
            if !ok {
                self.fails.push(format!("{name}: {detail}"));
            }
        }
        fn done(self) {
            assert!(self.fails.is_empty(), "missed: {:#?}", self.fails);
        }
    }

    /// The shell mesh of ADR 0556: `columns` columns (`periodic`: closing on column 0), node `(i, a)` at `a columns + i`.
    fn shell_mesh(kind: ElementKind, columns: usize, periodic: bool) -> SolidMesh {
        let id = |i: usize, a: usize| a * columns + i % columns;
        let mut nodes = Vec::new();
        for a in 0..=NA {
            for i in 0..columns {
                nodes.push(point(i, a as f64 / NA as f64, 0.0));
            }
        }
        let quads = if periodic { columns } else { columns - 1 };
        let mut conn = Vec::new();
        for a in 0..NA {
            for i in 0..quads {
                conn.extend([id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)]);
            }
        }
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(kind, conn).unwrap();
        mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
        let ne = mesh.blocks()[0].len();
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        mesh.add_node_set("clamp", (0..columns).map(|i| id(i, 0)).collect())
            .unwrap();
        mesh
    }

    fn sector_node(i: usize, a: usize) -> usize {
        a * (NC + 1) + i
    }

    fn clamp(m: &mut SolidModel) {
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
        }
    }

    fn shell_options() -> CyclicOptions {
        CyclicOptions::new().shells(true)
    }

    /// The D3 sector with node sets `low` and `high`, the clamp and the cyclic constraint with the shells option (Shell4).
    fn sector(kind: ElementKind) -> SolidModel {
        let mut mesh = shell_mesh(kind, NC + 1, false);
        mesh.add_node_set("low", (0..=NA).map(|a| sector_node(0, a)).collect())
            .unwrap();
        mesh.add_node_set("high", (0..=NA).map(|a| sector_node(NC, a)).collect())
            .unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", mat()).unwrap();
        clamp(&mut m);
        m.add_constraint(Constraint::cyclic_with(
            "low",
            "high",
            axis(),
            ORIGIN,
            N,
            shell_options(),
        ))
        .unwrap();
        m
    }

    /// The reference ring: `full_rotor` of the sector, a node set `r<p>` on every node, the material and the clamp (D3).
    fn ring_of(sector: &SolidModel) -> (FullRotor, SolidModel) {
        let rotor = full_rotor(sector).unwrap();
        let mut mesh = rotor.mesh().clone();
        for p in 0..mesh.nodes().len() {
            mesh.add_node_set(&format!("r{p}"), vec![p]).unwrap();
        }
        let mut m = SolidModel::new(mesh);
        m.add_material("all", mat()).unwrap();
        clamp(&mut m);
        (rotor, m)
    }

    fn rot(j: usize) -> Mat3 {
        rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
    }

    fn apply(r: &Mat3, v: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
    }

    /// The Hex8 twin sector of ADR 0556 B3: surface sets `low` / `high`, node sets `low_nodes` / `high_nodes`, `clamp`,
    /// `all`.
    fn solid_sector() -> SolidMesh {
        let id = |i: usize, a: usize, w: usize| w * (NA + 1) * (NC + 1) + a * (NC + 1) + i;
        let mut nodes = Vec::new();
        for w in 0..2 {
            for a in 0..=NA {
                for i in 0..=NC {
                    nodes.push(point(i, a as f64 / NA as f64, (w as f64 - 0.5) * 2.0 * T));
                }
            }
        }
        let mut conn = Vec::new();
        for a in 0..NA {
            for i in 0..NC {
                for w in 0..2 {
                    conn.extend([id(i, a, w), id(i + 1, a, w), id(i + 1, a + 1, w), id(i, a + 1, w)]);
                }
            }
        }
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, conn).unwrap();
        let ne = NA * NC;
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        let col = |i: usize| -> Vec<usize> {
            let mut v: Vec<usize> = (0..2).flat_map(|w| (0..=NA).map(move |a| id(i, a, w))).collect();
            v.sort_unstable();
            v
        };
        for (name, i) in [("low", 0), ("high", NC)] {
            let nodes = col(i);
            let mut faces = Vec::new();
            for e in 0..ne {
                for f in 0..6 {
                    let fr = FaceRef { element: e, face: f };
                    if mesh.face_nodes(fr).iter().all(|p| nodes.binary_search(p).is_ok()) {
                        faces.push(fr);
                    }
                }
            }
            mesh.add_surface_set(name, faces).unwrap();
            mesh.add_node_set(&format!("{name}_nodes"), nodes).unwrap();
        }
        let mut clamp: Vec<usize> = (0..2).flat_map(|w| (0..=NC).map(move |i| id(i, 0, w))).collect();
        clamp.sort_unstable();
        mesh.add_node_set("clamp", clamp).unwrap();
        mesh
    }

    /// The Hex8 twin sector with one disjoint element of `kind` (as NC2 of ADR 0561), the default options.
    fn twin_with(kind: ElementKind) -> SolidModel {
        let twin = solid_sector();
        let nh = twin.nodes().len();
        let mut nodes = twin.nodes().to_vec();
        nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 1.0, 0.0], [3.0, 1.0, 0.0]]);
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, twin.blocks()[0].connectivity().to_vec())
            .unwrap();
        if kind == ElementKind::Shell4 {
            mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
                .unwrap();
            mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
        } else {
            mesh.add_block(ElementKind::Beam2, vec![nh, nh + 1]).unwrap();
            let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
            mesh.set_block_property(1, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
                .unwrap();
        }
        let ne = twin.n_elements();
        mesh.add_element_set("all", (0..=ne).collect()).unwrap();
        for name in ["low", "high"] {
            mesh.add_surface_set(name, twin.surface_set(name).unwrap().to_vec())
                .unwrap();
        }
        let mut m2 = SolidModel::new(mesh);
        m2.add_material("all", mat()).unwrap();
        m2.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N))
            .unwrap();
        m2
    }

    /// `nc_refusals_without_the_options` of `sa_rotating_cyclic_shells.rs` restated by ADR 0799: NC1 as acceptance, NC2,
    /// NC3 and NC4 as they were.
    #[test]
    fn r_t3_nc1_as_acceptance_nc2_to_nc4_as_before() {
        let mut rec = Record::new();
        // NC1 as acceptance (ADR 0799): full_rotor of a Beam2 model with the shells option.
        let mut mesh = SolidMesh::new(vec![point(0, 0.5, 0.0), point(1, 0.5, 0.0), point(2, 0.5, 0.0)]).unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
        let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section, axis()).unwrap())
            .unwrap();
        mesh.add_element_set("all", vec![0, 1]).unwrap();
        mesh.add_node_set("low", vec![0]).unwrap();
        mesh.add_node_set("high", vec![2]).unwrap();
        let mut mb = SolidModel::new(mesh);
        mb.add_material("all", mat()).unwrap();
        mb.add_constraint(Constraint::cyclic_with(
            "low",
            "high",
            axis(),
            ORIGIN,
            N,
            shell_options(),
        ))
        .unwrap();
        match full_rotor(&mb) {
            Ok(rotor) => {
                let blocks = rotor.mesh().blocks();
                rec.hold(
                    "A-T3 full_rotor Ok, N blocks",
                    blocks.len() == N,
                    format!("{} blocks", blocks.len()),
                );
                let mut structure = true;
                let mut copy0 = true;
                let mut worst: f64 = 0.0;
                for j in 0..N {
                    let b = rotor.block(j, 0);
                    let Some(block) = blocks.get(b) else {
                        structure = false;
                        continue;
                    };
                    structure &= b == j && block.kind() == ElementKind::Beam2 && block.len() == 2;
                    match block.property() {
                        Some(BlockProperty::Beam {
                            section: s,
                            orientation,
                            ..
                        }) => {
                            structure &= format!("{s:?}") == format!("{section:?}");
                            if j == 0 {
                                copy0 &= orientation.iter().zip(axis()).all(|(x, y)| x.to_bits() == y.to_bits());
                            } else {
                                let want = apply(&rot(j), axis());
                                for k in 0..3 {
                                    worst = worst.max((orientation[k] - want[k]).abs());
                                }
                            }
                        }
                        _ => structure = false,
                    }
                }
                rec.hold(
                    "A-T3 block(j, 0) = j, Beam2, 2 elements, the section's bits",
                    structure,
                    String::new(),
                );
                rec.hold("A-T3 copy 0 orientation bits", copy0, String::new());
                rec.hold(
                    "A-T3 copies j >= 1 orientation R^j v",
                    worst <= 1e-15,
                    format!("max |d| {worst:.3e} (limit <= 1e-15)"),
                );
            }
            Err(e) => rec.hold("A-T3 full_rotor Ok, N blocks", false, format!("{e:?}")),
        }
        // NC2 (amendment 1): the Hex8 twin sector with a disjoint Shell4 element, the default options.
        let m2 = twin_with(ElementKind::Shell4);
        let r = full_rotor(&m2).map(|_| ());
        rec.hold(
            "NC2 full_rotor of a Hex8 sector with a Shell4 element, default options",
            matches!(
                r,
                Err(FemError::UnsupportedKind {
                    kind: "Shell4",
                    operation: "full_rotor",
                    ..
                })
            ),
            format!("{r:?}"),
        );
        // NC3: the ring as a part with the default rotating options: the whirl refuses the shell as before.
        let m = sector(ElementKind::Shell4);
        let (_, ring) = ring_of(&m);
        let part = RotatingPart::new(&ring, axis(), ORIGIN, &RotatingOptions::new(4).modal(modal_options(4))).unwrap();
        let r = part.whirl_modes(100.0).map(|_| ());
        let shell4 = |r: &Result<(), FemError>| match r {
            Err(FemError::Element { source, .. }) => {
                matches!(**source, FemError::NotYetSupported { variant: "Shell4", .. })
            }
            _ => false,
        };
        rec.hold("NC3 ring whirl with the default options", shell4(&r), format!("{r:?}"));
        // NC4: the public coriolis_matrix of the shell sector.
        let r = coriolis_matrix(&m, axis(), None).map(|_| ());
        rec.hold("NC4 coriolis_matrix of the shell sector", shell4(&r), format!("{r:?}"));
        rec.done();
    }

    /// N-F (ADR 0799 D2 case 3): Beam2 without the shells option stays refused by `full_rotor`.
    #[test]
    fn n_f_full_rotor_of_beam2_without_the_shells_option_is_refused() {
        let r = full_rotor(&twin_with(ElementKind::Beam2)).map(|_| ());
        let ok = matches!(
            r,
            Err(FemError::UnsupportedKind {
                kind: "Beam2",
                operation: "full_rotor",
                ..
            })
        );
        println!(
            "N-F full_rotor of a Hex8 sector with a Beam2 element, default options: {r:?} {}",
            met(ok)
        );
        assert!(ok, "N-F");
    }
}
