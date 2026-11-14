//! F-583 (ADR 0494): the harmonic stress of a viscoelastic material in the direct harmonic solve, `(G*/G_0) dev sigma_0 +
//! (K*/K_0) vol sigma_0` at each integration point. Checks S1A to S1C, S2, S3 (rods against the modulus times the strain
//! of the discrete chain), SE1, SE2 (the table route), SF1, SF2 (two materials), Z1 (zero frequency), T1 (shift), C0 (no
//! terms), P1 (threads), NC1, NC2 (negative controls), RF1, RF2 (refusals) of the registration.

mod common;

use common::*;
use rcfd_fem::harmonic::{HarmonicResponse, HarmonicStress};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mat_visco::{modulus_table, shift_factor};
use rcfd_fem::{FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
use std::f64::consts::PI;

const N: usize = 40;
const PER_ELEMENT: usize = 8;

fn comp_of(st: &HarmonicStress, q: usize, c: usize) -> C64 {
    C64::new(st.re[q][c], st.im[q][c])
}

fn solve(
    model: &SolidModel,
    options: &DirectHarmonicOptions,
    comp: usize,
    w: f64,
) -> (HarmonicResponse, HarmonicStress) {
    let solver = DirectHarmonic::new(model, options).unwrap();
    let r = solver.response(&tip_load(model, comp), w).unwrap();
    let st = solver.stress(model, &r).unwrap();
    (r, st)
}

/// The reference stress of element `e` of a rod: `[modulus eps, lateral eps, lateral eps, 0, 0, 0]` axial,
/// `[0, 0, 0, modulus gamma, 0, 0]` shear.
fn reference(kind: Kind, modulus: C64, lateral: C64, eps: C64) -> [C64; 6] {
    let mut s = [C64::ZERO; 6];
    match kind {
        Kind::Axial => {
            s[0] = modulus * eps;
            s[1] = lateral * eps;
            s[2] = lateral * eps;
        }
        Kind::Shear => s[3] = modulus * eps,
    }
    s
}

fn primary(kind: Kind) -> usize {
    match kind {
        Kind::Axial => 0,
        Kind::Shear => 3,
    }
}

/// Against the reference per element (the same for its eight points): `(pointwise relative error of the primary
/// component, error of the other components over the largest primary reference amplitude)`.
fn against(kind: Kind, st: &HarmonicStress, refs: &[[C64; 6]]) -> (f64, f64) {
    let pc = primary(kind);
    let (mut pw, mut other, mut peak): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for (e, r) in refs.iter().enumerate() {
        peak = peak.max(r[pc].abs());
        for p in 0..PER_ELEMENT {
            let q = e * PER_ELEMENT + p;
            let d = (comp_of(st, q, pc) - r[pc]).abs() / r[pc].abs();
            if !d.is_finite() {
                return (f64::INFINITY, f64::INFINITY);
            }
            pw = pw.max(d);
            for c in 0..6 {
                if c != pc {
                    let d = (comp_of(st, q, c) - r[c]).abs();
                    if !d.is_finite() {
                        return (f64::INFINITY, f64::INFINITY);
                    }
                    other = other.max(d);
                }
            }
        }
    }
    (pw, other / peak)
}

#[test]
fn s1_s2_s3_rods_against_the_modulus_times_the_chain_strain() {
    let mut c = Checks::new();
    for (name, rod) in [("A", rod_a()), ("B", rod_b()), ("C", rod_c())] {
        let model = rod_model(N, rod.kind, rod.material());
        let (mut e1, mut e2, mut e3) = (Vec::new(), Vec::new(), Vec::new());
        for w in rod.omegas() {
            let (_, st) = solve(&model, &DirectHarmonicOptions::new(), rod.component(), w);
            let m = rod.modulus(w);
            let u = chain_nodes(&vec![m; N], w);
            let eps = element_strains(&u);
            let refs: Vec<[C64; 6]> = eps.iter().map(|&e| reference(rod.kind, m, rod.lateral(w), e)).collect();
            let (pw, other) = against(rod.kind, &st, &refs);
            e1.push(pw);
            e2.push(other);
            // S3: eight points per element, inside the element, equal primary stress
            let h = L / N as f64;
            let pc = primary(rod.kind);
            let mut s3: f64 = if st.positions.len() == PER_ELEMENT * N {
                0.0
            } else {
                f64::INFINITY
            };
            if s3 == 0.0 {
                for e in 0..N {
                    let first = comp_of(&st, e * PER_ELEMENT, pc);
                    for p in 0..PER_ELEMENT {
                        let q = e * PER_ELEMENT + p;
                        let x = st.positions[q][0];
                        if !(x >= e as f64 * h && x <= (e + 1) as f64 * h) {
                            s3 = f64::INFINITY;
                        }
                        s3 = s3.max((comp_of(&st, q, pc) - first).abs() / first.abs());
                    }
                }
            }
            e3.push(s3);
        }
        c.at_most(
            &format!("S1{name} primary stress against the chain, pointwise"),
            worst(&e1),
            1e-9,
        );
        c.at_most(&format!("S2 rod {name} other components"), worst(&e2), 1e-9);
        c.at_most(&format!("S3 rod {name} points of an element"), worst(&e3), 1e-12);
    }
    c.finish();
}

/// The stress of two routes at all points and components, normalised by the largest amplitude of `reference`.
fn normalised(x: &HarmonicStress, reference: &HarmonicStress) -> f64 {
    let (mut num, mut den): (f64, f64) = (0.0, 0.0);
    for q in 0..reference.re.len() {
        for c in 0..6 {
            let d = (comp_of(x, q, c) - comp_of(reference, q, c)).abs();
            if !d.is_finite() {
                return f64::INFINITY;
            }
            num = num.max(d);
            den = den.max(comp_of(reference, q, c).abs());
        }
    }
    num / den
}

#[test]
fn se1_se2_equal_weights_against_the_table_route() {
    let mut c = Checks::new();
    let rod = rod_a();
    let material = rod.material();
    let omegas = rod.omegas();
    let freqs: Vec<f64> = omegas.iter().map(|w| w / (2.0 * PI)).collect();
    let table = modulus_table(&material, &freqs, None).unwrap();
    let visco = rod_model(N, Kind::Axial, material);
    let elastic = rod_model(N, Kind::Axial, base(0.0));
    let ov = DirectHarmonicOptions::new();
    let ot = DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(table));
    let (mut e1, mut e2) = (Vec::new(), Vec::new());
    for &w in &omegas {
        let (_, sv) = solve(&visco, &ov, 0, w);
        let (_, stb) = solve(&elastic, &ot, 0, w);
        e1.push(normalised(&sv, &stb));
        let mut pw: f64 = 0.0;
        for q in 0..stb.re.len() {
            pw = pw.max((comp_of(&sv, q, 0) - comp_of(&stb, q, 0)).abs() / comp_of(&stb, q, 0).abs());
        }
        e2.push(pw);
    }
    println!("  SE1 per frequency: {:?}", e1);
    c.at_most(
        "SE1 viscoelastic route against the table route, normalised",
        worst(&e1),
        1e-12,
    );
    c.at_most("SE2 the same, xx pointwise", worst(&e2), 1e-12);
    c.finish();
}

