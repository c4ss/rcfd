//! ADR 0227 (F-302), checks of `turbulence.periodic_seam.diffusivity = "pair"` for the property viscosity and the user-function
//! viscosity, and of k-epsilon and Spalart-Allmaras with `pair` on `pimple`: F2p / G2p and F2u / G2u / F2x the face values and the
//! global diffusion budgets of the two laminar laws, S1p the scope of the laminar runs, S1t the scope of ADR 0210 S1 restored, T4p,
//! T4u, T4k the solver's momentum face viscosity on the seam, T5k the laminar rule across the seam on `pimple`, R0p a report, and
//! V0c'' the explicit default. The limits are those registered in ADR 0227 before this code ran.

mod f295_common;
mod f296_common;
mod f302_common;

use f296_common::{chxr, seam, LAMINAR, NU_CH};
use f302_common::{
    case_dir, chxa, chxk, chxp, chxu, chxut, gchxp, lamxp, record_in, sim_in, v0_cases, wasm, LAW_P, P_COEFFS, UT, UX,
    V0B,
};
use rcfd_api::case::Case;
use rcfd_api::properties::Properties;
use rcfd_api::sim::Simulation;
use rcfd_api::udf::{face_viscosity, Udf};
use rcfd_core::materials::PropertyLaw;
use rcfd_core::vec3;
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_3d_sector, box_2d, box_2d_graded, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const KEY: &str = "turbulence.periodic_seam.diffusivity";

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

/// The weight `w = dn_g / (dn_f + dn_g)` of the owner of `f`.
fn weight(geom: &FvGeometry, f: usize, g: usize) -> f64 {
    let (dn_f, dn_g) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
    dn_g / (dn_f + dn_g)
}

/// The property law of CHXP evaluated as `Properties::nu_faces` does (`rho = 1`).
fn mu_p(t: f64) -> f64 {
    PropertyLaw::Polynomial(P_COEFFS.to_vec()).eval(t) / 1.0
}

// ---------------------------------------------------------------------------------------------------------------
// F2p, G2p, F2u, G2u, F2x: the library face values
// ---------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mesh {
    Box,
    Sec,
    GBox,
}

const MESHES: [Mesh; 3] = [Mesh::Box, Mesh::Sec, Mesh::GBox];

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

/// The temperature of the library checks (ADR 0227 item 8): `300 + 20 sin(2 pi s) + 5 y` (SEC: `+ 5 z`).
fn temperature(geom: &FvGeometry, mesh: Mesh) -> ScalarField {
    let sec = mesh == Mesh::Sec;
    ScalarField::from_fn("T", geom, |p| {
        300.0 + 20.0 * (2.0 * PI * s_of(mesh, p)).sin() + 5.0 * if sec { p[2] } else { p[1] }
    })
}

