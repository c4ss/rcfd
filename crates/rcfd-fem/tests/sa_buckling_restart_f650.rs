//! F-650 (ADR 0564): the buckling eigensolver when the default subspace holds fewer positive Ritz values than asked (the
//! ADR 0553 monosymmetric I at `L = 1.5` under `-M_ref`). Checks A1 to A3, the records, and B2 (the hash of results that
//! returned before) of the registration. Each registered run is made once; every value is printed. The model helpers and
//! the hash repeat those of `sa_buckling_sign_asymmetric_f642.rs` (ADR 0559).
use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{buckling_factors, Warping, WarpingBucklingResult, WarpingSection};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_buckling_factors, BucklingEigen, BucklingEigenOptions};
use rcfd_linalg::CsrMatrix;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;
const M_REF: f64 = 1000.0;

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// The monosymmetric I of ADR 0553: flange 1 (`0.15 x 0.012`, the larger) on `+e_3`.
fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, KAPPA).unwrap()
}

/// A fork-supported beam of `n` elements along `x`, orientation `y`, end moments `s M_ref` about `e_2` (ADR 0553 helpers).
fn loaded_fork(n: usize, l: f64, section: &BeamSection, s: f64) -> (SolidModel, SolidModel) {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [l * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for k in 0..4 {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for k in 1..4 {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_moment("x0", [0.0, -s * M_REF, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_moment("xl", [0.0, s * M_REF, 0.0]))
        .unwrap();
    (m, loaded)
}

fn solve(
    section: &BeamSection,
    ws: WarpingSection,
    n: usize,
    l: f64,
    s: f64,
    opts: &BucklingOptions,
) -> rcfd_fem::Result<WarpingBucklingResult> {
    let (m, loaded) = loaded_fork(n, l, section, s);
    let r = rcfd_fem::solve::solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &r)
        .unwrap()
        .with_beam_resultants(&loaded, r.displacement_values(), [0.0; 2])
        .unwrap();
    buckling_factors(&m, &pre, &Warping::new().block(0, ws), opts)
}

fn with_subspace(q: usize) -> BucklingOptions {
    let mut o = BucklingOptions::new(3);
    o.subspace_size = Some(q);
    o
}

#[test]
fn a1_to_a3_default_options_at_l_1_5() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let reference = solve(&s, ws, 16, 1.5, -1.0, &with_subspace(40)).expect("the subspace-40 solve");
    println!(
        "q 40: factors {:?}, iterations {}, Sturm {:?}",
        reference.factors, reference.iterations, reference.sturm
    );
    match solve(&s, ws, 16, 1.5, -1.0, &BucklingOptions::new(3)) {
        Ok(r) => {
            println!(
                "A1: factors {:?}, iterations {}, Sturm {:?}, exhausted {:?}, unknowns {}",
                r.factors, r.iterations, r.sturm, r.exhausted, r.unknowns
            );
            c.holds(
                "A1 returned with three factors, not exhausted",
                r.factors.len() == 3 && r.exhausted.is_none(),
                &format!("{} factors, exhausted {:?}", r.factors.len(), r.exhausted),
            );
            for j in 0..3 {
                let got = r.factors.get(j).copied().unwrap_or(f64::NAN);
                c.at_most(
                    &format!("A2 factor {} against q 40", j + 1),
                    rel(got, reference.factors[j]),
                    1e-9,
                );
            }
            let l3 = r.factors.get(2).copied().unwrap_or(f64::NAN);
            c.holds(
                "A3 Sturm record",
                r.sturm.counted == r.sturm.expected && r.sturm.expected >= 3 && r.sturm.shift > l3,
                &format!("{:?}, lambda_3 {l3:.12e}", r.sturm),
            );
        }
        Err(e) => c.holds("A1 returned", false, &format!("refused: {e}")),
    }
    // Record, no limit: 8 elements.
    let d8 = solve(&s, ws, 8, 1.5, -1.0, &BucklingOptions::new(3));
    let r8 = solve(&s, ws, 8, 1.5, -1.0, &with_subspace(24));
    match (&d8, &r8) {
        (Ok(d), Ok(a)) => println!(
            "record 8 elements L 1.5: default {:?} (iterations {}, Sturm {:?}), q 24 {:?}, differences {:?}",
            d.factors,
            d.iterations,
            d.sturm,
            a.factors,
            d.factors
                .iter()
                .zip(&a.factors)
                .map(|(x, y)| rel(*x, *y))
                .collect::<Vec<_>>()
        ),
        _ => println!(
            "record 8 elements L 1.5: default {:?}, q 24 {:?}",
            d8.as_ref().map(|r| r.factors.clone()),
            r8.as_ref().map(|r| r.factors.clone())
        ),
    }
    c.finish();
}
/// FNV-1a 64.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn f(&mut self, v: f64) {
        self.word(v.to_bits());
    }
    fn all(&mut self, v: &[f64]) {
        self.word(v.len() as u64);
        for &x in v {
            self.f(x);
        }
    }
    fn exhausted(&mut self, e: Option<f64>) {
        match e {
            None => self.word(0),
            Some(x) => {
                self.word(1);
                self.f(x);
            }
        }
    }
}

fn hash_linalg(h: &mut Fnv, r: &BucklingEigen) {
    h.all(&r.values);
    for v in &r.vectors {
        h.all(v);
    }
    h.all(&r.residuals);
    h.word(r.iterations as u64);
    h.word(r.subspace_size as u64);
    h.f(r.sturm.shift);
    h.word(r.sturm.counted as u64);
    h.word(r.sturm.expected as u64);
    h.exhausted(r.exhausted);
}