#[test]
fn sf_z1_t1_c0_p1_two_materials_zero_frequency_shift_no_terms_threads() {
    let mut c = Checks::new();
    let rod = rod_b();

    // SF1, SF2: two materials, the elastic half with a constant loss factor 0.03
    let m0 = C64::from_real(k0(NU_B) + 4.0 / 3.0 * g0(NU_B));
    let l0 = C64::from_real(k0(NU_B) - 2.0 / 3.0 * g0(NU_B));
    let loss = C64::new(1.0, 0.03);
    for (name, elastic_first) in [("SF1", true), ("SF2", false)] {
        let (left, right) = if elastic_first {
            (base(NU_B), rod.material())
        } else {
            (rod.material(), base(NU_B))
        };
        let model = two_rod(N, Kind::Axial, left, right);
        let set = if elastic_first { "left" } else { "right" };
        let options = DirectHarmonicOptions::new().loss_factor(set, LossFactor::Constant(0.03));
        let mut e = Vec::new();
        for w in rod.omegas() {
            let (_, st) = solve(&model, &options, 0, w);
            let (mv, lv) = (rod.modulus(w), rod.lateral(w));
            let (me, le) = (m0 * loss, l0 * loss);
            let per: Vec<(C64, C64)> = (0..N)
                .map(|el| {
                    if (el < N / 2) == elastic_first {
                        (me, le)
                    } else {
                        (mv, lv)
                    }
                })
                .collect();
            let moduli: Vec<C64> = per.iter().map(|p| p.0).collect();
            let eps = element_strains(&chain_nodes(&moduli, w));
            let refs: Vec<[C64; 6]> = (0..N)
                .map(|el| reference(Kind::Axial, per[el].0, per[el].1, eps[el]))
                .collect();
            let (pw, other) = against(Kind::Axial, &st, &refs);
            e.push(pw);
            e.push(other);
        }
        c.at_most(
            &format!("{name} two materials (primary pointwise, others normalised)"),
            worst(&e),
            1e-9,
        );
    }

    // Z1: omega = 0, the static rod has sigma_xx = F/A
    let model = rod_model(N, Kind::Axial, rod.material());
    let (_, st) = solve(&model, &DirectHarmonicOptions::new(), 0, 0.0);
    let mut z: f64 = 0.0;
    for q in 0..st.re.len() {
        z = z.max((comp_of(&st, q, 0).scale(A / F) - C64::ONE).abs());
        if !z.is_finite() {
            break;
        }
    }
    c.at_most("Z1 zero frequency, |sigma_xx A/F - 1|", z, 1e-10);

    // T1: the shifted material at T against the unshifted one with tau a_T
    let a_t = shift_factor(&wlf(), T_RUN).unwrap();
    let shifted = rod_model(N, Kind::Axial, standard(NU_B, G_B, K_B, rod.tau, Some(wlf())));
    let scaled = rod_model(N, Kind::Axial, standard(NU_B, G_B, K_B, rod.tau * a_t, None));
    let mut e = Vec::new();
    for w in rod.omegas() {
        let (_, a) = solve(&shifted, &DirectHarmonicOptions::new().temperature(T_RUN), 0, w);
        let (_, b) = solve(&scaled, &DirectHarmonicOptions::new(), 0, w);
        e.push(normalised(&a, &b));
    }
    c.at_most("T1 shifted against tau a_T", worst(&e), 1e-12);

    // C0: no terms against the base, away from resonance, no damping
    let none = rod_model(N, Kind::Axial, Material::viscoelastic(base(0.0), vec![], None).unwrap());
    let plain = rod_model(N, Kind::Axial, base(0.0));
    let we = PI / 2.0 * (E0 / RHO).sqrt();
    let mut e = Vec::new();
    for w in [0.5 * we, 1.5 * we] {
        let (_, a) = solve(&none, &DirectHarmonicOptions::new(), 0, w);
        let (_, b) = solve(&plain, &DirectHarmonicOptions::new(), 0, w);
        e.push(normalised(&a, &b));
    }
    c.at_most("C0 no terms against the base", worst(&e), 1e-12);

    // P1: threads 1 and 4
    let (_, a) = solve(&model, &DirectHarmonicOptions::new().threads(1), 0, rod.wc);
    let (_, b) = solve(&model, &DirectHarmonicOptions::new().threads(4), 0, rod.wc);
    let bits = |x: &Vec<[f64; 6]>, y: &Vec<[f64; 6]>| {
        x.iter()
            .zip(y)
            .all(|(p, q)| p.iter().zip(q).all(|(u, v)| u.to_bits() == v.to_bits()))
    };
    c.holds(
        "P1 threads 1 and 4",
        bits(&a.re, &b.re) && bits(&a.im, &b.im),
        "bits equal",
    );
    c.finish();
}

