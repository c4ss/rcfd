//! F-413 (ADR 0449): high-cycle fatigue. Checks C1 to C6, H1 to H4, S1, F1 to F5, K1 to K3, Q1 to Q7, B1, B2, R1 to R8 of the
//! registration (X1 is in `sa_scaffold.rs`).

use rcfd_core::rng::Rng;
use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::fatigue::{
    equivalent_history, gamma, harmonic_max_principal, harmonic_von_mises_amplitude, miner, narrow_band_damage_rate,
    narrow_band_damage_rate_curve, rainflow, rainflow_periodic, safety_factor, signed_von_mises, turning_points, Cycle,
    Equivalent, FatigueField, FatigueSpec, MeanStress, MeanStressCorrection, SnCurve,
};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex20::{faces, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::recover::{nodal_average, principal, von_mises, IntegrationPointField};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const SF: f64 = 900e6;
const BB: f64 = -0.09;
const SU: f64 = 600e6;
const SY: f64 = 400e6;
const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const SEED: u64 = 0xF413_0449;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, x: f64, limit: f64) {
        self.holds(name, x <= limit, &format!("{x:.3e} against {limit:.0e}"));
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn rel(x: f64, y: f64) -> f64 {
    (x / y - 1.0).abs()
}

fn basquin() -> SnCurve {
    SnCurve::basquin(SF, BB).unwrap()
}

fn hand_life(s: f64, b: f64) -> f64 {
    (s / SF).powf(1.0 / b) / 2.0
}

fn key_of(e: &FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => key,
        _ => "",
    }
}

fn refused_with(r: rcfd_fem::Result<impl std::fmt::Debug>, key: &str, text: &str) -> (bool, String) {
    match r {
        Ok(v) => (false, format!("accepted: {v:?}")),
        Err(e) => {
            let msg = e.to_string();
            (key_of(&e) == key && msg.contains(text), msg)
        }
    }
}

// ------------------------------------------------------------------------------------------------- C1 to C6

