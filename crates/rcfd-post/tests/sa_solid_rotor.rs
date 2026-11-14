//! F-416 (ADR 0476): the flexible rotor joined to the rigid rotor of ADR 0120. Checks K1 to K4, S1 to S3 and the negative
//! control N2 of the registration (B2 is in `sa_solid_rotor_default_bits.rs`).

use rcfd_fem::beam::circle_section;
use rcfd_fem::builders::hex_box;
use rcfd_fem::rotating::Whirl;
use rcfd_fem::rotor::{Bearing as FlexBearing, Disc, RotorModel};
use rcfd_fem::{FemError, Material, SolidModel};
use rcfd_post::rotordyn::{
    self, critical_speeds, natural_frequencies, response, unbalance_load, PointUnbalance, Rotor,
};
use rcfd_post::solid_rotor::{disc_from_solid, flexible_from_rigid, nodal_load, rigid_from_flexible, Shaft};

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
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

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn key_of<T: std::fmt::Debug>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

/// The rotor K of ADR 0476 (`damped = false`: K0).
fn rotor_k(damped: bool) -> Rotor {
    let c = if damped { 1.0 } else { 0.0 };
    Rotor {
        mass: 1.0,
        it: 4e-3,
        ip: 2e-3,
        bearings: [
            rotordyn::Bearing {
                z: -0.04,
                k: 2e5,
                c: 20.0 * c,
                eta: 0.0,
            },
            rotordyn::Bearing {
                z: 0.06,
                k: 3e5,
                c: 30.0 * c,
                eta: 0.0,
            },
        ],
    }
}

fn stiff_shaft(e: f64) -> Shaft {
    Shaft {
        section: circle_section(0.05, 0.9).unwrap(),
        material: Material::isotropic_elastic(e, 0.3, 0.0, 0.0).unwrap(),
    }
}

fn unbalances() -> [PointUnbalance; 2] {
    [
        PointUnbalance {
            mass: 1e-3,
            radius: 0.01,
            angle: 0.0,
            z: -0.02,
        },
        PointUnbalance {
            mass: 1e-3,
            radius: 0.01,
            angle: 2.0,
            z: 0.03,
        },
    ]
}

fn centre(m: &RotorModel) -> usize {
    m.stations().iter().position(|z| *z == 0.0).unwrap()
}

/// K1's measure: the forward critical speeds against rotordyn's (`None` if the counts differ).
fn k1_measure(e: f64) -> Option<f64> {
    let r = rotor_k(true);
    let rd = critical_speeds(&r).unwrap();
    let flex = flexible_from_rigid(&r, &stiff_shaft(e)).unwrap();
    let cs = flex.critical_speeds().unwrap();
    let fw: Vec<f64> = cs
        .iter()
        .filter(|s| s.direction == Whirl::Forward)
        .map(|s| s.speed)
        .collect();
    println!("  E = {e:e}: rotordyn {rd:?}, forward {fw:?}");
    (fw.len() == rd.len()).then(|| fw.iter().zip(&rd).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max))
}

#[test]
fn k1_to_k4_n2_stiff_shaft_limit() {
    let mut c = Checks::new();
    let e = 5e14;
    let r = rotor_k(true);
    let rd = critical_speeds(&r).unwrap();
    match k1_measure(e) {
        Some(d) => c.at_most("K1 forward critical speeds against rotordyn", d, 1e-6),
        None => c.holds("K1 count", false, "the forward count differs from rotordyn's"),
    }
    let flex = flexible_from_rigid(&r, &stiff_shaft(e)).unwrap();
    let cs = flex.critical_speeds().unwrap();
    let bw: Vec<f64> = cs
        .iter()
        .filter(|s| s.direction == Whirl::Backward)
        .map(|s| s.speed)
        .collect();
    let rec = [425.53, 743.14];
    c.holds("K4 two backward critical speeds", bw.len() == 2, &format!("{bw:?}"));
    if bw.len() == 2 {
        for (a, b) in bw.iter().zip(rec) {
            c.at_most(&format!("K4 backward {a:.6} against {b}"), rel(*a, b), 1e-4);
        }
    }
    let cn = centre(&flex);
    for f in [0.5, 1.0, 2.0] {
        let spin = f * rd[0];
        let l = unbalance_load(&unbalances(), spin).unwrap();
        let x_rd = response(&r, spin, &[l]).unwrap()[0].disp;
        let x = flex
            .harmonic_response(spin, spin, &[(cn, nodal_load(&l))])
            .unwrap()
            .disp[cn];
        let mut num: f64 = 0.0;
        let mut den: f64 = 0.0;
        for q in 0..4 {
            num = num.max((x[q].re - x_rd[q].re).hypot(x[q].im - x_rd[q].im));
            den = den.max(x_rd[q].abs());
        }
        c.at_most(&format!("K2 {f} w_c1 response against rotordyn"), num / den, 1e-6);
    }
    let r0 = rotor_k(false);
    let rd0 = critical_speeds(&r0).unwrap();
    let spin = rd0[0];
    let mut lam = natural_frequencies(&r0, spin).unwrap();
    lam.sort_by(|a, b| a.re.total_cmp(&b.re));
    let f0 = flexible_from_rigid(&r0, &stiff_shaft(e)).unwrap();
    let w = f0.whirl(spin).unwrap();
    let mut s: Vec<(f64, Whirl)> = w
        .modes
        .iter()
        .map(|m| (m.omega * m.whirl.signum(), m.direction))
        .collect();
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    c.holds(
        "K3 four modes",
        s.len() == 4 && lam.len() == 4,
        &format!("{} modes, {} roots", s.len(), lam.len()),
    );
    if s.len() == 4 && lam.len() == 4 {
        let mut d: f64 = 0.0;
        let mut dirs = true;
        for ((v, dir), l) in s.iter().zip(&lam) {
            d = d.max((v.abs() - l.re.abs()).abs() / l.re.abs());
            let want = if l.re > 0.0 { Whirl::Forward } else { Whirl::Backward };
            dirs &= *dir == want;
            println!("  K3 {v:.10e} against {:.10e}", l.re);
        }
        c.at_most("K3 natural frequencies against rotordyn", d, 1e-6);
        c.holds("K3 directions", dirs, "");
    }
    match k1_measure(2e11) {
        Some(d) => c.above("N2 steel shaft K1 measure", d, 1e-6),
        None => c.holds("N2 count", false, "the forward count differs"),
    }
    c.finish();
}