#[test]
fn nc1_nc2_rf1_rf2_negative_controls_and_refusals() {
    let mut c = Checks::new();
    for (name, rod) in [("B", rod_b()), ("C", rod_c())] {
        let model = rod_model(N, rod.kind, rod.material());
        let pc = primary(rod.kind);
        let (mut wrong, mut unscaled) = (Vec::new(), Vec::new());
        for w in rod.omegas() {
            let (_, st) = solve(&model, &DirectHarmonicOptions::new(), rod.component(), w);
            let eps = element_strains(&chain_nodes(&vec![rod.modulus(w); N], w));
            let (mut a, mut b): (f64, f64) = (0.0, 0.0);
            for (e, &ep) in eps.iter().enumerate() {
                for p in 0..PER_ELEMENT {
                    let s = comp_of(&st, e * PER_ELEMENT + p, pc);
                    a = a.max((s - rod.wrong_modulus(w) * ep).abs() / s.abs());
                    b = b.max((s - C64::from_real(rod.base_modulus()) * ep).abs() / s.abs());
                }
            }
            wrong.push(a);
            unscaled.push(b);
        }
        c.at_least(
            &format!("NC1 rod {name}: one-part route, min of 10"),
            least(&wrong),
            5e-3,
        );
        c.at_least(
            &format!("NC2 rod {name}: unscaled elastic stress, min of 10"),
            least(&unscaled),
            4e-2,
        );
    }

    // RF1: a response of a 4-element rod with the 40-element model
    let rod = rod_b();
    let model = rod_model(N, Kind::Axial, rod.material());
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let small = rod_model(4, Kind::Axial, rod.material());
    let r = DirectHarmonic::new(&small, &DirectHarmonicOptions::new())
        .unwrap()
        .response(&tip_load(&small, 0), rod.wc)
        .unwrap();
    match solver.stress(&model, &r) {
        Err(FemError::FieldLength {
            key: "displacement", ..
        }) => c.holds("RF1 response of another size", true, "refused"),
        other => c.holds(
            "RF1 response of another size",
            false,
            &format!("{:?}", other.map(|_| ())),
        ),
    }

    // RF2: a model whose materials are not those of the solver
    let other_model = two_rod(N, Kind::Axial, base(NU_B), rod.material());
    let r = solver.response(&tip_load(&model, 0), rod.wc).unwrap();
    match solver.stress(&other_model, &r) {
        Err(FemError::InvalidValue { key: "model", .. }) => c.holds("RF2 another material list", true, "refused"),
        other => c.holds("RF2 another material list", false, &format!("{:?}", other.map(|_| ()))),
    }
    c.finish();
}
