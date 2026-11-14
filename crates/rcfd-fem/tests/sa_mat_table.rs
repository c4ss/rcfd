//! F-409 (ADR 0452): elastic properties and expansion against temperature from tables. Checks O10 to O13 and R4 to R6 of
//! the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::mat_table::PENDING as TABLE_PENDING;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel, SolidResult};

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

const E_O: [f64; 3] = [1.4e11, 1.0e10, 1.0e10];
const NU_O: [f64; 3] = [0.3, 0.3, 0.45];
const G_O: [f64; 3] = [5.0e9, 5.0e9, 3.5e9];
const ALPHA: [f64; 3] = [-1e-6, 3e-5, 2e-5];
const RHO: f64 = 1600.0;

type Axes = [[f64; 3]; 3];

fn rot(axis: Vec3, angle: f64) -> Axes {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, co) = angle.sin_cos();
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]][i][j];
            r[i][j] = (if i == j { co } else { 0.0 }) + s * kk + (1.0 - co) * k[i] * k[j];
        }
    }
    r
}

fn ortho(r: Axes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO, ALPHA, MaterialAxes::fixed(r).unwrap()).unwrap()
}

fn steel(expansion: f64) -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, expansion).unwrap()
}

fn col(name: &str, v: Vec<f64>) -> (String, Vec<f64>) {
    (name.to_string(), v)
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |a: f64, x| a.max(x.abs()))
}

fn bits(v: impl IntoIterator<Item = f64>) -> Vec<u64> {
    v.into_iter().map(f64::to_bits).collect()
}

fn node_at(mesh: &SolidMesh, x: Vec3) -> usize {
    mesh.nodes()
        .iter()
        .position(|p| (0..3).all(|i| (p[i] - x[i]).abs() <= 1e-12))
        .expect("node")
}

fn cube_model(mat: Material, t: f64, reference: f64) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let p0 = node_at(&mesh, [0.0, 0.0, 0.0]);
    let p1 = node_at(&mesh, [1.0, 0.0, 0.0]);
    let p2 = node_at(&mesh, [0.0, 1.0, 0.0]);
    mesh.add_node_set("p0", vec![p0]).unwrap();
    mesh.add_node_set("p1", vec![p1]).unwrap();
    mesh.add_node_set("p2", vec![p2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("p0", c, 0.0)).unwrap();
    }
    for c in 1..3 {
        m.add_constraint(Constraint::displacement("p1", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("p2", 2, 0.0)).unwrap();
    m.add_load(Load::temperature(vec![t; 8], reference)).unwrap();
    m
}

fn field_of(m: &SolidModel, r: &SolidResult) -> IntegrationPointField {
    IntegrationPointField::compute(m, r.displacement_values()).unwrap()
}

/// `R eps R^T` of an engineering strain, back to engineering Voigt.
fn strain_to_material(r: &Axes, v: &[f64; 6]) -> [f64; 6] {
    const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];
    let mut t = [[0.0; 3]; 3];
    for (k, &(i, j)) in PAIRS.iter().enumerate() {
        let x = if i == j { v[k] } else { v[k] / 2.0 };
        t[i][j] = x;
        t[j][i] = x;
    }
    std::array::from_fn(|k| {
        let (a, b) = PAIRS[k];
        let s: f64 = (0..3)
            .flat_map(|i| (0..3).map(move |j| (i, j)))
            .map(|(i, j)| r[a][i] * t[i][j] * r[b][j])
            .sum();
        if a == b {
            s
        } else {
            2.0 * s
        }
    })
}

// ------------------------------------------------------------------------------------------------- O10, O13

const BAR_E0: f64 = 2.0e11;
const BAR_E1: f64 = 1.0e11;
const BAR_F: f64 = 1e5;
const BAR_A: f64 = 0.01;

fn bar(hot_end: Option<f64>) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let t: Vec<f64> = mesh
        .nodes()
        .iter()
        .map(|x| match hot_end {
            Some(h) if (x[0] - 1.0).abs() <= 1e-12 => h,
            _ => 293.0 + 400.0 * x[0],
        })
        .collect();
    let mut m = SolidModel::new(mesh);
    let base = Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap();
    let table = Material::tabulated(base, vec![293.0, 693.0], vec![col("e", vec![BAR_E0, BAR_E1])]).unwrap();
    m.add_material("all", table).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [BAR_F / BAR_A, 0.0, 0.0])).unwrap();
    m.add_load(Load::temperature(t, 293.0)).unwrap();
    m
}

