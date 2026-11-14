//! F-547 (ADR 0465): the pre-stressed modes on the transformation with a cyclic constraint and the orthotropic or the
//! tabulated material of ADR 0452. Checks P1, P1t, P2 (the index-0 pre-stressed modes of a sector against the harmonic-0
//! modes of the full rotor), A1, A2 (axes variants inside the coincidence test), R1 (other axes refused by name), R2 (no
//! cyclic constraint: the check does not refuse) and the negative controls NC0 (the record of the gap on the base), NC1,
//! NC2 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::cyclic::{full_rotor, rotation_matrix, FullRotor};
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::mat_ortho::FOLLOW_UP;
use rcfd_fem::material::{Axes, MaterialAxes};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.3e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} <= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const E_O: [f64; 3] = [1.4e11, 1.0e10, 1.0e10];
const NU_O: [f64; 3] = [0.3, 0.3, 0.45];
const G_O: [f64; 3] = [5.0e9, 5.0e9, 3.5e9];
const RHO_O: f64 = 1600.0;
const TABLE_T: [f64; 2] = [200.0, 600.0];
const T_REF: f64 = 293.0;
/// The number of sectors of the disc D.
const N: usize = 12;
/// The index of the cyclic constraint in the sector model (after the three clamps).
const J: usize = 3;
/// The speed of the centrifugal pre-stress `CEN` (arithmetic 4), rad/s.
const OMEGA: f64 = 680.0;
/// The uniform pre-stress `UNI`, Pa.
const UNI: [f64; 6] = [1.0e8, 1.0e8, 0.0, 0.0, 0.0, 0.0];
/// The number of full modes of D6 step 2.
const FULL_MODES: usize = 60;

// ------------------------------------------------------------------------------------------------------------ materials

fn cyl(origin: Vec3, axis: Vec3) -> MaterialAxes {
    MaterialAxes::cylindrical(origin, axis).unwrap()
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO_O, [0.0; 3], axes).unwrap()
}

/// `ORTHO_C` of D8.
fn ortho_c() -> Material {
    ortho(cyl(ORIGIN, AXIS))
}

fn o_columns() -> Vec<(String, Vec<f64>)> {
    vec![
        ("e_1".to_string(), vec![1.4e11, 1.0e11]),
        ("g_12".to_string(), vec![5.0e9, 4.0e9]),
    ]
}

/// `TAB_O` of D8.
fn tab_o() -> Material {
    Material::tabulated(ortho_c(), TABLE_T.to_vec(), o_columns()).unwrap()
}