#[test]
fn s1_s2_s3_join() {
    let mut c = Checks::new();
    let r = rotor_k(true);
    let back = rigid_from_flexible(&flexible_from_rigid(&r, &stiff_shaft(5e14)).unwrap()).unwrap();
    let mut d: f64 = 0.0;
    d = d
        .max(rel(back.mass, r.mass))
        .max(rel(back.it, r.it))
        .max(rel(back.ip, r.ip));
    for (a, b) in back.bearings.iter().zip(&r.bearings) {
        d = d.max(rel(a.z, b.z)).max(rel(a.k, b.k)).max(rel(a.c, b.c));
    }
    c.at_most("S1 round trip", d, 1e-12);
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let solid = |o: [f64; 3], l: [f64; 3]| {
        let mut m = SolidModel::new(hex_box(o, l, [2, 2, 1]).unwrap());
        m.add_material("all", steel.clone()).unwrap();
        m
    };
    let sd = disc_from_solid(&solid([-0.05, -0.05, -0.01], [0.1, 0.1, 0.02])).unwrap();
    let m = 7850.0 * 2e-4;
    let it = m * (0.1f64.powi(2) + 0.02f64.powi(2)) / 12.0;
    let ip = m * (0.1f64.powi(2) + 0.1f64.powi(2)) / 12.0;
    c.at_most("S2 mass", rel(sd.disc.mass, m), 1e-12);
    c.at_most("S2 I_t", rel(sd.disc.transverse, it), 1e-12);
    c.at_most("S2 I_p", rel(sd.disc.polar, ip), 1e-12);
    let cmax = sd.centre.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    c.at_most("S2 centre", cmax, 1e-15);
    c.holds(
        "S3 non-square box",
        key_of(&disc_from_solid(&solid([-0.05, -0.04, -0.01], [0.1, 0.08, 0.02]))) == Some("inertia"),
        "",
    );
    c.holds(
        "S3 shifted box",
        key_of(&disc_from_solid(&solid([-0.04, -0.05, -0.01], [0.1, 0.1, 0.02]))) == Some("centre"),
        "",
    );
    let mut re = rotor_k(true);
    re.bearings[0].eta = 0.01;
    c.holds(
        "S3 eta",
        key_of(&flexible_from_rigid(&re, &stiff_shaft(5e14))) == Some("eta"),
        "",
    );
    let mut a = RotorModel::new(vec![0.0, 0.05, 0.1]).unwrap();
    let shaft = stiff_shaft(2e11);
    a.add_shaft(0, 1, shaft.section, &shaft.material).unwrap();
    a.add_shaft(1, 2, shaft.section, &shaft.material).unwrap();
    a.add_disc(
        1,
        Disc {
            mass: 1.0,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    for n in [0, 2] {
        a.add_bearing(n, FlexBearing::new([[1e5, 0.0], [0.0, 2.25e5]], [[0.0; 2]; 2]))
            .unwrap();
    }
    c.holds(
        "S3 anisotropic",
        key_of(&rigid_from_flexible(&a)) == Some("bearing"),
        "",
    );
    c.finish();
}