#[test]
fn c1_to_c6_curves_and_corrections() {
    let mut c = Checks::new();
    let amps = [50e6, 150e6, 400e6, 900e6, 1200e6];
    // C1
    let mut worst = 0.0f64;
    let mut worst_inv = 0.0f64;
    for b in [-0.05, -0.09, -0.12] {
        let curve = SnCurve::basquin(SF, b).unwrap();
        for &s in &amps {
            let n = curve.life(s).unwrap();
            worst = worst.max(rel(n, hand_life(s, b)));
            worst_inv = worst_inv.max(rel(curve.strength_at(n).unwrap(), s));
        }
    }
    c.at_most("C1 Basquin life against (s/sf)^(1/b)/2", worst, 1e-12);
    c.at_most("C1 strength_at(life(s)) = s", worst_inv, 1e-12);
    // C2
    let curve = basquin();
    let good = MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU });
    let gerb = MeanStressCorrection::new(MeanStress::Gerber { ultimate: SU });
    let sode = MeanStressCorrection::new(MeanStress::Soderberg { yield_strength: SY });
    let (mut wg, mut wr, mut ws, mut wl) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for sa in [50e6, 150e6, 300e6] {
        for sm in [0.0, 50e6, 150e6, 300e6, 399e6] {
            let g = good.equivalent_amplitude(sa, sm).unwrap();
            wg = wg.max(rel(g, sa / (1.0 - sm / SU)));
            let r = gerb.equivalent_amplitude(sa, sm).unwrap();
            wr = wr.max(rel(r, sa / (1.0 - (sm / SU) * (sm / SU))));
            let s = sode.equivalent_amplitude(sa, sm).unwrap();
            ws = ws.max(rel(s, sa / (1.0 - sm / SY)));
            for (sar, hand) in [
                (g, sa / (1.0 - sm / SU)),
                (r, sa / (1.0 - (sm / SU) * (sm / SU))),
                (s, sa / (1.0 - sm / SY)),
            ] {
                wl = wl.max(rel(curve.life(sar).unwrap(), hand_life(hand, BB)));
            }
        }
    }
    c.at_most("C2 Goodman", wg, 1e-12);
    c.at_most("C2 Gerber", wr, 1e-12);
    c.at_most("C2 Soderberg", ws, 1e-12);
    c.at_most("C2 corrected life = Basquin of the formula", wl, 1e-12);
    // C2c
    let (sa, sm) = (150e6, -200e6);
    let plain = [&good, &gerb, &sode]
        .iter()
        .all(|k| k.equivalent_amplitude(sa, sm).unwrap().to_bits() == sa.to_bits());
    c.holds(
        "C2c compressive mean, no credit: sigma_ar = sigma_a",
        plain,
        "bit-equal",
    );
    let cg = rel(
        good.with_compressive_credit(true).equivalent_amplitude(sa, sm).unwrap(),
        sa / (1.0 + 1.0 / 3.0),
    );
    let cs = rel(
        sode.with_compressive_credit(true).equivalent_amplitude(sa, sm).unwrap(),
        sa / 1.5,
    );
    let cr = rel(
        gerb.with_compressive_credit(true).equivalent_amplitude(sa, sm).unwrap(),
        sa / (1.0 - 1.0 / 9.0),
    );
    c.at_most("C2c with credit Goodman", cg, 1e-12);
    c.at_most("C2c with credit Soderberg", cs, 1e-12);
    c.at_most("C2c with credit Gerber", cr, 1e-12);
    // C3
    for (name, k, sm) in [
        ("Goodman at sigma_u", good, SU),
        ("Goodman at 1.1 sigma_u", good, 1.1 * SU),
        ("Gerber at sigma_u", gerb, SU),
        (
            "Gerber with credit at -sigma_u",
            gerb.with_compressive_credit(true),
            -SU,
        ),
        ("Soderberg at sigma_y", sode, SY),
    ] {
        let (ok, msg) = refused_with(
            k.equivalent_amplitude(100e6, sm),
            "mean stress",
            "at or above the strength",
        );
        c.holds(&format!("C3 {name} refused"), ok, &msg);
    }
    let accepted = good.equivalent_amplitude(100e6, 0.999 * SU).is_ok()
        && gerb.equivalent_amplitude(100e6, 0.999 * SU).is_ok()
        && gerb
            .with_compressive_credit(true)
            .equivalent_amplitude(100e6, -0.999 * SU)
            .is_ok()
        && sode.equivalent_amplitude(100e6, 0.999 * SY).is_ok();
    c.holds("C3 accepted at 0.999 of each strength", accepted, "");
    // C4
    let lives: [f64; 3] = [1e3, 1e5, 1e7];
    let tab_s: Vec<f64> = lives.iter().map(|&n| SF * (2.0 * n).powf(BB)).collect();
    let table = SnCurve::table(&lives, &tab_s).unwrap();
    let mut wt = 0.0f64;
    for (n, s) in lives.iter().zip(&tab_s) {
        wt = wt.max(rel(table.life(*s).unwrap(), *n));
    }
    let mut wb = 0.0f64;
    for i in 0..20 {
        let s = 30e6 * (1500e6 / 30e6f64).powf(i as f64 / 19.0);
        wb = wb.max(rel(table.life(s).unwrap(), hand_life(s, BB)));
    }
    c.at_most("C4 table at its points", wt, 1e-12);
    c.at_most("C4 table of Basquin points = Basquin, 30 to 1500 MPa", wb, 1e-12);
    // C5
    let endu = basquin().with_endurance(100e6).unwrap();
    let ok5 = endu.life(100e6).unwrap().is_infinite()
        && endu.life(99e6).unwrap().is_infinite()
        && endu.life(101e6).unwrap() == curve.life(101e6).unwrap()
        && endu.strength_at(1e12).unwrap() == 100e6;
    c.holds(
        "C5 endurance limit",
        ok5,
        &format!(
            "life(101 MPa) {:e}, strength_at(1e12) {:e}",
            endu.life(101e6).unwrap(),
            endu.strength_at(1e12).unwrap()
        ),
    );
    // C6
    let m = -1.0 / BB;
    let cpow = SF.powf(m) / 2.0;
    let pw = SnCurve::power(m, cpow).unwrap();
    let mut wp = 0.0f64;
    for &s in &amps {
        wp = wp.max(rel(pw.life(s).unwrap(), curve.life(s).unwrap()));
    }
    c.at_most("C6 power law of Basquin = Basquin", wp, 1e-12);
    c.holds(
        "C6 power_law of Basquin",
        curve.power_law() == Some((m, cpow)),
        &format!("{:?}", curve.power_law()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- H1 to H4

fn sym(rng: &mut Rng, scale: f64) -> [f64; 6] {
    std::array::from_fn(|_| rng.range(-1.0, 1.0) * scale)
}

#[test]
fn h1_to_h4_equivalent_stresses() {
    let mut c = Checks::new();
    // H1
    let mut w = 0.0f64;
    let mut wi = 0.0f64;
    for (s, t) in [
        (100e6, 20e6),
        (100e6, 100e6 / 3f64.sqrt()),
        (100e6, 80e6),
        (0.0, 50e6),
        (70e6, 0.0),
    ] {
        let a = harmonic_von_mises_amplitude(&[s, 0.0, 0.0, 0.0, 0.0, 0.0], &[0.0, 0.0, 0.0, t, 0.0, 0.0]);
        w = w.max(rel(a, s.max(3f64.sqrt() * t)));
        let ip = harmonic_von_mises_amplitude(&[s, 0.0, 0.0, t, 0.0, 0.0], &[0.0; 6]);
        wi = wi.max(rel(ip, (s * s + 3.0 * t * t).sqrt()));
    }
    c.at_most("H1 90 degrees out of phase: max(s, sqrt 3 t)", w, 1e-10);
    c.at_most("H1 in phase: sqrt(s^2 + 3 t^2)", wi, 1e-10);
    // H2
    let mut rng = Rng::new(SEED);
    let (mut above, mut below) = (0.0f64, 0.0f64);
    for _ in 0..200 {
        let a = sym(&mut rng, 100e6);
        let b = sym(&mut rng, 100e6);
        let amp = harmonic_von_mises_amplitude(&a, &b);
        let mut sampled = 0.0f64;
        for j in 0..200_000 {
            let th = 2.0 * PI * j as f64 / 200_000.0;
            let (sn, cs) = th.sin_cos();
            let s: [f64; 6] = std::array::from_fn(|k| a[k] * cs - b[k] * sn);
            sampled = sampled.max(von_mises(&s));
        }
        above = above.max(sampled / amp - 1.0);
        below = below.max(amp / sampled - 1.0);
    }
    c.at_most("H2 sampled maximum above the closed form", above, 1e-12);
    c.at_most("H2 closed form above the sampled maximum", below, 1e-8);
    // H3
    let mut w3 = 0.0f64;
    for (s, s2) in [(100e6, 60e6), (60e6, 100e6)] {
        let (a, _) = harmonic_max_principal(&[s, 0.0, 0.0, 0.0, 0.0, 0.0], &[0.0, s2, 0.0, 0.0, 0.0, 0.0]);
        w3 = w3.max(rel(a, s.max(s2)));
    }
    let (ash, _) = harmonic_max_principal(&[0.0, 0.0, 0.0, 70e6, 0.0, 0.0], &[0.0; 6]);
    w3 = w3.max(rel(ash, 70e6));
    for _ in 0..50 {
        let s = sym(&mut rng, 100e6);
        let (l, mu) = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        let re = s.map(|x| l * x);
        let im = s.map(|x| mu * x);
        let p = principal(&s);
        let expect = l.hypot(mu) * p.values[0].abs().max(p.values[2].abs());
        w3 = w3.max(rel(harmonic_max_principal(&re, &im).0, expect));
    }
    c.at_most("H3 max principal amplitude, closed forms", w3, 1e-9);
    // H4
    let s = 123e6;
    let u = rel(signed_von_mises(&[s, 0.0, 0.0, 0.0, 0.0, 0.0]), s)
        .max(rel(signed_von_mises(&[-s, 0.0, 0.0, 0.0, 0.0, 0.0]), -s));
    c.at_most("H4 signed von Mises of +-sigma", u, 1e-15);
    let shear = signed_von_mises(&[0.0, 0.0, 0.0, s, 0.0, 0.0]) > 0.0;
    let hydro = signed_von_mises(&[-s, -s, -s, 0.0, 0.0, 0.0]) == 0.0;
    c.holds("H4 pure shear positive, hydrostatic 0", shear && hydro, "");
    let spec = FatigueSpec::new(basquin(), MeanStressCorrection::none(), Equivalent::MaxPrincipal);
    let pf = spec
        .evaluate_harmonic(
            &[80e6, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[100e6, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[0.0; 6],
        )
        .unwrap();
    c.at_most("H4 MaxPrincipal mean of a uniaxial mean", rel(pf.mean, 80e6), 1e-12);
    let hist: Vec<[f64; 6]> = (0..50)
        .map(|k| [100e6 * (k as f64 * 0.3).sin() + 20e6, 0.0, 0.0, 0.0, 0.0, 0.0])
        .collect();
    let mut wh = 0.0f64;
    for eq in [Equivalent::VonMises, Equivalent::MaxPrincipal] {
        for (v, s) in equivalent_history(&hist, eq).iter().zip(&hist) {
            wh = wh.max((v - s[0]).abs() / s[0].abs().max(1.0));
        }
    }
    c.at_most("H4 equivalent_history of a uniaxial history", wh, 1e-15);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1

#[test]
fn s1_safety_factor() {
    let mut c = Checks::new();
    let curve = basquin();
    let nd: f64 = 1e7;
    let strength = SF * (2.0 * nd).powf(BB);
    let kinds = [
        ("None", MeanStressCorrection::none()),
        (
            "Goodman",
            MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU }),
        ),
        ("Gerber", MeanStressCorrection::new(MeanStress::Gerber { ultimate: SU })),
        (
            "Soderberg",
            MeanStressCorrection::new(MeanStress::Soderberg { yield_strength: SY }),
        ),
    ];
    let mut w = 0.0f64;
    for (_, k) in &kinds {
        for (sa, sm) in [(150e6, 100e6), (150e6, -100e6)] {
            let l = safety_factor(&curve, k, sa, sm, nd).unwrap();
            let life = curve.life(k.equivalent_amplitude(l * sa, l * sm).unwrap()).unwrap();
            w = w.max(rel(life, nd));
        }
    }
    c.at_most("S1 life at lambda times the stress = design life", w, 1e-10);
    let lg = safety_factor(&curve, &kinds[1].1, 150e6, 100e6, nd).unwrap();
    c.at_most(
        "S1 Goodman lambda closed form",
        rel(lg, strength / (150e6 + strength * 100e6 / SU)),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- F1 to F5

const CL: f64 = 0.5;
const CB: f64 = 0.05;
const CH: f64 = 0.05;
const P_S: f64 = 100e6 * CB * CH * CH / (6.0 * CL);
const P_A: f64 = 150e6 * CB * CH * CH / (6.0 * CL);

fn cantilever(p: f64) -> SolidModel {
    let (mut mesh, _) = upgrade_to_hex20(&hex_box([0.0; 3], [CL, CB, CH], [20, 2, 4]).unwrap()).unwrap();
    let neutral: Vec<usize> = mesh
        .node_set("xmin")
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| (mesh.nodes()[n][2] - 0.5 * CH).abs() <= 1e-12)
        .collect();
    let y0: Vec<usize> = neutral
        .iter()
        .copied()
        .filter(|&n| mesh.nodes()[n][1].abs() <= 1e-12)
        .collect();
    assert_eq!(y0.len(), 1);
    mesh.add_node_set("root_neutral", neutral).unwrap();
    mesh.add_node_set("root_neutral_y", y0).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_neutral", 2, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("root_neutral_y", 1, 0.0))
        .unwrap();
    m.add_load(Load::traction("xmax", [0.0, 0.0, -p / (CB * CH)])).unwrap();
    m
}

fn nodal(model: &SolidModel, field: &IntegrationPointField, v: &[[f64; 6]]) -> Vec<[f64; 6]> {
    nodal_average(model, field, v, None)
        .unwrap()
        .into_iter()
        .map(|x| x.unwrap())
        .collect()
}

#[test]
fn f1_to_f5_cantilever() {
    let mut c = Checks::new();
    let t0 = std::time::Instant::now();
    let ms = cantilever(P_S);
    let mh = cantilever(P_A);
    let res = solve_static(&ms, &StaticOptions::default()).unwrap();
    let field = IntegrationPointField::from_result(&ms, &res).unwrap();
    let i = CB * CH.powi(3) / 12.0;
    let w1 = 1.875104f64.powi(2) * (E_MOD * i / (RHO * CB * CH * CL.powi(4))).sqrt();
    let w = 0.01 * w1;
    let dh = DirectHarmonic::new(&mh, &DirectHarmonicOptions::new()).unwrap();
    let resp = dh.response(&HarmonicLoad::from_model(&mh).unwrap(), w).unwrap();
    let st = dh.stress(&mh, &resp).unwrap();
    println!(
        "cantilever: {} unknowns, w = {w:.4} rad/s, {:.2} s",
        resp.re.len(),
        t0.elapsed().as_secs_f64()
    );
    let spec = FatigueSpec::new(
        basquin(),
        MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU }),
        Equivalent::VonMises,
    );
    let f_hz = w / (2.0 * PI);
    let (nm, nr, ni) = (
        nodal(&ms, &field, field.stress()),
        nodal(&ms, &field, &st.re),
        nodal(&ms, &field, &st.im),
    );
    let fld = FatigueField::harmonic(&spec, Some(&nm), &nr, &ni, f_hz).unwrap();
    let p = fld.largest_damage().unwrap();
    let x = ms.mesh().nodes()[p];
    c.holds(
        "F1 largest damage on the root surface (x = 0, z = h)",
        x[0].abs() <= 1e-12 && (x[2] - CH).abs() <= 1e-12,
        &format!("node {p} at {x:?}"),
    );
    let sa = 6.0 * P_A * CL / (CB * CH * CH);
    let sm = 6.0 * P_S * CL / (CB * CH * CH);
    let n_hand = (sa / (1.0 - sm / SU) / SF).powf(1.0 / BB) / 2.0;
    let r = fld.points[p].life / n_hand;
    c.holds(
        "F2 life over the hand life in [0.767541, 1.308433]",
        (0.767541..=1.308433).contains(&r),
        &format!("{r:.6} (life {:.6e}, hand {n_hand:.6e})", fld.points[p].life),
    );
    c.at_most(
        "F3 amplitude against 6 P_a L/(b h^2)",
        rel(fld.points[p].amplitude, sa),
        0.02,
    );
    c.at_most("F3 mean against 6 P_s L/(b h^2)", rel(fld.points[p].mean, sm), 0.02);
    let ip = FatigueField::harmonic(&spec, Some(field.stress()), &st.re, &st.im, f_hz).unwrap();
    let q = ip.largest_damage().unwrap();
    let y: Vec3 = field.positions()[q];
    c.holds(
        "F4 integration point of largest damage x < L/20, z > 3h/4",
        y[0] < CL / 20.0 && y[2] > 0.75 * CH,
        &format!("point {q} at {y:?}"),
    );
    let pt = spec.evaluate_harmonic(&nm[p], &nr[p], &ni[p]).unwrap();
    c.holds("F5 field equals evaluate_harmonic", pt == fld.points[p], "bit-equal");
    c.at_most(
        "F5 damage rate = f/life",
        rel(fld.damage_rate(p), f_hz / fld.points[p].life),
        1e-15,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- K1 to K3

const HR: f64 = 0.005;
const HA: f64 = 0.1;
const HB: f64 = 0.05;
const HT: f64 = 0.001;
const S0: f64 = 50e6;

fn plate_map(p: Vec3) -> Vec3 {
    let (s, q) = (p[0], p[1]);
    let (circle, outer) = if q == 1.0 {
        ([0.0, HR], [0.0, HB])
    } else {
        let th = 0.5 * PI * q;
        let ell = q * (HA + HB);
        let o = if ell <= HB { [HA, ell] } else { [HA - (ell - HB), HB] };
        ([HR * th.cos(), HR * th.sin()], o)
    };
    let g = ((4.0 * s).exp() - 1.0) / (4f64.exp() - 1.0);
    [
        circle[0] + g * (outer[0] - circle[0]),
        circle[1] + g * (outer[1] - circle[1]),
        p[2],
    ]
}

fn plate() -> SolidModel {
    let (mut mesh, _) =
        upgrade_to_hex20_mapped(&hex_box([0.0; 3], [1.0, 1.0, HT], [24, 24, 1]).unwrap(), plate_map).unwrap();
    let right: Vec<_> = mesh
        .surface_set("xmax")
        .unwrap()
        .iter()
        .copied()
        .filter(|f| {
            let nodes = mesh.element_nodes(f.element);
            faces()[f.face]
                .iter()
                .all(|&a| (mesh.nodes()[nodes[a]][0] - HA).abs() <= 1e-12)
        })
        .collect();
    assert_eq!(right.len(), 8, "faces on x = A");
    mesh.add_surface_set("right", right).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("right", [S0, 0.0, 0.0])).unwrap();
    m
}

#[test]
fn k1_to_k3_plate_with_a_hole() {
    let mut c = Checks::new();
    let t0 = std::time::Instant::now();
    let model = plate();
    let mesh: &SolidMesh = model.mesh();
    let dh = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let resp = dh.response(&HarmonicLoad::from_model(&model).unwrap(), 100.0).unwrap();
    let st = dh.stress(&model, &resp).unwrap();
    println!(
        "plate: {} elements, {} nodes, {} unknowns, {:.2} s",
        mesh.n_elements(),
        mesh.nodes().len(),
        resp.re.len(),
        t0.elapsed().as_secs_f64()
    );
    let field = IntegrationPointField::compute(&model, &resp.re).unwrap();
    let (nr, ni) = (nodal(&model, &field, &st.re), nodal(&model, &field, &st.im));
    let edge: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&n| {
            let x = mesh.nodes()[n];
            x[0].abs() <= 1e-9 && (x[1] - HR).abs() <= 1e-9
        })
        .collect();
    let sxx = edge.iter().map(|&n| nr[n][0].hypot(ni[n][0])).fold(0.0, f64::max);
    println!("hole edge nodes {edge:?}");
    c.at_most(
        "K1 s_xx amplitude at (0, r) against 3 sigma_0",
        rel(sxx, 3.0 * S0),
        0.03,
    );
    let spec = FatigueSpec::new(basquin(), MeanStressCorrection::none(), Equivalent::VonMises);
    let fld = FatigueField::harmonic(&spec, None, &nr, &ni, 100.0 / (2.0 * PI)).unwrap();
    let p = fld.largest_damage().unwrap();
    let x = mesh.nodes()[p];
    c.holds(
        "K2 largest damage at (0, r)",
        x[0].abs() <= 1e-9 && (x[1] - HR).abs() <= 1e-9,
        &format!("node {p} at {x:?}"),
    );
    let pt = fld.points[p];
    c.at_most(
        "K3 life = Basquin life of the von Mises amplitude",
        rel(pt.life, basquin().life(pt.amplitude).unwrap()),
        1e-12,
    );
    c.at_most(
        "K3 von Mises amplitude against 3 sigma_0",
        rel(pt.amplitude, 3.0 * S0),
        0.03,
    );
    let r = pt.life / hand_life(3.0 * S0, BB);
    c.holds(
        "K3 life over N(3 sigma_0) in [0.720053, 1.402751]",
        (0.720053..=1.402751).contains(&r),
        &format!("{r:.6}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- Q1 to Q7

fn key(cycles: &[Cycle], negate: bool) -> Vec<(u64, u64, u64)> {
    let mut v: Vec<(u64, u64, u64)> = cycles
        .iter()
        .map(|c| {
            let m = if negate { -c.mean } else { c.mean } + 0.0;
            (c.range.to_bits(), m.to_bits(), c.count.to_bits())
        })
        .collect();
    v.sort_unstable();
    v
}

fn rotate_to_max(x: &[f64]) -> Vec<f64> {
    let mut k = 0;
    for (i, v) in x.iter().enumerate() {
        if *v > x[k] {
            k = i;
        }
    }
    x[k..].iter().chain(&x[..k]).copied().collect()
}

fn ripples(m: usize, s: f64) -> Vec<f64> {
    let d = 0.05 * s;
    let mut h = vec![-s];
    for i in 1..=m {
        let l = -s + 2.0 * s * i as f64 / (m as f64 + 1.0);
        h.push(l);
        h.push(l - d);
    }
    h.push(s);
    h
}

fn random_history(rng: &mut Rng, lo: usize, hi: usize) -> Vec<f64> {
    let n = lo + rng.below(hi - lo + 1);
    (0..n).map(|_| rng.range(-1.0, 1.0) * 100e6).collect()
}

#[test]
fn q1_to_q7_counting() {
    let mut c = Checks::new();
    let s = 120e6;
    // Q1
    let mut ok1 = true;
    let mut detail = String::new();
    let mut constructed: Vec<Vec<f64>> = Vec::new();
    for n in [1usize, 7, 1000] {
        let h: Vec<f64> = (0..n).flat_map(|_| [s, -s]).collect();
        let r = rainflow_periodic(&h).unwrap();
        let good = r.cycles.len() == n
            && r.cycles
                .iter()
                .all(|c| c.range == 2.0 * s && c.mean == 0.0 && c.count == 1.0)
            && r.residue.is_empty();
        ok1 &= good;
        detail += &format!("n {n}: {} cycles; ", r.cycles.len());
        constructed.push(h);
    }
    c.holds("Q1 [S, -S] x n gives n cycles of 2S", ok1, &detail);
    let sine: Vec<f64> = (0..16 * 50).map(|k| s * (2.0 * PI * k as f64 / 16.0).sin()).collect();
    let r = rainflow_periodic(&sine).unwrap();
    let wr = r.cycles.iter().map(|c| rel(c.range, 2.0 * s)).fold(0.0, f64::max);
    c.holds(
        "Q1 sampled sine: 50 cycles of 2S within 1e-12",
        r.cycles.len() == 50 && wr <= 1e-12,
        &format!("{} cycles, worst {wr:.2e}", r.cycles.len()),
    );
    constructed.push(sine);
    // Q2
    let mut ok2 = true;
    let mut d2 = String::new();
    for m in [1usize, 4, 25] {
        let h = ripples(m, s);
        let r = rainflow_periodic(&h).unwrap();
        let big = r.cycles.iter().filter(|c| c.range == 2.0 * s).count();
        let small = r
            .cycles
            .iter()
            .filter(|c| (c.range - 0.05 * s).abs() <= 1e-12 * s)
            .count();
        ok2 &= big == 1 && small == m && r.cycles.len() == m + 1;
        d2 += &format!("m {m}: {big} big, {small} small of {}; ", r.cycles.len());
        constructed.push(h);
    }
    c.holds("Q2 large cycle with m ripples gives 1 and m", ok2, &d2);
    // Q3
    let mut rng = Rng::new(SEED);
    let mut q3 = constructed.clone();
    for _ in 0..20 {
        q3.push(random_history(&mut rng, 12, 60));
    }
    let mut ok3 = true;
    for h in &q3 {
        let h = rotate_to_max(h);
        let p = rainflow_periodic(&h).unwrap();
        ok3 &= p.residue.is_empty() && p.cycles.iter().all(|c| c.count == 1.0);
        let mut closed = h.clone();
        closed.push(h[0]);
        let plain = rainflow(&closed).unwrap();
        let max = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min = h.iter().copied().fold(f64::INFINITY, f64::min);
        ok3 &= plain.residue == vec![max, min, max];
        let k = key(&p.cycles, false);
        for sft in 1..h.len() {
            let rot: Vec<f64> = h[sft..].iter().chain(&h[..sft]).copied().collect();
            ok3 &= key(&rainflow_periodic(&rot).unwrap().cycles, false) == k;
        }
    }
    c.holds(
        "Q3 started at the maximum: no residual; plain residue [M, min, M]; rotation invariant",
        ok3,
        &format!("{} histories", q3.len()),
    );
    // Q4
    let mut q4 = q3.clone();
    for _ in 0..200 {
        q4.push(random_history(&mut rng, 3, 40));
    }
    let (mut neg_ok, mut rev_ok) = (true, true);
    for h in &q4 {
        let k = key(&rainflow_periodic(h).unwrap().cycles, false);
        let neg: Vec<f64> = h.iter().map(|v| -v).collect();
        neg_ok &= key(&rainflow_periodic(&neg).unwrap().cycles, true) == k;
        let rev: Vec<f64> = h.iter().rev().copied().collect();
        rev_ok &= key(&rainflow_periodic(&rev).unwrap().cycles, false) == k;
    }
    c.holds(
        "Q4 negated history: same ranges, negated means",
        neg_ok,
        &format!("{} histories", q4.len()),
    );
    c.holds("Q4 reversed history: same counts", rev_ok, "");
    // Q5
    let (s1, n1, s2, n2) = (300e6, 1000usize, 150e6, 100_000usize);
    let h: Vec<f64> = (0..n1)
        .flat_map(|_| [s1, -s1])
        .chain((0..n2).flat_map(|_| [s2, -s2]))
        .collect();
    let r = rainflow_periodic(&h).unwrap();
    let c1 = r.cycles.iter().filter(|c| c.range == 2.0 * s1).count();
    let c2 = r.cycles.iter().filter(|c| c.range == 2.0 * s2).count();
    c.holds(
        "Q5 counts n1 of 2 S1 and n2 of 2 S2",
        c1 == n1 && c2 == n2 && r.cycles.len() == n1 + n2,
        &format!("{c1}, {c2} of {}", r.cycles.len()),
    );
    let d = miner(&r.cycles, &basquin(), &MeanStressCorrection::none()).unwrap();
    let hand = n1 as f64 / hand_life(s1, BB) + n2 as f64 / hand_life(s2, BB);
    c.at_most("Q5 Miner = n1/N1 + n2/N2", rel(d, hand), 1e-12);
    // Q6
    let r = rainflow(&[-200e6, 100e6, -100e6, 200e6]).unwrap();
    let ok6 = r.cycles.len() == 1
        && r.cycles[0].range == 200e6
        && r.cycles[0].mean == 0.0
        && r.residue == vec![-200e6, 200e6];
    c.holds("Q6 one cycle and residue [-200, 200]", ok6, &format!("{r:?}"));
    let d6 = miner(&r.with_half_cycles(), &basquin(), &MeanStressCorrection::none()).unwrap();
    let h6 = 1.0 / hand_life(100e6, BB) + 0.5 / hand_life(200e6, BB);
    c.at_most("Q6 Miner with half cycles", rel(d6, h6), 1e-12);
    // Q7
    let ok7 = turning_points(&[0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.0]).unwrap() == vec![0.0, 2.0, 0.0]
        && turning_points(&[0.0, 1.0, 2.0, 3.0]).unwrap() == vec![0.0, 3.0];
    c.holds("Q7 turning points", ok7, "");
    c.finish();
}

// ------------------------------------------------------------------------------------------------- B1, B2

#[test]
fn b1_narrow_band_function() {
    let mut c = Checks::new();
    let sp = PI.sqrt();
    let exact = [
        (3.0, 0.75 * sp),
        (4.0, 2.0),
        (5.0, 15.0 / 8.0 * sp),
        (6.0, 6.0),
        (9.0, 945.0 / 32.0 * sp),
        (10.0, 120.0),
    ];
    let (nu0, s, cc) = (12.5, 40e6, 1e30);
    let mut w = 0.0f64;
    for (m, g) in exact {
        let f = narrow_band_damage_rate(nu0, s, m, cc).unwrap();
        w = w.max(rel(f, nu0 * (2f64.sqrt() * s).powf(m) * g / cc));
    }
    c.at_most("B1 nu0 (sqrt 2 s)^m Gamma(1 + m/2)/C", w, 1e-12);
    let mut wg = 0.0f64;
    for x in [0.3, 1.7, 3.37, 7.3, 12.9, 20.5] {
        wg = wg.max(rel(gamma(x + 1.0).unwrap(), x * gamma(x).unwrap()));
    }
    c.at_most("B1 Gamma(x + 1) = x Gamma(x)", wg, 1e-13);
    c.finish();
}

/// In-place radix-2 transform `y_n = sum_k x_k e^{+2 pi i k n/N}` (Cooley and Tukey 1965).
fn fft_inverse(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let tw: Vec<(f64, f64)> = (0..n / 2).map(|k| (2.0 * PI * k as f64 / n as f64).sin_cos()).collect();
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sn, cs) = tw[k * step];
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * cs - im[b] * sn, re[b] * sn + im[b] * cs);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

#[test]
fn b2_synthesised_narrow_band() {
    let mut c = Checks::new();
    let n = 1usize << 22;
    let (k0, k1) = (128_450usize, 133_693usize);
    let mut rng = Rng::new(SEED);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    let (mut w, mut wk2) = (0.0, 0.0);
    for k in k0..=k1 {
        let (u1, u2) = (rng.uniform(), rng.uniform());
        let r = (-2.0 * (1.0 - u1).ln()).sqrt();
        let (sn, cs) = (2.0 * PI * u2).sin_cos();
        re[k] = r * cs;
        im[k] = r * sn;
        let a2 = r * r;
        w += a2;
        wk2 += (k * k) as f64 * a2;
    }
    fft_inverse(&mut re, &mut im);
    let x = re;
    let s2 = x.iter().map(|v| v * v).sum::<f64>() / n as f64;
    let nu0_t = (wk2 / w).sqrt();
    let curve = SnCurve::power(3.0, 1e30).unwrap();
    let counts = rainflow_periodic(&x).unwrap();
    let d = miner(&counts.cycles, &curve, &MeanStressCorrection::none()).unwrap();
    let theory = narrow_band_damage_rate_curve(nu0_t, s2.sqrt(), &curve).unwrap();
    println!(
        "B2: {} cycles, nu0 T {nu0_t:.1}, s {:.4}, counted {d:.6e}, theory {theory:.6e}",
        counts.cycles.len(),
        s2.sqrt()
    );
    c.holds(
        "B2 at least 1e5 cycles",
        counts.cycles.len() >= 100_000,
        &format!("{}", counts.cycles.len()),
    );
    c.at_most("B2 counted damage against the narrow-band rate", rel(d, theory), 0.05);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1 to R8

#[test]
fn r1_to_r8_refusals() {
    let mut c = Checks::new();
    let curve_refusals: Vec<(&str, rcfd_fem::Result<SnCurve>)> = vec![
        ("sigma_f 0", SnCurve::basquin(0.0, BB)),
        ("b 0", SnCurve::basquin(SF, 0.0)),
        ("b NaN", SnCurve::basquin(SF, f64::NAN)),
        ("one point", SnCurve::table(&[1e3], &[300e6])),
        ("not ascending", SnCurve::table(&[1e5, 1e3], &[200e6, 100e6])),
        ("not descending", SnCurve::table(&[1e3, 1e5], &[100e6, 200e6])),
    ];
    for (name, r) in curve_refusals {
        let (ok, msg) = refused_with(r, "stress-life curve", "");
        c.holds(&format!("R1 {name}"), ok, &msg);
    }
    let (ok, msg) = refused_with(basquin().life(-1.0), "stress-life curve", "");
    c.holds("R2 negative amplitude", ok, &msg);
    let (ok, msg) = refused_with(rainflow(&[0.0, f64::NAN, 1.0]), "history", "sample 1");
    c.holds("R3 NaN in a history", ok, &msg);
    let spec = FatigueSpec::new(
        basquin(),
        MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU }),
        Equivalent::VonMises,
    );
    let z = [[0.0; 6]; 3];
    let r4 = FatigueField::harmonic(&spec, None, &z, &z[..2], 1.0);
    c.holds(
        "R4 field lengths differ",
        matches!(r4, Err(FemError::FieldLength { .. })),
        &format!("{r4:?}"),
    );
    let endu = basquin().with_endurance(50e6).unwrap();
    let table = SnCurve::table(&[1e3, 1e5], &[300e6, 200e6]).unwrap();
    for (name, r) in [
        ("rms < 0", narrow_band_damage_rate(1.0, -1.0, 3.0, 1.0)),
        ("nu0 < 0", narrow_band_damage_rate(-1.0, 1.0, 3.0, 1.0)),
        ("table", narrow_band_damage_rate_curve(1.0, 1.0, &table)),
        ("endurance", narrow_band_damage_rate_curve(1.0, 1.0, &endu)),
    ] {
        let (ok, msg) = refused_with(r, "narrow band", "");
        c.holds(&format!("R5 {name}"), ok, &msg);
    }
    let mean = [[0.0; 6], [SU, 0.0, 0.0, 0.0, 0.0, 0.0], [0.0; 6]];
    let re = [[10e6, 0.0, 0.0, 0.0, 0.0, 0.0]; 3];
    let (ok, msg) = refused_with(
        FatigueField::harmonic(&spec, Some(&mean), &re, &z, 1.0),
        "mean stress",
        "point 1",
    );
    c.holds("R6 field point at the strength", ok, &msg);
    let (ok, msg) = refused_with(FatigueField::harmonic(&spec, None, &re, &z, -1.0), "frequency", "");
    c.holds("R7 negative frequency", ok, &msg);
    let (ok, msg) = refused_with(
        safety_factor(&basquin(), &MeanStressCorrection::none(), 100e6, 0.0, 0.0),
        "design life",
        "",
    );
    c.holds("R8 design life 0", ok, &msg);
    c.finish();
}