/// `TAB_F` of D8.
fn tab_f() -> Material {
    Material::tabulated(ortho(MaterialAxes::fixed(I3).unwrap()), TABLE_T.to_vec(), o_columns()).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ models

fn mesh_d() -> SolidMesh {
    let a = 2.0 * PI / N as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [3, 2, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

/// The field of `TAB_O`: `T = 293 + 250 (r - 0.25)/0.75 + 200 z`.
fn field_value(x: Vec3) -> f64 {
    T_REF + 250.0 * ((x[0] * x[0] + x[1] * x[1]).sqrt() - 0.25) / 0.75 + 200.0 * x[2]
}

fn clamp(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
}

/// The sector D with `mat` on `"all"`, `xmin` clamped, the cyclic constraint (index `J`) and, with `field`, the field.
fn sector(mat: Material, field: bool) -> SolidModel {
    let mesh = mesh_d();
    let t: Vec<f64> = mesh.nodes().iter().map(|&x| field_value(x)).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    if field {
        m.add_load(Load::temperature(t, T_REF)).unwrap();
    }
    m
}

/// The full rotor of `sector_model` with `mat` and the sector field copied node by node (ADR 0462 D4).
fn full_of(sector_model: &SolidModel, mat: Material, field: bool) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector_model).unwrap();
    let mut full = SolidModel::new(rotor.mesh().clone());
    full.add_material("all", mat).unwrap();
    clamp(&mut full);
    if field {
        let xs = sector_model.mesh().nodes();
        let mut t = vec![f64::NAN; rotor.mesh().nodes().len()];
        for j in 0..rotor.sectors() {
            for (p, &x) in xs.iter().enumerate() {
                t[rotor.node(j, p)] = field_value(x);
            }
        }
        assert!(t.iter().all(|v| v.is_finite()), "every full node has a temperature");
        full.add_load(Load::temperature(t, T_REF)).unwrap();
    }
    (rotor, full)
}

/// `CEN` of D8: the pre-stress of the static solve of `m` with the centrifugal load at `OMEGA` about the cyclic axis.
fn cen(m: &SolidModel) -> Prestress {
    let mut c = m.clone();
    c.add_load(Load::centrifugal(AXIS, ORIGIN, OMEGA)).unwrap();
    let r = solve_static(&c, &StaticOptions::default()).unwrap();
    Prestress::from_static(&c, &r).unwrap()
}

fn eq_opts(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn quad(a: &CsrMatrix, x: &[f64]) -> f64 {
    mv(a, x).iter().zip(x).map(|(p, q)| p * q).sum()
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

// ------------------------------------------------------------------------------------------------- the procedure of D6

/// The field repeating in every sector of a full field `phi` (D6 step 3): the average over the rotations, as a full field.
fn repeating_part(rotor: &FullRotor, sector_model: &SolidModel, full: &SolidModel, phi: &[f64]) -> Vec<f64> {
    let sdofs = sector_model.dof_map();
    let fdofs = full.dof_map();
    let nsec = rotor.sectors();
    let mut u0 = vec![0.0; sdofs.n_equations()];
    for p in 0..sector_model.mesh().nodes().len() {
        let mut acc = [0.0; 3];
        for j in 0..nsec {
            let r = rotation_matrix(AXIS, 2.0 * PI * j as f64 / nsec as f64);
            let q = rotor.node(j, p);
            let v: Vec<f64> = (0..3).map(|i| phi[fdofs.equation(q, i).unwrap()]).collect();
            for (i, a) in acc.iter_mut().enumerate() {
                *a += (0..3).map(|m| r[m][i] * v[m]).sum::<f64>();
            }
        }
        for (i, a) in acc.iter().enumerate() {
            u0[sdofs.equation(p, i).unwrap()] = a / nsec as f64;
        }
    }
    let zero = vec![0.0; u0.len()];
    rotor.expand_field(&sdofs, &fdofs, 0, &u0, &zero)
}

/// The free rows of the full model.
fn free_rows(full: &SolidModel) -> Vec<bool> {
    let mut free = vec![true; full.dof_map().n_equations()];
    for (e, _) in prescribed_equations(full, &[]).unwrap() {
        free[e] = false;
    }
    free
}

/// `|A x - lambda M x| / |A x|` on the free rows.
fn residual(a: &CsrMatrix, m: &CsrMatrix, free: &[bool], lambda: f64, x: &[f64]) -> f64 {
    let (ax, mx) = (mv(a, x), mv(m, x));
    let (mut rn, mut an) = (0.0, 0.0);
    for e in 0..x.len() {
        if free[e] {
            rn += (ax[e] - lambda * mx[e]).powi(2);
            an += ax[e].powi(2);
        }
    }
    (rn / an).sqrt()
}

/// The part of the full rotor for the residual operators: `operator(1) = K + K_sigma(prestress)`, `operator(0) = K`.
fn full_part<'a>(full: &'a SolidModel, prestress: &Prestress) -> RotatingPart<'a> {
    RotatingPart::with_unit_prestress(
        full,
        AXIS,
        ORIGIN,
        prestress.clone(),
        &RotatingOptions::new(2).modal(eq_opts(2)).spin_softening(false),
    )
    .unwrap()
}

/// What the procedure returns for the negative controls: the compared sector eigenvalues and their expanded modes.
struct Compared {
    values: Vec<f64>,
    expanded: Vec<Vec<f64>>,
}

/// D6 steps 1 to 4 for a sector model with its pre-stress and its full rotor with its pre-stress.
fn equivalence(
    tag: &str,
    s: &SolidModel,
    ps: &Prestress,
    rotor: &FullRotor,
    full: &SolidModel,
    pf: &Prestress,
    ch: &mut Checks,
) -> Compared {
    // Step 1.
    let sr = prestressed_modes(s, ps, &eq_opts(4)).unwrap();
    println!("{tag} sector: f = {:?} Hz", sr.frequencies);
    let lambda_c = 0.999 * sr.eigenvalues.iter().fold(0.0f64, |m, &v| m.max(v));
    let below: Vec<usize> = (0..sr.eigenvalues.len())
        .filter(|&i| sr.eigenvalues[i] < lambda_c)
        .collect();
    // Step 2.
    let fr = prestressed_modes(full, pf, &eq_opts(FULL_MODES)).unwrap();
    println!(
        "{tag} full: {} unknowns, {} modes, largest {:e}, cut-off {lambda_c:e}, compared {}",
        fr.unknowns,
        fr.eigenvalues.len(),
        fr.eigenvalues.last().unwrap(),
        below.len()
    );
    assert!(
        *fr.eigenvalues.last().unwrap() > lambda_c,
        "defective set-up: the full model's computed modes do not pass the cut-off"
    );
    let near = sr
        .eigenvalues
        .iter()
        .chain(fr.eigenvalues.iter())
        .map(|l| (l / lambda_c - 1.0).abs())
        .fold(f64::INFINITY, f64::min);
    assert!(
        near >= 1e-7,
        "defective set-up: an eigenvalue within 1e-7 of the cut-off"
    );
    // Step 3.
    let part = full_part(full, pf);
    let m = part.mass();
    let mut full_0: Vec<f64> = Vec::new();
    let mut worst_sep: f64 = 0.0;
    for (l, phi) in fr.eigenvalues.iter().zip(&fr.modes) {
        if *l >= lambda_c {
            continue;
        }
        let psi = repeating_part(rotor, s, full, phi);
        let f = quad(m, &psi) / quad(m, phi);
        worst_sep = worst_sep.max(f.min(1.0 - f).abs());
        if f >= 0.5 {
            full_0.push(*l);
        }
    }
    println!(
        "{tag}: full harmonic-0 f = {:?} Hz",
        full_0.iter().map(|l| l.sqrt() / (2.0 * PI)).collect::<Vec<_>>()
    );
    // Step 4.
    let values: Vec<f64> = below.iter().map(|&i| sr.eigenvalues[i]).collect();
    ch.holds(
        &format!("{tag} count"),
        full_0.len() == values.len(),
        &format!("full harmonic 0 {} = sector {}", full_0.len(), values.len()),
    );
    let worst_f = values
        .iter()
        .zip(&full_0)
        .map(|(c, f)| ((c / f).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.at_most(&format!("{tag} frequencies"), worst_f, 1e-8);
    ch.at_most(&format!("{tag} separation"), worst_sep, 1e-6);
    let sdofs = s.dof_map();
    let fdofs = full.dof_map();
    let free = free_rows(full);
    let a = part.operator(1.0);
    let mut expanded = Vec::new();
    let mut worst_r: f64 = 0.0;
    for &i in &below {
        let zero = vec![0.0; sr.modes[i].len()];
        let x = rotor.expand_field(&sdofs, &fdofs, 0, &sr.modes[i], &zero);
        worst_r = worst_r.max(residual(&a, m, &free, sr.eigenvalues[i], &x));
        expanded.push(x);
    }
    ch.at_most(&format!("{tag} expanded residual"), worst_r, 1e-7);
    Compared { values, expanded }
}

/// D6 step 5: the stress of the full rotor's pre-stress in the elements of copy 0 against the sector's.
fn prestress_of_copy_0(tag: &str, ps: &Prestress, pf: &Prestress, ch: &mut Checks) {
    let n_e = ps.offsets().len() - 1;
    let n_p = ps.stress().len();
    assert_eq!(pf.offsets()[n_e], n_p, "copy 0 is the first elements of the full rotor");
    let a: Vec<f64> = pf.stress()[..n_p].iter().flatten().copied().collect();
    let b: Vec<f64> = ps.stress().iter().flatten().copied().collect();
    let d = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / inf(&b);
    ch.at_most(&format!("{tag} pre-stress of copy 0"), d, 1e-8);
}

// --------------------------------------------------------------------------------------------------- P1, P1t, NC1, NC2

#[test]
fn p1_p1t_nc1_nc2_centrifugal_sector_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(ortho_c(), false);
    let (rotor, full) = full_of(&s, ortho_c(), false);
    let (ps, pf) = (cen(&s), cen(&full));
    prestress_of_copy_0("P1 ORTHO_C CEN", &ps, &pf, &mut ch);
    let c = equivalence("P1 ORTHO_C CEN", &s, &ps, &rotor, &full, &pf, &mut ch);
    // NC1: the same sector without the pre-stress.
    let z = prestressed_modes(&s, &Prestress::uniform(&s, [0.0; 6]).unwrap(), &eq_opts(4)).unwrap();
    let nc1 = c
        .values
        .iter()
        .zip(&z.eigenvalues)
        .map(|(a, b)| ((a / b).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.above("NC1 pre-stressed against unstressed", nc1, 1e-2);
    // NC2: the residual of the expanded modes on the full rotor's K alone.
    let part = full_part(&full, &pf);
    let k = part.operator(0.0);
    let free = free_rows(&full);
    let nc2 = c
        .values
        .iter()
        .zip(&c.expanded)
        .map(|(&l, x)| residual(&k, part.mass(), &free, l, x))
        .fold(0.0f64, f64::max);
    ch.above("NC2 expanded residual on K alone", nc2, 1e-2);
    // P1t: the table with its field.
    let st = sector(tab_o(), true);
    let (rotor_t, full_t) = full_of(&st, tab_o(), true);
    let (pst, pft) = (cen(&st), cen(&full_t));
    prestress_of_copy_0("P1t TAB_O CEN", &pst, &pft, &mut ch);
    equivalence("P1t TAB_O CEN", &st, &pst, &rotor_t, &full_t, &pft, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- P2

#[test]
fn p2_uniform_prestress_sector_against_full_rotor() {
    let mut ch = Checks::new();
    let s = sector(ortho_c(), false);
    let (rotor, full) = full_of(&s, ortho_c(), false);
    let ps = Prestress::uniform(&s, UNI).unwrap();
    let pf = Prestress::uniform(&full, UNI).unwrap();
    equivalence("P2 ORTHO_C UNI", &s, &ps, &rotor, &full, &pf, &mut ch);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- A1, A2

#[test]
fn a1_a2_axes_variants_inside_the_test() {
    let mut ch = Checks::new();
    let base = sector(ortho_c(), false);
    let p = cen(&base);
    let r0 = prestressed_modes(&base, &p, &opts(3)).unwrap();
    for (name, axes) in [
        ("axis (0, 0, -1)", cyl(ORIGIN, [0.0, 0.0, -1.0])),
        ("origin (0, 0, 0.3)", cyl([0.0, 0.0, 0.3], AXIS)),
    ] {
        let v = sector(ortho(axes), false);
        match prestressed_modes(&v, &p, &opts(3)) {
            Ok(r) => {
                let modes_same =
                    r.modes.len() == r0.modes.len() && r.modes.iter().zip(&r0.modes).all(|(x, y)| same(x, y));
                ch.holds(
                    &format!("A1 {name}"),
                    same(&r.eigenvalues, &r0.eigenvalues) && modes_same,
                    "eigenvalues and modes equal as numbers",
                );
            }
            Err(e) => ch.holds(&format!("A1 {name}"), false, &format!("{e:?}")),
        }
    }
    let t = sector(ortho(cyl(ORIGIN, [0.0, 1e-11, 1.0])), false);
    let r = prestressed_modes(&t, &p, &opts(3));
    ch.holds("A2 axis (0, 1e-11, 1) solves", r.is_ok(), &format!("{:?}", r.err()));
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- R1, R2

fn cyclic_refusal(r: &rcfd_fem::Result<()>, variant: &str) -> bool {
    let want = format!("material 0, element set 'all', with the cyclic constraint {J}");
    matches!(r, Err(FemError::NotYetSupported { item: "material", variant: v, location: l, package: p })
        if *v == variant && *l == want && *p == FOLLOW_UP)
}

/// The transformation block TRB of D8 (no cyclic constraint), with the field `T = 293 + 250 x` if `field`.
fn trb(mat: Material, field: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.4, 0.25], [5, 2, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let t: Vec<f64> = mesh.nodes().iter().map(|x| T_REF + 250.0 * x[0]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    for &n in &root {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
            .unwrap();
    }
    m.add_support(ElasticSupport::nodal_spring("tip", 2, 5.0e7)).unwrap();
    if field {
        m.add_load(Load::temperature(t, T_REF)).unwrap();
    }
    m
}

#[test]
fn r1_r2_refusals_and_models_without_a_cyclic_constraint() {
    let mut ch = Checks::new();
    let cases: Vec<(&str, Material, bool, &str)> = vec![
        (
            "Fixed(I)",
            ortho(MaterialAxes::fixed(I3).unwrap()),
            false,
            "Orthotropic",
        ),
        (
            "tilted (0, 0.1, 1)",
            ortho(cyl(ORIGIN, [0.0, 0.1, 1.0])),
            false,
            "Orthotropic",
        ),
        (
            "tilted (0, 1e-9, 1)",
            ortho(cyl(ORIGIN, [0.0, 1e-9, 1.0])),
            false,
            "Orthotropic",
        ),
        (
            "origin (0.05, 0, 0)",
            ortho(cyl([0.05, 0.0, 0.0], AXIS)),
            false,
            "Orthotropic",
        ),
        (
            "PerElement",
            ortho(MaterialAxes::per_element(vec![I3; 6]).unwrap()),
            false,
            "Orthotropic",
        ),
        ("Tabulated on Fixed(I)", tab_f(), true, "Tabulated"),
    ];
    for (name, mat, field, variant) in cases {
        let m = sector(mat, field);
        let r = prestressed_modes(&m, &Prestress::uniform(&m, [0.0; 6]).unwrap(), &ModalOptions::new(2)).map(|_| ());
        ch.holds(
            &format!("R1 {name}: prestressed_modes refused"),
            cyclic_refusal(&r, variant),
            &format!("{:?}", r.err()),
        );
    }
    for (name, mat, field) in [
        ("ORTHO Fixed(I)", ortho(MaterialAxes::fixed(I3).unwrap()), false),
        ("TAB_F with its field", tab_f(), true),
    ] {
        let b = trb(mat, field);
        let p = Prestress::uniform(&b, [2.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
        let r = prestressed_modes(&b, &p, &ModalOptions::new(3));
        ch.holds(
            &format!("R2 TRB {name}: prestressed_modes solves"),
            r.is_ok(),
            &format!("{:?}", r.err()),
        );
    }
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- NC0

#[test]
#[ignore = "record of ADR 0465 NC0: run once at the registration commit on the base library (main a52a5291), before any library line of F-547; after F-547 the sector is refused (R1)"]
fn nc0_base_gap_record() {
    let mut ch = Checks::new();
    let fixed = || ortho(MaterialAxes::fixed(I3).unwrap());
    let s = sector(fixed(), false);
    // Amendment 1 of ADR 0465 (before any run): the uniform pre-stress UNI on both (the static solve of this sector under a
    // centrifugal load is refused since ADR 0462, so CEN cannot be formed for it).
    let ps = Prestress::uniform(&s, UNI).unwrap();
    let sr = match prestressed_modes(&s, &ps, &eq_opts(4)) {
        Ok(r) => r,
        Err(e) => {
            println!("NC0: prestressed_modes refused ({e:?}): the state after F-547");
            return;
        }
    };
    ch.holds("NC0 sector Fixed(I) accepted on the base", true, "solved");
    println!("NC0 sector Fixed(I): f = {:?} Hz", sr.frequencies);
    let (rotor, full) = full_of(&s, fixed(), false);
    let pf = Prestress::uniform(&full, UNI).unwrap();
    let part = full_part(&full, &pf);
    let a = part.operator(1.0);
    let free = free_rows(&full);
    let sdofs = s.dof_map();
    let fdofs = full.dof_map();
    let lambda_c = 0.999 * sr.eigenvalues.iter().fold(0.0f64, |m, &v| m.max(v));
    let smallest = (0..sr.eigenvalues.len())
        .filter(|&i| sr.eigenvalues[i] < lambda_c)
        .map(|i| {
            let zero = vec![0.0; sr.modes[i].len()];
            let x = rotor.expand_field(&sdofs, &fdofs, 0, &sr.modes[i], &zero);
            let r = residual(&a, part.mass(), &free, sr.eigenvalues[i], &x);
            println!("NC0 mode {i}: residual on the full rotor as written {r:.3e}");
            r
        })
        .fold(f64::INFINITY, f64::min);
    ch.above(
        "NC0 smallest expanded residual on the full rotor as written",
        smallest,
        1e-2,
    );
    ch.finish();
}