#[test]
fn o10_o13_bar_with_a_varying_modulus() {
    let mut c = Checks::new();
    c.holds("X mat_table landed", TABLE_PENDING.is_none(), "");
    // O10.
    let m = bar(None);
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let end = m.mesh().node_set("xmax").unwrap();
    let u: f64 = end.iter().map(|&n| res.displacement(n)[0]).sum::<f64>() / end.len() as f64;
    let exact = BAR_F * 1.0 * (BAR_E1 / BAR_E0).ln() / (BAR_A * (BAR_E1 - BAR_E0));
    println!("O10: stretch {u:.9e} m, closed form {exact:.9e} m");
    c.at_most(
        "O10 |stretch/closed form - 1| (20 elements)",
        (u / exact - 1.0).abs(),
        1e-3,
    );
    // O13.
    let r = solve_static(&bar(Some(800.0)), &StaticOptions::default());
    let (element, text) = match &r {
        Err(e @ FemError::Element { element, .. }) => (Some(*element), e.to_string()),
        Err(e) => (None, e.to_string()),
        Ok(_) => (None, String::new()),
    };
    let t: Option<f64> = text
        .match_indices("temperature ")
        .filter_map(|(i, s)| text[i + s.len()..].split_whitespace().next()?.parse::<f64>().ok())
        .next();
    println!("O13: {text}");
    let ok = element == Some(19) && text.contains("outside the table") && t.is_some_and(|t| t > 693.0 && t <= 800.0);
    c.holds(
        "O13 temperature outside the table refused with element 19 and the temperature",
        ok,
        &format!("{element:?} {t:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O11, O11b

#[test]
fn o11_integrated_expansion() {
    let mut c = Checks::new();
    let (a0, a1) = (1e-5, 2e-8);
    let temps = vec![250.0, 350.0, 450.0, 550.0];
    let alpha: Vec<f64> = temps.iter().map(|t| a0 + a1 * t).collect();
    let (t, tref) = (513.0, 293.0);
    let s = a0 * (t - tref) + a1 * (t * t - tref * tref) / 2.0;
    // O11.
    let table = Material::tabulated(steel(0.0), temps.clone(), vec![col("expansion", alpha.clone())]).unwrap();
    let m = cube_model(table, t, tref);
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = field_of(&m, &res);
    let (mut en, mut es): (f64, f64) = (0.0, 0.0);
    for eps in field.strain() {
        en = en.max(max_abs((0..3).map(|k| eps[k] / s - 1.0)));
        es = es.max(max_abs((3..6).map(|k| eps[k] / s)));
    }
    c.at_most("O11 normal strains against a0 dT + a1 (T^2 - Tref^2)/2", en, 1e-10);
    c.at_most("O11 shear strains", es, 1e-10);
    // O11b.
    let r = rot([1.0, 2.0, 3.0], 0.5);
    let table = Material::tabulated(
        ortho(r),
        temps.clone(),
        vec![col("expansion_1", alpha.clone()), col("expansion_2", vec![3e-5; 4])],
    )
    .unwrap();
    let m = cube_model(table, t, tref);
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = field_of(&m, &res);
    let want = [s, 3e-5 * (t - tref), ALPHA[2] * (t - tref), 0.0, 0.0, 0.0];
    let scale = max_abs(want);
    let mut eb: f64 = 0.0;
    for eps in field.strain() {
        let em = strain_to_material(&r, eps);
        eb = eb.max(max_abs((0..6).map(|k| em[k] - want[k])) / scale);
    }
    c.at_most("O11b orthotropic base, material-axes strains", eb, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O12

fn b1_model(mat: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let t: Vec<f64> = mesh
        .nodes()
        .iter()
        .map(|x| 293.0 + 120.0 * x[0] + 35.0 * x[1] - 20.0 * x[2])
        .collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 300.0, -100.0])).unwrap();
    m.add_load(Load::temperature(t, 293.0)).unwrap();
    m
}

fn same_bits(a: &SolidModel, b: &SolidModel) -> (bool, String) {
    let ra = solve_static(a, &StaticOptions::default()).unwrap();
    let rb = solve_static(b, &StaticOptions::default()).unwrap();
    let fa = field_of(a, &ra);
    let fb = field_of(b, &rb);
    let flat = |v: &[[f64; 6]]| bits(v.iter().flatten().copied());
    let du = bits(ra.displacement_values().iter().copied()) == bits(rb.displacement_values().iter().copied());
    let ds = flat(fa.stress()) == flat(fb.stress());
    let dt = flat(fa.thermal_strain().unwrap()) == flat(fb.thermal_strain().unwrap());
    let de = fa.strain_energy().to_bits() == fb.strain_energy().to_bits();
    (
        du && ds && dt && de,
        format!("displacement {du}, stress {ds}, thermal strain {dt}, energy {de}"),
    )
}

#[test]
fn o12_constant_table_bits() {
    let mut c = Checks::new();
    let temps = vec![200.0, 400.0, 600.0];
    let s = steel(1.2e-5);
    let table = Material::tabulated(
        s.clone(),
        temps.clone(),
        vec![
            col("e", vec![2.0e11; 3]),
            col("nu", vec![0.3; 3]),
            col("expansion", vec![1.2e-5; 3]),
        ],
    )
    .unwrap();
    let (ok, detail) = same_bits(&b1_model(table), &b1_model(s));
    c.holds("O12 constant table on steel: bits of steel", ok, &detail);
    let r = rot([1.0, 2.0, 3.0], 0.5);
    let o = ortho(r);
    let names = [
        "e_1",
        "e_2",
        "e_3",
        "nu_12",
        "nu_13",
        "nu_23",
        "g_12",
        "g_13",
        "g_23",
        "expansion_1",
        "expansion_2",
        "expansion_3",
    ];
    let values = [E_O, NU_O, G_O, ALPHA].concat();
    let columns: Vec<(String, Vec<f64>)> = names.iter().zip(&values).map(|(n, &v)| col(n, vec![v; 3])).collect();
    let table = Material::tabulated(o.clone(), temps, columns).unwrap();
    let (ok, detail) = same_bits(&b1_model(table), &b1_model(o));
    c.holds("O12b constant table on ORTHO: bits of ORTHO", ok, &detail);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R4 to R6

#[test]
fn r4_to_r6_table_refusals() {
    let mut c = Checks::new();
    let table =
        |columns: Vec<(String, Vec<f64>)>, temps: Vec<f64>| Material::tabulated(steel(1e-5), temps, columns).unwrap();
    // R4: no temperature field.
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap());
    m.add_material("all", table(vec![col("e", vec![2e11, 1.9e11])], vec![200.0, 600.0]))
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    let r4 = solve_static(&m, &StaticOptions::default());
    c.holds(
        "R4 Tabulated without a temperature field refused (temperature)",
        matches!(&r4, Err(FemError::InvalidValue { key: "temperature", .. })),
        &format!("{:?}", r4.as_ref().err()),
    );
    // R5: unknown and repeated columns.
    for (tag, columns) in [
        ("density", vec![col("density", vec![7850.0, 7800.0])]),
        (
            "repeated e",
            vec![col("e", vec![2e11, 1.9e11]), col("e", vec![2e11, 1.8e11])],
        ),
    ] {
        let m = b1_model(table(columns, vec![200.0, 600.0]));
        let r = solve_static(&m, &StaticOptions::default());
        c.holds(
            &format!("R5 column {tag} refused (columns)"),
            matches!(&r, Err(FemError::InvalidValue { key: "columns", .. })),
            &format!("{:?}", r.as_ref().err()),
        );
    }
    // R6: the reference temperature outside a table with an expansion column.
    let m = b1_model(table(vec![col("expansion", vec![1e-5, 1.2e-5])], vec![300.0, 600.0]));
    let r6 = solve_static(&m, &StaticOptions::default());
    c.holds(
        "R6 reference temperature outside an expansion table refused (temperature)",
        matches!(&r6, Err(FemError::InvalidValue { key: "temperature", .. })),
        &format!("{:?}", r6.as_ref().err()),
    );
    c.finish();
}