fn hash_fem(h: &mut Fnv, r: &WarpingBucklingResult) {
    h.all(&r.factors);
    for v in &r.modes {
        h.all(v);
    }
    h.all(&r.residuals);
    h.word(r.iterations as u64);
    h.f(r.sturm.shift);
    h.word(r.sturm.counted as u64);
    h.word(r.sturm.expected as u64);
    h.word(r.unknowns as u64);
    h.exhausted(r.exhausted);
}

fn diag(v: &[f64]) -> CsrMatrix {
    CsrMatrix::from_triplets(v.len(), v.iter().enumerate().map(|(i, &x)| (i, i, x)).collect())
}

fn second_difference(n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, 2.0));
        if i + 1 < n {
            t.push((i, i + 1, -1.0));
            t.push((i + 1, i, -1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

/// B2: one hash over the results of inputs that returned before F-650. The same file is run on a copy of cf32422b and on
/// the branch; the hashes must be equal (the comparison is in the ADR addendum).
#[test]
fn b2_hash_of_results_that_returned_before() {
    let mut total = Fnv::new();
    let mut each = |name: &str, f: &dyn Fn(&mut Fnv)| {
        let mut h = Fnv::new();
        f(&mut h);
        println!("B2 case {name}: {:016x}", h.0);
        total.word(h.0);
    };
    let dflt = BucklingEigenOptions::default();
    let k200 = second_difference(200);
    each("1 tridiagonal, -I, ten factors", &|h| {
        hash_linalg(
            h,
            &lowest_buckling_factors(&k200, &diag(&[-1.0; 200]), 10, &dflt).unwrap(),
        )
    });
    each("2 mixed signs, q 30", &|h| {
        let d: Vec<f64> = (0..200).map(|i| if i < 100 { -1.0 } else { 1.0 }).collect();
        let o = BucklingEigenOptions {
            subspace_size: Some(30),
            ..BucklingEigenOptions::default()
        };
        hash_linalg(h, &lowest_buckling_factors(&k200, &diag(&d), 5, &o).unwrap())
    });
    each("3 L4, q 10", &|h| {
        let d: Vec<f64> = (0..10).map(|i| if i == 0 { -1.0 } else { 100.0 * i as f64 }).collect();
        let o = BucklingEigenOptions {
            subspace_size: Some(10),
            ..BucklingEigenOptions::default()
        };
        hash_linalg(
            h,
            &lowest_buckling_factors(&diag(&[1.0; 10]), &diag(&d), 1, &o).unwrap(),
        )
    });
    each("4 +I, no factor", &|h| {
        hash_linalg(
            h,
            &lowest_buckling_factors(&k200, &diag(&[1.0; 200]), 3, &dflt).unwrap(),
        )
    });
    each("5 branch (ii) accepted at S", &|h| {
        let mut d = vec![-30.0, -29.0, -28.0];
        d.extend((1..=17).map(|i| i as f64));
        hash_linalg(
            h,
            &lowest_buckling_factors(&diag(&[1.0; 20]), &diag(&d), 3, &dflt).unwrap(),
        )
    });
    let (s, ws) = mono();
    each("6 mono 16, +M_ref, default", &|h| {
        hash_fem(h, &solve(&s, ws, 16, 3.0, 1.0, &BucklingOptions::new(3)).unwrap())
    });
    each("7 mono 16, -M_ref, q 40", &|h| {
        hash_fem(h, &solve(&s, ws, 16, 3.0, -1.0, &with_subspace(40)).unwrap())
    });
    each("8 mono 8, both signs, q 24", &|h| {
        hash_fem(h, &solve(&s, ws, 8, 3.0, 1.0, &with_subspace(24)).unwrap());
        hash_fem(h, &solve(&s, ws, 8, 3.0, -1.0, &with_subspace(24)).unwrap());
    });
    let (sd, wd) = WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap();
    each("9 doubly symmetric 16, +M_ref, default", &|h| {
        hash_fem(h, &solve(&sd, wd, 16, 3.0, 1.0, &BucklingOptions::new(3)).unwrap())
    });
    each("10 branch (i) accepted at S, two of three", &|h| {
        let mut d = vec![-1.0, -0.5];
        d.extend((10..=18).map(|i| i as f64));
        d.extend([0.01; 9]);
        hash_linalg(
            h,
            &lowest_buckling_factors(&diag(&[1.0; 20]), &diag(&d), 3, &dflt).unwrap(),
        )
    });
    each("11 mono 16, L 3, -M_ref, default", &|h| {
        hash_fem(h, &solve(&s, ws, 16, 3.0, -1.0, &BucklingOptions::new(3)).unwrap())
    });
    each("12 mono 16, L 6, -M_ref, default", &|h| {
        hash_fem(h, &solve(&s, ws, 16, 6.0, -1.0, &BucklingOptions::new(3)).unwrap())
    });
    each("13 ADR 0559 P1 pencil", &|h| {
        let mut d = vec![-1.0, -0.9, -0.8];
        d.extend((10..=17).map(|i| i as f64));
        d.extend((0..9).map(|i| -(0.5 - 0.05 * i as f64)));
        hash_linalg(
            h,
            &lowest_buckling_factors(&diag(&[1.0; 20]), &diag(&d), 3, &dflt).unwrap(),
        )
    });
    println!("B2 total: {:016x}", total.0);
}