/// The velocity of ADR 0212 F2r: `(sin(2 pi s) + 0.1 y, 0.5 sin(2 pi s + 1), 0.3 z)`.
fn velocity(geom: &FvGeometry, mesh: Mesh) -> VectorField {
    VectorField::from_fn("U", geom, |c| {
        let s = s_of(mesh, c);
        [
            (2.0 * PI * s).sin() + 0.1 * c[1],
            0.5 * (2.0 * PI * s + 1.0).sin(),
            0.3 * c[2],
        ]
    })
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

/// The G fields: `q` (every mesh), `u_x` and `u_y` of the velocity (BOX, GBOX).
fn g_fields(geom: &FvGeometry, mesh: Mesh) -> Vec<(&'static str, Vec<f64>, Vec<rcfd_fv::FaceCoeffs>)> {
    let sec = mesh == Mesh::Sec;
    let q = ScalarField::from_fn("q", geom, |p| {
        1.0 + 0.5 * (2.0 * PI * s_of(mesh, p)).sin() + 0.2 * if sec { p[2] } else { p[1] }
    });
    let mut fields = vec![("q", q.values.clone(), q.boundary_coeffs(geom))];
    if !sec {
        let u = velocity(geom, mesh);
        for (k, name) in [(0usize, "u_x"), (1, "u_y")] {
            fields.push((
                name,
                u.values.iter().map(|v| v[k]).collect(),
                u.boundary_coeffs(geom, k),
            ));
        }
    }
    fields
}

/// The pair comparison of F2p and F2u: (pairs with unequal bits, pairs off the D1 bits, faces off the seam changed, largest owner
/// pair difference of the largest owner face value, owner pairs with unequal bits).
fn compare(
    geom: &FvGeometry,
    owner: &[f64],
    pair: &[f64],
    d1: impl Fn(usize, usize) -> f64,
) -> (usize, usize, usize, f64, usize) {
    let seam = seam_faces(geom);
    let mut on_seam = vec![false; geom.mesh.n_faces()];
    let (mut unequal, mut not_d1, mut owner_unequal) = (0, 0, 0);
    let mut owner_diff: f64 = 0.0;
    for &(f, g) in &seam {
        on_seam[f] = true;
        on_seam[g] = true;
        if pair[f].to_bits() != pair[g].to_bits() {
            unequal += 1;
        }
        if pair[f].to_bits() != d1(f, g).to_bits() {
            not_d1 += 1;
        }
        if owner[f].to_bits() != owner[g].to_bits() {
            owner_unequal += 1;
        }
        owner_diff = owner_diff.max((owner[f] - owner[g]).abs());
    }
    let changed = (0..geom.mesh.n_faces())
        .filter(|&x| !on_seam[x] && pair[x].to_bits() != owner[x].to_bits())
        .count();
    let big = owner.iter().cloned().fold(0.0, f64::max);
    (unequal, not_d1, changed, owner_diff / big, owner_unequal)
}

/// The law of CHXP.
fn law_p() -> Properties {
    let case = Case::parse_toml(&chxp("")).unwrap_or_else(|e| panic!("{e:?}"));
    Properties::from_case(&case).expect("a property law")
}

/// F2p / G2p: the property viscosity takes one value per pair with `pair`, the D1 bits; by default it is already one value per pair
/// up to rounding (item 2), so its budget closes in both modes.
#[test]
fn f2p_g2p_property_viscosity_face_values_and_budget() {
    for mesh in MESHES {
        let geom = geometry(mesh);
        let t = temperature(&geom, mesh);
        let owner = law_p().nu_faces(&geom, &t).unwrap();
        let mut lp = law_p();
        lp.set_seam_pairs(Some(geom.periodic_pairs().unwrap()));
        let pair = lp.nu_faces(&geom, &t).unwrap();
        let own = geom.mesh.owner();
        let d1 = |f: usize, g: usize| {
            let w = weight(&geom, f, g);
            mu_p(w * t.values[own[f]] + (1.0 - w) * t.values[own[g]])
        };
        let (unequal, not_d1, changed, owner_rel, owner_unequal) = compare(&geom, &owner, &pair, d1);
        let n = seam_faces(&geom).len();
        println!(
            "F2p {mesh:?}: {n} pairs; pair: {unequal} with unequal bits, {not_d1} not the D1 bits, {changed} other faces changed; owner (the finding of item 2): {owner_unequal} pairs with unequal bits, largest pair difference {owner_rel:.4e} of the largest face viscosity (limit 1e-14)"
        );
        assert!(n > 0, "F2p {mesh:?}: no seam");
        assert_eq!(unequal, 0, "F2p {mesh:?}: pairs with unequal bits");
        assert_eq!(not_d1, 0, "F2p {mesh:?}: pairs off the D1 bits");
        assert_eq!(changed, 0, "F2p {mesh:?}: faces off the seam changed");
        assert!(owner_rel <= 1e-14, "F2p {mesh:?}: owner {owner_rel:.3e}");
        for (name, x, bc) in g_fields(&geom, mesh) {
            let (gp, go) = (budget(&geom, &pair, &x, &bc), budget(&geom, &owner, &x, &bc));
            println!("G2p {mesh:?} {name}: pair {gp:.3e} (limit 1e-13), owner {go:.3e} (limit 1e-13, the finding)");
            assert!(gp <= 1e-13, "G2p {mesh:?} {name}: pair {gp:.3e}");
            assert!(go <= 1e-13, "G2p {mesh:?} {name}: owner {go:.3e}");
        }
    }
}

fn udf_of(text: &str) -> Udf {
    Udf::load("v", &wasm(text), 1_000_000).unwrap()
}

/// The D1 value of a module on the pair `(f, g)`: the module at the centre of `face` with the interpolated temperature and speed.
fn udf_d1(m: &mut Udf, geom: &FvGeometry, u: &[[f64; 3]], t: Option<&[f64]>, f: usize, g: usize, face: usize) -> f64 {
    let own = geom.mesh.owner();
    let w = weight(geom, f, g);
    let (a, b) = (own[f], own[g]);
    let speed = w * vec3::norm(u[a]) + (1.0 - w) * vec3::norm(u[b]);
    let temp = t.map_or(0.0, |t| w * t[a] + (1.0 - w) * t[b]);
    m.kinematic_viscosity(geom.mesh.face_centres()[face], 0.0, temp, speed)
        .unwrap()
}

/// F2u / G2u: the user-function viscosity takes one value per pair with `pair`, the D1 bits; the owner values differ across the
/// seam and miss the budget.
#[test]
fn f2u_g2u_user_function_viscosity_face_values_and_budget() {
    for mesh in MESHES {
        let geom = geometry(mesh);
        let t = temperature(&geom, mesh);
        let u = velocity(&geom, mesh);
        let owner = face_viscosity(&mut udf_of(UT), &geom, &u.values, Some(&t.values), None).unwrap();
        let pairs = geom.periodic_pairs().unwrap();
        let pair = face_viscosity(
            &mut udf_of(UT),
            &geom,
            &u.values,
            Some(&t.values),
            Some(pairs.as_slice()),
        )
        .unwrap();
        let m = std::cell::RefCell::new(udf_of(UT));
        let d1 = |f: usize, g: usize| udf_d1(&mut m.borrow_mut(), &geom, &u.values, Some(&t.values), f, g, f);
        let (unequal, not_d1, changed, owner_rel, owner_unequal) = compare(&geom, &owner, &pair, d1);
        println!(
            "F2u {mesh:?}: {} pairs; pair: {unequal} with unequal bits, {not_d1} not the D1 bits, {changed} other faces changed; owner: {owner_unequal} pairs with unequal bits, largest pair difference {owner_rel:.4e} of the largest face viscosity (must be >= 1e-3)",
            pairs.len()
        );
        assert!(!pairs.is_empty(), "F2u {mesh:?}: no seam");
        assert_eq!(unequal, 0, "F2u {mesh:?}: pairs with unequal bits");
        assert_eq!(not_d1, 0, "F2u {mesh:?}: pairs off the D1 bits");
        assert_eq!(changed, 0, "F2u {mesh:?}: faces off the seam changed");
        assert!(owner_rel >= 1e-3, "F2u {mesh:?}: vacuous, {owner_rel:.3e}");
        for (name, x, bc) in g_fields(&geom, mesh) {
            let (gp, go) = (budget(&geom, &pair, &x, &bc), budget(&geom, &owner, &x, &bc));
            println!("G2u {mesh:?} {name}: pair {gp:.3e} (limit 1e-13), owner {go:.3e} (must be >= 1e-5)");
            assert!(gp <= 1e-13, "G2u {mesh:?} {name}: pair {gp:.3e}");
            assert!(
                go >= 1e-5,
                "G2u {mesh:?} {name}: the owner control did not fire, {go:.3e}"
            );
        }
    }
}

/// F2x: a module that depends on position takes its pair value at the centre of the pair's face of lower index (D1).
#[test]
fn f2x_user_function_viscosity_position_on_the_pair() {
    let mesh = Mesh::Box;
    let geom = geometry(mesh);
    let t = temperature(&geom, mesh);
    let u = velocity(&geom, mesh);
    let owner = face_viscosity(&mut udf_of(UX), &geom, &u.values, Some(&t.values), None).unwrap();
    let pairs = geom.periodic_pairs().unwrap();
    let pair = face_viscosity(
        &mut udf_of(UX),
        &geom,
        &u.values,
        Some(&t.values),
        Some(pairs.as_slice()),
    )
    .unwrap();
    let mut m = udf_of(UX);
    let (mut unequal, mut not_d1) = (0, 0);
    let (mut seen, mut owner_diff, mut big) = (f64::INFINITY, 0.0_f64, 0.0_f64);
    for &(f, g) in &seam_faces(&geom) {
        let at_f = udf_d1(&mut m, &geom, &u.values, Some(&t.values), f, g, f);
        let at_g = udf_d1(&mut m, &geom, &u.values, Some(&t.values), f, g, g);
        if pair[f].to_bits() != pair[g].to_bits() {
            unequal += 1;
        }
        if pair[f].to_bits() != at_f.to_bits() {
            not_d1 += 1;
        }
        seen = seen.min((at_g - at_f).abs() / at_f.abs());
        owner_diff = owner_diff.max((owner[f] - owner[g]).abs());
        big = big.max(owner[f]).max(owner[g]);
    }
    let big_all = owner.iter().cloned().fold(0.0, f64::max).max(big);
    println!(
        "F2x {mesh:?}: {} pairs; pair: {unequal} with unequal bits, {not_d1} not the value at x_f; the value at x_g differs by at least {seen:.4e} of it (must be >= 1e-3); owner: largest pair difference {:.4e} of the largest face viscosity (must be >= 1e-3)",
        pairs.len(),
        owner_diff / big_all
    );
    assert!(!pairs.is_empty(), "F2x: no seam");
    assert_eq!(unequal, 0, "F2x: pairs with unequal bits");
    assert_eq!(not_d1, 0, "F2x: pairs off the value at x_f");
    assert!(seen >= 1e-3, "F2x: the face is not seen, {seen:.3e}");
    assert!(owner_diff / big_all >= 1e-3, "F2x: vacuous");
}

// ---------------------------------------------------------------------------------------------------------------
// S1p, S1t: scope
// ---------------------------------------------------------------------------------------------------------------

/// Paths of the diagnostics of a text in the case directory: parse errors, then static errors, then the errors of
/// `Simulation::new`.
fn paths_in(text: &str) -> Vec<String> {
    match Case::parse_toml_in(text, case_dir()) {
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

/// The static diagnostics of a text (ADR 0205 amendment 1).
fn static_errors_in(text: &str) -> Vec<(String, String)> {
    let d = match Case::parse_toml_in(text, case_dir()) {
        Err(d) => d,
        Ok(c) => c.semantic_errors(),
    };
    d.iter().map(|x| (x.path.clone(), x.message.clone())).collect()
}

/// S1p: the laminar runs that accept `pair` and those located at the key.
#[test]
fn s1p_scope_of_the_laminar_runs() {
    use f295_common::{ch, CH_SST, SIMPLEC};
    let pair = seam("pair");
    let accepted = [
        ("CHXP pair", chxp(&pair)),
        ("GCHXP pair", gchxp(&pair)),
        ("CHXU pair", chxu(&pair)),
        ("CHXUT pair", chxut(&pair)),
        ("CHXR pair", chxr(&pair)),
        ("CHXP owner", chxp(&seam("owner"))),
        ("CHXU owner", chxu(&seam("owner"))),
    ];
    for (id, text) in &accepted {
        let p = paths_in(text);
        assert!(p.is_empty(), "S1p {id}: expected no diagnostic, got {p:?}\n{text}");
    }
    println!(
        "S1p: accepted: {}",
        accepted.iter().map(|a| a.0).collect::<Vec<_>>().join(", ")
    );
    let with_model = |text: &str, model: &str| {
        assert!(text.contains(CH_SST), "the CH text changed");
        text.replace(CH_SST, model)
    };
    let swap_law = |law: &str| {
        let t = chxp(&pair);
        assert!(t.contains(LAW_P), "the CHXP text changed");
        t.replace(LAW_P, law)
    };
    let refused = [
        (
            "CH laminar pair",
            with_model(&ch("", &pair, SIMPLEC), LAMINAR),
            &["rheology", "properties.viscosity", "udf"][..],
        ),
        (
            "CHXP constant law pair",
            swap_law("[properties.viscosity]\nlaw = \"constant\"\n"),
            &["properties.viscosity"][..],
        ),
        (
            "CHXP conductivity law pair",
            swap_law("[properties.conductivity]\nlaw = \"polynomial\"\ncoefficients = [0.05]\n"),
            &["properties.viscosity"][..],
        ),
        (
            "CH plugin pair",
            with_model(&ch("", &pair, SIMPLEC), "model = \"plugin\"\nplugin = \"k_epsilon\"\n"),
            &["plugin"][..],
        ),
    ];
    for (id, text, words) in &refused {
        let e = static_errors_in(text);
        let Some((_, msg)) = e.iter().find(|(p, _)| p == KEY) else {
            panic!("S1p {id}: pair not refused statically at {KEY}: {e:?}");
        };
        for w in *words {
            assert!(msg.contains(w), "S1p {id}: the message does not name {w}: {msg}");
        }
        println!("S1p: {id} located at {KEY}: {msg}");
    }
}

/// The scope of ADR 0210 S1 and amendment 1, as registered there (`f294_transient_keps_sa.rs`, item 6 of ADR 0227), with the
/// texts and functions of that test copied unchanged.
mod adr0210 {
    use rcfd_api::case::Case;
    use rcfd_api::sim::Simulation;
    use rcfd_fv::FvGeometry;
    use rcfd_mesh::gen::box_2d;
    use rcfd_turb::{
        create_model, KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode, TurbulenceModel,
        WallTreatment,
    };
    use std::sync::Arc;

    pub fn paths(text: &str) -> Vec<String> {
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

    pub fn messages(text: &str) -> Vec<(String, String)> {
        match Case::parse_toml(text) {
            Err(d) => d.iter().map(|x| (x.path.clone(), x.message.clone())).collect(),
            Ok(c) => c
                .semantic_errors()
                .iter()
                .map(|x| (x.path.clone(), x.message.clone()))
                .collect(),
        }
    }

    pub fn has(text: &str, path: &str) {
        let p = paths(text);
        assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
    }

    pub fn ok(text: &str) {
        let p = paths(text);
        assert!(p.is_empty(), "expected no diagnostic, got {p:?}\n{text}");
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Model {
        KEpsilon,
        Sa,
    }

    impl Model {
        fn lines(self) -> &'static str {
            match self {
                Model::KEpsilon => "model = \"k_epsilon\"\nk_initial = 1.0\nepsilon_initial = 3.6\n",
                Model::Sa => "model = \"spalart_allmaras\"\nnutilde_ratio = 40.0\n",
            }
        }
    }

    const UNIFORM: &str = "velocity = [15.0, 0.0, 0.0]\n";
    pub const DRIVE: &str =
        "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";

    fn ch_with(model: Model, initial: &str, extra: &str, solver: &str) -> String {
        format!(
            r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 20
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.0025316455696202532
rho = 1.0
[initial]
{initial}[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
{DRIVE}[turbulence]
{}{extra}[solver]
convection = "upwind"
{solver}"#,
            model.lines()
        )
    }

    pub fn ch(model: Model, extra: &str, solver: &str) -> String {
        ch_with(model, UNIFORM, extra, solver)
    }

    pub fn pimple(scheme: &str) -> String {
        format!("algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n")
    }

    pub fn sector(model: Model, moving_outer: bool) -> String {
        let (inlet, turb) = match model {
            Model::KEpsilon => (
                "turbulence_spec = \"values\"\nk_value = 0.01\nepsilon_value = 0.009\n",
                "model = \"k_epsilon\"\n",
            ),
            Model::Sa => ("", "model = \"spalart_allmaras\"\n"),
        };
        let outer = if moving_outer {
            "velocity = \"moving_wall\"\nwall_rotation_rate = -2.0\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n"
        } else {
            "velocity = \"fixed\"\nturbulence = \"wall\"\n"
        };
        format!(
            r#"
[mesh]
generator = "annulus_3d_sector"
nx = 8
ny = 6
nz = 8
r_in = 0.5
r_out = 1.0
lz = 0.5
sector_angle = 40.0
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "2 [1/s] * y + 0.2 [1/s] * x"
velocity_expression_y = "-2 [1/s] * x + 0.2 [1/s] * y"
velocity_expression_z = "1 [m/s] + 1 [1/s] * y"
{inlet}[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
{outer}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[turbulence]
{turb}[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
algorithm = "pimple"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
dt = 0.05
end_time = 100.0
"#
        )
    }

    pub fn registry_model(key: &str) -> Box<dyn TurbulenceModel> {
        let mesh = box_2d(4, 4, 1.0, 1.0, true, false).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let ctx = ModelContext {
            geom,
            nu: 0.01,
            walls: vec!["ymin".into(), "ymax".into()],
            k0: 1.0,
            eps0: 1.0,
            omega0: 10.0,
            nutilde0: 0.03,
            variant: KEpsilonVariant::Standard,
            wall_treatment: WallTreatment::Smooth,
            sa_wall_mode: SaWallMode::Auto,
            sa_limiter: SLimiter::Allmaras,
            sa_negative: true,
            kappa: 0.41,
            e_wall: 9.8,
            convection: rcfd_fv::convection::ConvScheme::Upwind,
            relax_turb: 0.7,
            relax_nut: 0.5,
            wall_damping: false,
            length_scale: 0.0,
            mixing_profile: MixingProfile::Linear,
            time_accurate: true,
        };
        create_model(key, &ctx).unwrap()
    }
}

/// S1t: ADR 0210 S1 and its amendment 1 restored, with the `pair` case accepted (ADR 0212 R1').
#[test]
fn s1t_scope_of_transient_kepsilon_and_sa_restored() {
    use adr0210::Model::{KEpsilon as K, Sa as A};
    use adr0210::{ch, has, messages, ok, pimple, registry_model, sector, DRIVE};
    for m in [K, A] {
        for scheme in ["euler", "bdf2", "bdf2_bounded"] {
            ok(&ch(m, "", &pimple(scheme)));
        }
    }
    for extra in [
        "variant = \"rng\"\n",
        "variant = \"realizable\"\n",
        "wall_function = \"scalable\"\n",
        "wall_function = \"enhanced\"\n",
        "wall_treatment = \"hysteresis\"\n",
    ] {
        ok(&ch(K, extra, &pimple("euler")));
    }
    for extra in [
        "[turbulence.sa]\nwall_mode = \"resolved\"\n",
        "[turbulence.sa]\nwall_mode = \"auto_sa_law\"\n",
        "[turbulence.sa]\nlimiter = \"min_0p3\"\n",
        "[turbulence.sa]\nnegative_nutilde = false\n",
        "[turbulence.sa]\ndes = \"ddes\"\n",
    ] {
        ok(&ch(A, extra, &pimple("euler")));
    }
    ok(&ch(
        A,
        "[turbulence.sa]\ndes = \"ddes\"\n",
        "algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n",
    ));
    ok(&sector(K, false));
    ok(&sector(A, false));
    // ADR 0212 R1': the one case that changes, accepted in place of located at the key
    for m in [K, A] {
        ok(&ch(
            m,
            "[turbulence.periodic_seam]\ndiffusivity = \"pair\"\n",
            &pimple("euler"),
        ));
    }
    println!("S1t: accepted: CH-K and CH-A on pimple with euler, bdf2, bdf2_bounded; k-epsilon rng, realizable, scalable, enhanced, hysteresis; SA resolved, auto_sa_law, min_0p3, negative_nutilde off, ddes on pimple and piso; the two S4 sectors; CH-K and CH-A on pimple with periodic_seam.diffusivity = pair (ADR 0212 R1')");
    for m in [K, A] {
        let piso = ch(m, "", "algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n");
        has(&piso, "turbulence.model");
        let msgs = messages(&piso);
        let (_, text) = msgs
            .iter()
            .find(|(p, _)| p == "turbulence.model")
            .expect("piso message");
        assert!(
            text.contains("pimple") && text.contains("steady"),
            "{m:?}: the piso refusal does not name pimple and steady: {text}"
        );
        has(
            &format!("{}[energy]\nenabled = true\n", ch(m, "", &pimple("euler"))),
            "turbulence.model",
        );
        has(
            &format!(
                "{}[dynamic_mesh]\nenabled = true\nmotion = \"prescribed\"\nmoving_patches = [\"ymin\"]\n",
                ch(m, "", &pimple("euler"))
            ),
            "dynamic_mesh.enabled",
        );
        has(
            &ch(m, "", &format!("{}error_tolerance = 1e-3\n", pimple("euler"))),
            "solver.error_tolerance",
        );
        let fractional = ch(
            m,
            "",
            &format!("{}time_advance = \"fractional_step\"\n", pimple("euler")),
        );
        assert!(fractional.contains(DRIVE), "the CH text changed");
        has(&fractional.replace(DRIVE, ""), "solver.time_advance");
    }
    // ADR 0210 amendment 1: the moving wall of ADR 0199's sector stays refused for k-epsilon
    has(&sector(K, true), "boundaries.outer.velocity");
    println!("S1t: located at turbulence.model (piso, naming pimple and steady; energy), dynamic_mesh.enabled, solver.error_tolerance, solver.time_advance (pimple with the fractional step, no drive), boundaries.outer.velocity (the moving wall)");
    // the dispatch of ADR 0210 D3
    assert!(registry_model("k_epsilon").updates_in_outer_iterations());
    let mut sa = registry_model("spalart_allmaras");
    assert!(sa.updates_in_outer_iterations());
    for (des, want) in [("des", false), ("ddes", false), ("off", true)] {
        sa.set_option("des", des).unwrap();
        assert_eq!(sa.updates_in_outer_iterations(), want, "SA des = {des}");
    }
    println!("S1t: updates_in_outer_iterations: k_epsilon true; spalart_allmaras off true, des false, ddes false");
}

// ---------------------------------------------------------------------------------------------------------------
// T4p, T4u, T4k, T5k: the solver's momentum face viscosity on the seam
// ---------------------------------------------------------------------------------------------------------------

/// `n` steps of a text in the case directory.
fn run_steps(text: &str, n: usize) -> Simulation {
    let mut sim = sim_in(text);
    for k in 1..=n {
        sim.step();
        assert!(sim.failure().is_none(), "step {k}: {:?}", sim.failure());
    }
    sim
}

/// The seam state of a face viscosity: (pairs with unequal bits, pairs off the bits of `d1`, largest `abs(nu_f - nu_g) / max nu`
/// over the pairs, pairs).
fn seam_state(geom: &FvGeometry, face: &[f64], d1: &dyn Fn(usize, usize) -> f64) -> (usize, usize, f64, usize) {
    let pairs = seam_faces(geom);
    let (mut unequal, mut off) = (0, 0);
    let (mut diff, mut big) = (0.0_f64, 0.0_f64);
    for &(f, g) in &pairs {
        if face[f].to_bits() != face[g].to_bits() {
            unequal += 1;
        }
        if face[f].to_bits() != d1(f, g).to_bits() {
            off += 1;
        }
        diff = diff.max((face[f] - face[g]).abs());
        big = big.max(face[f]).max(face[g]);
    }
    (unequal, off, diff / big, pairs.len())
}

/// T4p: the property viscosity of GCHXP after 5 iterations: one value per pair with `pair`, the D1 bits from the temperature the
/// iteration read; the owner values equal up to rounding (item 2).
#[test]
fn t4p_gchxp_property_viscosity_on_the_seam() {
    let mut sim = run_steps(&gchxp(&seam("pair")), 4);
    let t4 = sim.field("T").unwrap();
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let geom = sim.geom.clone();
    let own = geom.mesh.owner();
    let d1 = |f: usize, g: usize| {
        let w = weight(&geom, f, g);
        mu_p(w * t4[own[f]] + (1.0 - w) * t4[own[g]])
    };
    let (unequal, off, _, n) = seam_state(&geom, &face, &d1);
    let owner = run_steps(&gchxp(""), 5);
    let (o_unequal, _, o_rel, _) = seam_state(&owner.geom, &owner.momentum_face_viscosity().unwrap(), &|_, _| f64::NAN);
    println!(
        "T4p GCHXP: {n} seam pairs after 5; pair: {unequal} with unequal bits, {off} off the D1 bits; owner (the finding): {o_unequal} pairs with unequal bits, largest relative pair difference {o_rel:.4e} (limit 1e-14)"
    );
    assert!(n > 0, "T4p: no seam");
    assert_eq!(unequal, 0, "T4p: pairs with unequal bits");
    assert_eq!(off, 0, "T4p: pairs off the D1 bits");
    assert!(o_rel <= 1e-14, "T4p: owner {o_rel:.3e}");
}

/// T4u: the user-function viscosity of CHXU after 5 iterations: one value per pair with `pair`, the D1 bits from the velocity the
/// iteration read; the owner values differ.
#[test]
fn t4u_chxu_user_function_viscosity_on_the_seam() {
    let mut sim = run_steps(&chxu(&seam("pair")), 4);
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let u4: Vec<[f64; 3]> = (0..ux.len()).map(|c| [ux[c], uy[c], uz[c]]).collect();
    sim.step();
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let geom = sim.geom.clone();
    let m = std::cell::RefCell::new(udf_of(UT));
    let d1 = |f: usize, g: usize| udf_d1(&mut m.borrow_mut(), &geom, &u4, None, f, g, f);
    let (unequal, off, _, n) = seam_state(&geom, &face, &d1);
    let owner = run_steps(&chxu(""), 5);
    let (_, _, o_rel, _) = seam_state(&owner.geom, &owner.momentum_face_viscosity().unwrap(), &|_, _| f64::NAN);
    println!(
        "T4u CHXU: {n} seam pairs after 5; pair: {unequal} with unequal bits, {off} off the D1 bits; owner: largest relative pair difference {o_rel:.4e} (must be >= 1e-3)"
    );
    assert!(n > 0, "T4u: no seam");
    assert_eq!(unequal, 0, "T4u: pairs with unequal bits");
    assert_eq!(off, 0, "T4u: pairs off the D1 bits");
    assert!(o_rel >= 1e-3, "T4u: the owner control did not fire, {o_rel:.3e}");
}

/// T4k: k-epsilon and Spalart-Allmaras on pimple after 3 steps: one value per pair with `pair`, the D1 bits from `nut`; the owner
/// values differ.
#[test]
fn t4k_kepsilon_and_sa_on_pimple_seam_face_viscosity() {
    type Text = fn(&str, &str) -> String;
    for (name, text) in [("CHXK", chxk as Text), ("CHXA", chxa as Text)] {
        for scheme in ["euler", "bdf2"] {
            let sim = run_steps(&text(scheme, &seam("pair")), 3);
            let face = sim.momentum_face_viscosity().expect("a face viscosity");
            let nut = sim.field("nut").unwrap();
            let geom = sim.geom.clone();
            let own = geom.mesh.owner();
            let d1 = |f: usize, g: usize| {
                let w = weight(&geom, f, g);
                NU_CH + w * nut[own[f]] + (1.0 - w) * nut[own[g]]
            };
            let (unequal, off, _, n) = seam_state(&geom, &face, &d1);
            let owner = run_steps(&text(scheme, ""), 3);
            let (_, _, o_rel, _) = seam_state(&owner.geom, &owner.momentum_face_viscosity().unwrap(), &|_, _| f64::NAN);
            println!(
                "T4k {name} {scheme}: {n} seam pairs after 3 steps; pair: {unequal} with unequal bits, {off} off the D1 bits; owner: largest relative pair difference {o_rel:.4e} (must be >= 1e-3)"
            );
            assert!(n > 0, "T4k {name} {scheme}: no seam");
            assert_eq!(unequal, 0, "T4k {name} {scheme}: pairs with unequal bits");
            assert_eq!(off, 0, "T4k {name} {scheme}: pairs off the D1 bits");
            assert!(
                o_rel >= 1e-3,
                "T4k {name} {scheme}: the owner control did not fire, {o_rel:.3e}"
            );
        }
    }
}

/// The seam faces of LAMXP after 3 steps: (faces off the bits of nu, largest value, faces).
fn lamxp_seam(extra: &str) -> (usize, f64, usize) {
    let sim = run_steps(&lamxp(extra), 3);
    let face = sim.momentum_face_viscosity().expect("a face viscosity");
    let faces: Vec<usize> = seam_faces(&sim.geom).iter().flat_map(|&(f, g)| [f, g]).collect();
    let off = faces.iter().filter(|&&f| face[f].to_bits() != NU_CH.to_bits()).count();
    let big = faces.iter().map(|&f| face[f]).fold(0.0, f64::max);
    (off, big, faces.len())
}

/// T5k: with k-epsilon on pimple a pair that touches a laminar cell takes the molecular viscosity on both faces.
#[test]
fn t5k_laminar_region_at_the_seam_on_pimple() {
    let (off, _, n) = lamxp_seam(&seam("pair"));
    let (_, big_owner, _) = lamxp_seam("");
    println!(
        "T5k LAMXP: {n} seam faces; pair: {off} off the bits of nu; owner: largest seam face viscosity {big_owner:.4e} = {:.3} nu (must be >= 1.001)",
        big_owner / NU_CH
    );
    assert!(n > 0, "T5k: no seam");
    assert_eq!(off, 0, "T5k: pair faces off the molecular value");
    assert!(big_owner >= 1.001 * NU_CH, "T5k: the owner control did not fire");
}

// ---------------------------------------------------------------------------------------------------------------
// R0p, V0c''
// ---------------------------------------------------------------------------------------------------------------

/// R0p (report only): CHXP with `pair` against the CHXP record of V0b'' (prediction PR5: equal, item 2 on a uniform seam).
#[test]
fn r0p_chxp_pair_record_report() {
    let got = record_in(&chxp(&seam("pair")), 50);
    println!(
        "R0p CHXP with pair: ({}, {}, 0x{:016x}); the V0b'' record ({}, {}, 0x{:016x}); equal: {}",
        got.0,
        got.1,
        got.2,
        V0B[0].0,
        V0B[0].1,
        V0B[0].2,
        got == V0B[0]
    );
}

/// V0c'': every V0b'' run with `owner` written gives its record.
#[test]
fn v0c_explicit_owner() {
    let mut bad = Vec::new();
    for (name, text, n, want) in v0_cases(&seam("owner")) {
        let got = record_in(&text, n);
        println!(
            "V0c'' {name} with owner written: ({}, {}, 0x{:016x})",
            got.0, got.1, got.2
        );
        if got != want {
            bad.push(format!("{name}: {got:?} against {want:?}"));
        }
    }
    assert!(bad.is_empty(), "V0c'': {bad:#?}");
}
