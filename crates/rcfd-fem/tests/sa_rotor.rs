//! F-416 (ADR 0476): the flexible rotor on bearings in the stationary frame. Checks Z1, Z2, J1 to J3, X1 to X3, R1, R2, O1,
//! T1, A1, A2, U1, U2, L1, G1, X0 and the negative controls N1, N3, N4 of the registration (K, S, N2 are in rcfd-post's
//! `sa_solid_rotor.rs`; B1 in `sa_rotor_default_bits.rs`).

use rcfd_fem::beam;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::rotating::{Stability, Whirl};
use rcfd_fem::rotor::{Bearing, Disc, RotorModel, RotorWhirlMode, Unbalance, C64};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, Material, SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

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

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 0.9;

fn steel(rho: f64) -> Material {
    Material::isotropic_elastic(E, NU, rho, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn shear_modulus() -> f64 {
    E / (2.0 * (1.0 + NU))
}

/// A uniform shaft of `ne` elements on `[0, l]`.
fn uniform(l: f64, ne: usize, s: BeamSection, rho: f64) -> RotorModel {
    let z: Vec<f64> = (0..=ne).map(|i| l * i as f64 / ne as f64).collect();
    let mut m = RotorModel::new(z).unwrap();
    for i in 0..ne {
        m.add_shaft(i, i + 1, s, &steel(rho)).unwrap();
    }
    m
}

fn key_of<T: std::fmt::Debug>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

/// `sqrt(z)`, principal branch.
fn csqrt(z: C64) -> C64 {
    let r = z.abs().sqrt();
    let a = 0.5 * z.im.atan2(z.re);
    C64::new(r * a.cos(), r * a.sin())
}

// ------------------------------------------------------------------------------------------------- Z: zero speed

fn f391_shaft(springs: Option<f64>) -> SolidModel {
    let (l, r, ne) = (1.0, 0.05, 20usize);
    let nodes: Vec<[f64; 3]> = (0..=ne).map(|i| [0.0, 0.0, l * i as f64 / ne as f64]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..ne).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    let s = beam::circle_section(r, KAPPA).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [1.0, 0.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..=ne).collect()).unwrap();
    mesh.add_node_set("ends", vec![0, ne]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(RHO)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 5, 0.0)).unwrap();
    match springs {
        None => {
            m.add_constraint(Constraint::displacement("ends", 0, 0.0)).unwrap();
            m.add_constraint(Constraint::displacement("ends", 1, 0.0)).unwrap();
        }
        Some(k) => {
            m.add_support(ElasticSupport::nodal_spring("ends", 0, k)).unwrap();
            m.add_support(ElasticSupport::nodal_spring("ends", 1, k)).unwrap();
        }
    }
    m
}

fn rotor_z(springs: Option<f64>) -> RotorModel {
    let mut m = uniform(1.0, 20, beam::circle_section(0.05, KAPPA).unwrap(), RHO);
    match springs {
        None => {
            m.pin(0).unwrap();
            m.pin(20).unwrap();
        }
        Some(k) => {
            m.add_bearing(0, Bearing::isotropic(k, 0.0)).unwrap();
            m.add_bearing(20, Bearing::isotropic(k, 0.0)).unwrap();
        }
    }
    m
}

#[test]
fn z1_z2_zero_speed_against_f391() {
    let mut c = Checks::new();
    for (name, springs) in [("Z-a", None), ("Z-b", Some(1.0e7))] {
        let f = modal_analysis(
            &f391_shaft(springs),
            &ModalOptions::new(8)
                .tolerance(1e-12)
                .residual_test(ResidualTest::Backward),
        )
        .unwrap();
        let w = rotor_z(springs).whirl(0.0).unwrap();
        c.holds(
            &format!("Z1 {name} count"),
            w.modes.len() >= 8 && f.eigenvalues.len() == 8,
            &format!("{} rotor modes, {} F-391 modes", w.modes.len(), f.eigenvalues.len()),
        );
        let mut d: f64 = 0.0;
        for i in 0..8 {
            let wf = f.eigenvalues[i].sqrt();
            println!("  {name} mode {i}: rotor {:.12e} F-391 {wf:.12e}", w.modes[i].omega);
            d = d.max(rel(w.modes[i].omega, wf));
        }
        c.at_most(&format!("Z1 {name} max |w/w_F391 - 1|"), d, 1e-10);
        c.holds(
            &format!("Z2 {name} stable"),
            w.stability == Stability::Stable,
            &format!("{:?}", w.stability),
        );
        let res = w.modes.iter().map(|m| m.residual).fold(0.0, f64::max);
        c.at_most(&format!("Z2 {name} max residual"), res, 1e-12);
        println!("  {name}: {} unknowns after the condensation", w.unknowns);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- J and X: Jeffcott

const JL: f64 = 0.4;
const JR: f64 = 0.01;
const JM: f64 = 2.0;
const JKB: f64 = 1.0e6;
const JZETA: f64 = 0.05;
const JE: f64 = 1.0e-4;

/// `(k, w_n, c)` of set-up J.
fn jeffcott_constants() -> (f64, f64, f64) {
    let a = PI * JR * JR;
    let i = PI * JR.powi(4) / 4.0;
    let ks = 1.0 / (JL.powi(3) / (48.0 * E * i) + JL / (4.0 * KAPPA * shear_modulus() * a));
    let k = 1.0 / (1.0 / ks + 1.0 / (2.0 * JKB));
    let wn = (k / JM).sqrt();
    let c = 2.0 * JZETA * (k * JM).sqrt();
    (k, wn, c)
}

fn jeffcott(kc: f64, damper_at: usize) -> RotorModel {
    let (_, _, c) = jeffcott_constants();
    let mut m = uniform(JL, 4, beam::circle_section(JR, KAPPA).unwrap(), 0.0);
    m.add_disc(
        2,
        Disc {
            mass: JM,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    m.add_bearing(0, Bearing::isotropic(JKB, 0.0)).unwrap();
    m.add_bearing(4, Bearing::isotropic(JKB, 0.0)).unwrap();
    m.add_bearing(damper_at, Bearing::isotropic(0.0, c)).unwrap();
    if kc != 0.0 {
        m.add_bearing(2, Bearing::new([[0.0, kc], [-kc, 0.0]], [[0.0; 2]; 2]))
            .unwrap();
    }
    m
}

#[test]
fn j1_j2_j3_jeffcott() {
    let mut c = Checks::new();
    let (k, wn, _) = jeffcott_constants();
    println!("J: k {k:.6e} N/m, w_n {wn:.6e} rad/s");
    let m = jeffcott(0.0, 2);
    let cs = m.critical_speeds().unwrap();
    c.holds("J1 two critical speeds", cs.len() == 2, &format!("{}", cs.len()));
    for s in &cs {
        c.at_most(
            &format!("J1 {:?} critical speed against sqrt(k/m)", s.direction),
            rel(s.speed, wn),
            1e-3,
        );
    }
    let dirs: Vec<Whirl> = cs.iter().map(|s| s.direction).collect();
    c.holds(
        "J1 one forward, one backward",
        dirs.contains(&Whirl::Forward) && dirs.contains(&Whirl::Backward),
        &format!("{dirs:?} (whirl {:?})", cs.iter().map(|s| s.whirl).collect::<Vec<_>>()),
    );
    for r in [0.5, 1.0, 2.0] {
        let spin = r * wn;
        let resp = m
            .unbalance_response(
                spin,
                &[Unbalance {
                    node: 2,
                    amount: JM * JE,
                    angle: 0.0,
                }],
            )
            .unwrap();
        let o = resp.orbits[2];
        let cf = JE * r * r / ((1.0 - r * r).powi(2) + (2.0 * JZETA * r).powi(2)).sqrt();
        c.at_most(&format!("J2 r = {r} amplitude"), rel(o.major, cf), 1e-3);
        let sense = (o.forward.powi(2) - o.backward.powi(2)) / (o.forward.powi(2) + o.backward.powi(2));
        c.holds(
            &format!("J3 r = {r} forward circular"),
            o.direction == Whirl::Forward && sense >= 1.0 - 1e-9 && o.minor / o.major >= 1.0 - 1e-9,
            &format!(
                "{:?}, sense {sense:.12}, minor/major {:.12}",
                o.direction,
                o.minor / o.major
            ),
        );
    }
    c.finish();
}

#[test]
fn x1_x2_x3_n4_cross_coupling() {
    let mut c = Checks::new();
    let (k, wn, cd) = jeffcott_constants();
    let th = cd * wn;
    let spin = 0.5 * wn;
    let stab = |kc: f64| jeffcott(kc, 2).whirl(spin).unwrap();
    c.holds(
        "X1 stable at 0.98 c w_n",
        stab(0.98 * th).stability == Stability::Stable,
        &format!("{:?}", stab(0.98 * th).stability),
    );
    c.holds(
        "X1 unstable at 1.02 c w_n",
        !stab(1.02 * th).stability.is_stable(),
        &format!("{:?}", stab(1.02 * th).stability),
    );
    let (mut lo, mut hi) = (0.5 * th, 2.0 * th);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if stab(mid).stability.is_stable() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let kstar = 0.5 * (lo + hi);
    c.at_most("X2 threshold |k_c*/(c w_n) - 1|", rel(kstar, th), 1e-2);
    let kc = 1.5 * th;
    let w = stab(kc);
    // m s^2 + c s + k - i kc = 0
    let disc = C64::new(cd * cd - 4.0 * JM * k, 4.0 * JM * kc);
    let sq = csqrt(disc);
    let g_cf = ((-cd + sq.re) / (2.0 * JM)).max((-cd - sq.re) / (2.0 * JM));
    match w.stability {
        Stability::Unstable { growth } => {
            c.at_most("X3 |growth - g_cf| / w_n", (growth - g_cf).abs() / wn, 1e-8);
            println!("  X3 growth {growth:.12e}, closed form {g_cf:.12e}");
        }
        Stability::Stable => c.holds("X3 unstable at 1.5 c w_n", false, "stable"),
    }
    let grow: Vec<&RotorWhirlMode> = w.modes.iter().filter(|m| m.growth > 0.0).collect();
    c.holds(
        "X3 the growing mode is reported, forward",
        !grow.is_empty() && grow.iter().all(|m| m.direction == Whirl::Forward),
        &format!(
            "{:?}",
            grow.iter()
                .map(|m| (m.omega, m.growth, m.direction))
                .collect::<Vec<_>>()
        ),
    );
    let n4 = stab(0.0);
    let dr = n4
        .modes
        .iter()
        .map(|m| (m.damping_ratio - JZETA).abs())
        .fold(0.0, f64::max);
    c.holds(
        "N4 k_c = 0 stable with damping ratio zeta",
        n4.stability == Stability::Stable && n4.modes.len() == 2 && dr <= 1e-10,
        &format!(
            "{:?}, {} modes, max |zeta_i - zeta| {dr:.3e}",
            n4.stability,
            n4.modes.len()
        ),
    );
    println!("  X2 k_c* / (c w_n) = {:.12}", kstar / th);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R: Rayleigh shaft

const RR: f64 = 0.05;

fn rayleigh(n: usize, spin: f64, sign: f64) -> f64 {
    let a = PI * RR * RR;
    let i = PI * RR.powi(4) / 4.0;
    let k = n as f64 * PI;
    let aa = RHO * (a + i * k * k);
    let b = 2.0 * RHO * i * k * k * spin;
    let e = E * i * k.powi(4);
    (sign * b + (b * b + 4.0 * aa * e).sqrt()) / (2.0 * aa)
}

fn rotor_r() -> RotorModel {
    let mut m = uniform(1.0, 20, beam::circle_section(RR, 1e8).unwrap(), RHO);
    m.pin(0).unwrap();
    m.pin(20).unwrap();
    m
}

/// The mode number of a shape by the projection rule of set-up R.
fn mode_number(shape: &[[C64; 4]], z: &[f64]) -> usize {
    let mut best = (0, -1.0);
    for n in 1..=4 {
        let s: Vec<f64> = z.iter().map(|&x| (n as f64 * PI * x).sin()).collect();
        let (mut px, mut py) = (C64::ZERO, C64::ZERO);
        for (u, v) in shape.iter().zip(&s) {
            px += u[0].scale(*v);
            py += u[1].scale(*v);
        }
        let ss: f64 = s.iter().map(|v| v * v).sum();
        let p = (px.norm_sqr() + py.norm_sqr()) / ss;
        if p > best.1 {
            best = (n, p);
        }
    }
    best.0
}

#[test]
fn r1_r2_n1_rayleigh_shaft() {
    let mut c = Checks::new();
    let m = rotor_r();
    let w1 = rayleigh(1, 0.0, 1.0);
    println!("w_1R = {w1:.6e} rad/s");
    for f in [2.0, 10.0, 30.0] {
        let spin = f * w1;
        let w = m.whirl(spin).unwrap();
        let low = &w.modes[..6];
        for n in [1usize, 2] {
            for (dir, sign) in [(Whirl::Forward, 1.0), (Whirl::Backward, -1.0)] {
                let found: Vec<&RotorWhirlMode> = low
                    .iter()
                    .filter(|md| md.direction == dir && mode_number(&md.shape, m.stations()) == n)
                    .collect();
                if found.len() != 1 {
                    c.holds(
                        &format!("R1 {f} w_1R n = {n} {dir:?}: one mode"),
                        false,
                        &format!("{} found (defective set-up)", found.len()),
                    );
                    continue;
                }
                let cf = rayleigh(n, spin, sign);
                c.at_most(
                    &format!("R1 {f} w_1R n = {n} {dir:?} ({:.6e} against {cf:.6e})", found[0].omega),
                    rel(found[0].omega, cf),
                    5e-3,
                );
                let ok = if sign > 0.0 {
                    found[0].whirl >= 1.0 - 1e-6
                } else {
                    found[0].whirl <= -1.0 + 1e-6
                };
                c.holds(
                    &format!("R2 {f} w_1R n = {n} {dir:?} whirl number"),
                    ok,
                    &format!("{:.12}", found[0].whirl),
                );
            }
        }
    }
    let rest = m.whirl(0.0).unwrap();
    c.above(
        "N1 rest n = 1 against omega_F at 30 w_1R",
        rel(rest.modes[0].omega, rayleigh(1, 30.0 * w1, 1.0)),
        0.1,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O: overhung disc

const OL: f64 = 0.1;
const OR: f64 = 0.01;
const OM: f64 = 1.0;
const OIT: f64 = 2.5e-3;
const OIP: f64 = 5e-3;

fn rotor_o() -> RotorModel {
    let mut m = uniform(OL, 4, beam::circle_section(OR, KAPPA).unwrap(), 0.0);
    m.clamp(0).unwrap();
    m.add_disc(
        4,
        Disc {
            mass: OM,
            transverse: OIT,
            polar: OIP,
        },
    )
    .unwrap();
    m
}

/// `(k11, k12, k22)` of the cantilever tip.
fn overhung_stiffness() -> (f64, f64, f64) {
    let a = PI * OR * OR;
    let ei = E * PI * OR.powi(4) / 4.0;
    let a11 = OL.powi(3) / (3.0 * ei) + OL / (KAPPA * shear_modulus() * a);
    let a12 = OL * OL / (2.0 * ei);
    let a22 = OL / ei;
    let det = a11 * a22 - a12 * a12;
    (a22 / det, -a12 / det, a11 / det)
}

/// The four real roots of the quartic of set-up O at the spin, ascending (amendment 1: Fujiwara's bound, a 1e6-step scan,
/// bisection to 1e-14 relative).
fn quartic_roots(spin: f64) -> Vec<f64> {
    let (k11, k12, k22) = overhung_stiffness();
    let a = [
        k11 * k22 - k12 * k12,
        k11 * OIP * spin,
        -(OM * k22 + OIT * k11),
        -OM * OIP * spin,
        OM * OIT,
    ];
    let p = |x: f64| (((a[4] * x + a[3]) * x + a[2]) * x + a[1]) * x + a[0];
    let lam_max = 2.0
        * (a[3] / a[4])
            .abs()
            .max((a[2] / a[4]).abs().sqrt())
            .max((a[1] / a[4]).abs().cbrt())
            .max((a[0] / (2.0 * a[4])).abs().powf(0.25));
    let steps = 1_000_000;
    let (x0, h) = (-2.0 * lam_max, 4.0 * lam_max / steps as f64);
    let mut roots = Vec::new();
    let mut prev = p(x0);
    for s in 1..=steps {
        let x = x0 + h * s as f64;
        let v = p(x);
        if prev == 0.0 {
            roots.push(x - h);
        } else if prev * v < 0.0 {
            let (mut lo, mut hi) = (x - h, x);
            let mut flo = prev;
            while (hi - lo) > 1e-14 * hi.abs().max(lo.abs()) {
                let mid = 0.5 * (lo + hi);
                let fm = p(mid);
                if fm == 0.0 {
                    lo = mid;
                    hi = mid;
                    break;
                }
                if flo * fm < 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                    flo = fm;
                }
            }
            roots.push(0.5 * (lo + hi));
        }
        prev = v;
    }
    roots
}

fn w_ref() -> f64 {
    quartic_roots(0.0)
        .into_iter()
        .filter(|x| *x > 0.0)
        .fold(f64::INFINITY, f64::min)
}

fn signed(w: &rcfd_fem::rotor::RotorWhirl) -> Vec<f64> {
    let mut v: Vec<f64> = w.modes.iter().map(|m| m.omega * m.whirl.signum()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

#[test]
fn o1_overhung_disc() {
    let mut c = Checks::new();
    let wr = w_ref();
    println!("w_ref = {wr:.6e} rad/s, rest roots {:?}", quartic_roots(0.0));
    let m = rotor_o();
    for f in [0.5, 1.0, 2.0] {
        let spin = f * wr;
        let q = quartic_roots(spin);
        let w = m.whirl(spin).unwrap();
        let s = signed(&w);
        c.holds(
            &format!("O1 {f} w_ref: four modes, four roots"),
            s.len() == 4 && q.len() == 4,
            &format!("{} modes, {} roots", s.len(), q.len()),
        );
        if s.len() == 4 && q.len() == 4 {
            for i in 0..4 {
                c.at_most(
                    &format!("O1 {f} w_ref root {i} ({:.6e} against {:.6e})", s[i], q[i]),
                    rel(s[i], q[i]),
                    5e-3,
                );
            }
        }
    }
    c.finish();
}

#[test]
fn t1_overhung_campbell() {
    let mut c = Checks::new();
    let wr = w_ref();
    let speeds: Vec<f64> = (0..=10).map(|j| 0.2 * j as f64 * wr).collect();
    let t = rotor_o().campbell(&speeds).unwrap();
    let nt = t.campbell.table.tracks.len();
    c.holds("T1 four tracks", nt == 4, &format!("{nt}"));
    let mut mac_min: f64 = 1.0;
    for tr in &t.campbell.table.tracks {
        for v in tr.mac.iter().skip(1).flatten() {
            mac_min = mac_min.min(*v);
        }
    }
    c.at_most("T1 1 - smallest MAC", 1.0 - mac_min, 0.1);
    for (ti, d) in t.campbell.directions.iter().enumerate() {
        let ds: Vec<Whirl> = d.iter().skip(1).flatten().copied().collect();
        c.holds(
            &format!("T1 track {ti} one direction"),
            ds.len() == speeds.len() - 1 && ds.iter().all(|x| *x == ds[0]),
            &format!("{ds:?}"),
        );
    }
    let mut dmax: f64 = 0.0;
    let mut ok = true;
    for (j, &spin) in speeds.iter().enumerate().skip(1) {
        let q = quartic_roots(spin);
        let mut fw: Vec<f64> = Vec::new();
        let mut bw: Vec<f64> = Vec::new();
        for ti in 0..nt {
            let w = t.campbell.omegas(ti)[j];
            match (w, t.campbell.directions[ti][j]) {
                (Some(w), Some(Whirl::Forward)) => fw.push(w),
                (Some(w), Some(Whirl::Backward)) => bw.push(w),
                _ => ok = false,
            }
        }
        fw.sort_by(|a, b| a.total_cmp(b));
        bw.sort_by(|a, b| a.total_cmp(b));
        let mut qp: Vec<f64> = q.iter().filter(|x| **x > 0.0).copied().collect();
        let mut qn: Vec<f64> = q.iter().filter(|x| **x < 0.0).map(|x| -x).collect();
        qp.sort_by(|a, b| a.total_cmp(b));
        qn.sort_by(|a, b| a.total_cmp(b));
        if fw.len() != qp.len() || bw.len() != qn.len() {
            ok = false;
            continue;
        }
        for (a, b) in fw.iter().zip(&qp).chain(bw.iter().zip(&qn)) {
            dmax = dmax.max(rel(*a, *b));
        }
    }
    c.holds("T1 tracks have a mode and a direction at every speed > 0", ok, "");
    c.at_most("T1 tracks against the quartic", dmax, 5e-3);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- A: anisotropic bearings

fn rotor_a(kx: f64, ky: f64) -> RotorModel {
    let mut m = uniform(0.1, 2, beam::circle_section(0.05, KAPPA).unwrap(), 0.0);
    m.add_disc(
        1,
        Disc {
            mass: 1.0,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    for n in [0, 2] {
        m.add_bearing(n, Bearing::new([[kx / 2.0, 0.0], [0.0, ky / 2.0]], [[0.0; 2]; 2]))
            .unwrap();
    }
    m
}

#[test]
fn a1_a2_n3_anisotropic_bearings() {
    let mut c = Checks::new();
    let (kx, ky) = (2.0e5, 4.5e5);
    let cs = rotor_a(kx, ky).critical_speeds().unwrap();
    c.holds("A1 two critical speeds", cs.len() == 2, &format!("{}", cs.len()));
    if cs.len() == 2 {
        c.at_most("A1 against sqrt(kx/m)", rel(cs[0].speed, kx.sqrt()), 1e-3);
        c.at_most("A1 against sqrt(ky/m)", rel(cs[1].speed, ky.sqrt()), 1e-3);
        for s in &cs {
            c.holds(
                &format!("A2 {:.4e} rad/s neutral", s.speed),
                s.direction == Whirl::Neutral && s.whirl.abs() <= 1e-6,
                &format!("{:?}, whirl {:.3e}", s.direction, s.whirl),
            );
        }
    }
    let c0 = rotor_a(kx, kx).critical_speeds().unwrap();
    let dirs: Vec<Whirl> = c0.iter().map(|s| s.direction).collect();
    c.holds(
        "N3 isotropic: two equal, forward and backward",
        c0.len() == 2
            && rel(c0[0].speed, c0[1].speed) <= 1e-12
            && dirs.contains(&Whirl::Forward)
            && dirs.contains(&Whirl::Backward),
        &format!("{:?}", c0.iter().map(|s| (s.speed, s.direction)).collect::<Vec<_>>()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- U: unstable band

fn rotor_u() -> RotorModel {
    let wr = w_ref();
    let cd = 2.0 * 0.02 * OM * wr;
    let kc = 1.5 * cd * wr;
    let mut m = rotor_o();
    m.add_bearing(4, Bearing::new([[0.0, kc], [-kc, 0.0]], [[cd, 0.0], [0.0, cd]]))
        .unwrap();
    m
}

fn u_speeds() -> Vec<f64> {
    let wr = w_ref();
    (0..=8).map(|j| 0.25 * j as f64 * wr).collect()
}

#[test]
fn u1_u2_unstable_band_reported() {
    let mut c = Checks::new();
    let m = rotor_u();
    let speeds = u_speeds();
    let t = m.campbell(&speeds).unwrap();
    let un = t.unstable();
    println!("U unstable: {un:?}");
    c.holds(
        "U1 unstable() is exactly j = 0 .. 4",
        un.len() == 5 && un.iter().zip(&speeds).all(|(u, s)| u.0.to_bits() == s.to_bits()),
        &format!("{:?}", un.iter().map(|u| u.0).collect::<Vec<_>>()),
    );
    for (j, &s) in speeds.iter().enumerate() {
        let w = m.whirl(s).unwrap();
        let same = match (t.speeds[j].stability, w.stability) {
            (Some(Stability::Unstable { growth: a }), Stability::Unstable { growth: b }) => a.to_bits() == b.to_bits(),
            (Some(Stability::Stable), Stability::Stable) => true,
            _ => false,
        };
        let want_unstable = j <= 4;
        c.holds(
            &format!("U1 j = {j} stability"),
            same && w.stability.is_stable() != want_unstable,
            &format!("{:?}", w.stability),
        );
        if want_unstable {
            let g: Vec<&RotorWhirlMode> = w.modes.iter().filter(|md| md.growth > 0.0).collect();
            c.holds(
                &format!("U2 j = {j} growing mode reported, forward"),
                !g.is_empty() && g.iter().all(|md| md.direction == Whirl::Forward),
                &format!("{:?}", g.iter().map(|md| (md.omega, md.growth)).collect::<Vec<_>>()),
            );
        }
    }
    let csv = t.to_csv();
    let rows: Vec<&str> = csv.lines().skip(1).collect();
    let ok = rows.len() == 9
        && rows
            .iter()
            .enumerate()
            .all(|(j, r)| r.contains(",unstable,") == (j <= 4) && r.contains(",stable,0") == (j > 4));
    c.holds("U1 CSV rows", ok, &format!("{} rows", rows.len()));
    c.finish();
}

// ------------------------------------------------------------------------------------------------- L1: sense of the orbit

fn orbit_sense(md: &RotorWhirlMode) -> Whirl {
    let mut best = (0usize, -1.0);
    for (i, u) in md.shape.iter().enumerate() {
        let a = u[0].norm_sqr() + u[1].norm_sqr();
        if a > best.1 {
            best = (i, a);
        }
    }
    let u = md.shape[best.0];
    let s = (u[0] * u[1].conj()).im;
    if s > 0.0 {
        Whirl::Forward
    } else if s < 0.0 {
        Whirl::Backward
    } else {
        Whirl::Neutral
    }
}

#[test]
#[ignore = "known miss of ADR 0476 (amendment 2): L1 on R, 3, 2 and 3 of 80 modes at 2, 10, 30 w_1R (translation-free modes at 2.7e5 to 7.4e5 rad/s, translation orbit at rounding level); J, O, U met; continued in l1b_direction_is_the_sense_of_the_orbit_after_amendment_2"]
fn l1_direction_is_the_sense_of_the_orbit_known_miss() {
    let mut c = Checks::new();
    let (_, wn, _) = jeffcott_constants();
    let w1 = rayleigh(1, 0.0, 1.0);
    let wr = w_ref();
    let mut runs: Vec<(String, rcfd_fem::rotor::RotorWhirl)> =
        vec![("J w_n".into(), jeffcott(0.0, 2).whirl(wn).unwrap())];
    let r = rotor_r();
    for f in [2.0, 10.0, 30.0] {
        runs.push((format!("R {f} w_1R"), r.whirl(f * w1).unwrap()));
    }
    let o = rotor_o();
    for f in [0.5, 1.0, 2.0] {
        runs.push((format!("O {f} w_ref"), o.whirl(f * wr).unwrap()));
    }
    let u = rotor_u();
    for (j, s) in u_speeds().into_iter().enumerate() {
        runs.push((format!("U j = {j}"), u.whirl(s).unwrap()));
    }
    for (name, w) in &runs {
        let bad: Vec<(f64, Whirl, Whirl)> = w
            .modes
            .iter()
            .filter(|md| md.direction != orbit_sense(md))
            .map(|md| (md.omega, md.direction, orbit_sense(md)))
            .collect();
        c.holds(
            &format!("L1 {name}: {} modes", w.modes.len()),
            bad.is_empty(),
            &format!("mismatches {bad:?}"),
        );
    }
    c.finish();
}

/// The reference of amendment 2: the translation orbit at the station of largest translation when the translational kinetic
/// energy is at least 1e-6 of `x^H M x`, otherwise the tilt orbit `(theta_y, -theta_x)` at the station of largest tilt.
fn orbit_sense_amended(md: &RotorWhirlMode, m: &[f64], n: usize) -> Whirl {
    let x: Vec<C64> = md.shape.iter().flatten().copied().collect();
    let (mut total, mut trans) = (0.0, 0.0);
    for i in 0..n {
        for j in 0..n {
            let v = (x[i].conj() * x[j]).re * m[i * n + j];
            total += v;
            if i % 4 < 2 && j % 4 < 2 {
                trans += v;
            }
        }
    }
    let sign = |s: f64| {
        if s > 0.0 {
            Whirl::Forward
        } else if s < 0.0 {
            Whirl::Backward
        } else {
            Whirl::Neutral
        }
    };
    if trans >= 1e-6 * total {
        return orbit_sense(md);
    }
    let mut best = (0usize, -1.0);
    for (i, u) in md.shape.iter().enumerate() {
        let a = u[2].norm_sqr() + u[3].norm_sqr();
        if a > best.1 {
            best = (i, a);
        }
    }
    let u = md.shape[best.0];
    sign((u[3] * (-u[2]).conj()).im)
}

#[test]
fn l1b_direction_is_the_sense_of_the_orbit_after_amendment_2() {
    let mut c = Checks::new();
    let (_, wn, _) = jeffcott_constants();
    let w1 = rayleigh(1, 0.0, 1.0);
    let wr = w_ref();
    let j = jeffcott(0.0, 2);
    let r = rotor_r();
    let o = rotor_o();
    let u = rotor_u();
    let mut runs: Vec<(String, &RotorModel, rcfd_fem::rotor::RotorWhirl)> =
        vec![("J w_n".into(), &j, j.whirl(wn).unwrap())];
    for f in [2.0, 10.0, 30.0] {
        runs.push((format!("R {f} w_1R"), &r, r.whirl(f * w1).unwrap()));
    }
    for f in [0.5, 1.0, 2.0] {
        runs.push((format!("O {f} w_ref"), &o, o.whirl(f * wr).unwrap()));
    }
    for (k, s) in u_speeds().into_iter().enumerate() {
        runs.push((format!("U j = {k}"), &u, u.whirl(s).unwrap()));
    }
    for (name, model, w) in &runs {
        let mats = model.matrices();
        let mut tilt = 0;
        let bad: Vec<(f64, Whirl, Whirl)> = w
            .modes
            .iter()
            .filter_map(|md| {
                let s = orbit_sense_amended(md, &mats.m, mats.n);
                if s != orbit_sense(md) {
                    tilt += 1;
                }
                (md.direction != s).then_some((md.omega, md.direction, s))
            })
            .collect();
        c.holds(
            &format!("L1b {name}: {} modes ({tilt} judged by the tilt orbit)", w.modes.len()),
            bad.is_empty(),
            &format!("mismatches {bad:?}"),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- G1, X0

#[test]
fn g1_gyroscopic_matrix_skew() {
    let mut c = Checks::new();
    let mut m = rotor_z(None);
    m.add_disc(
        10,
        Disc {
            mass: 1.0,
            transverse: 0.005,
            polar: 0.01,
        },
    )
    .unwrap();
    let mats = m.matrices();
    let n = mats.n;
    let mut d: f64 = 0.0;
    let mut big: f64 = 0.0;
    for i in 0..n {
        for j in 0..n {
            d = d.max((mats.g[i * n + j] + mats.g[j * n + i]).abs());
            big = big.max(mats.g[i * n + j].abs());
        }
    }
    c.holds(
        "G1 G skew bit for bit",
        d == 0.0 && big > 0.0,
        &format!("max |G + G^T| {d:e}, max |G| {big:e}"),
    );
    c.finish();
}

#[test]
fn x0_refusals() {
    let mut c = Checks::new();
    let mut check = |name: &str, key: Option<&'static str>, want: &str| {
        c.holds(&format!("X0 {name}"), key == Some(want), &format!("{key:?}"));
    };
    let s_bad = BeamSection::new(1e-3, 1e-7, 2e-7, 1e-7, 0.9, 0.9).unwrap();
    let mut m = RotorModel::new(vec![0.0, 0.1]).unwrap();
    check("section", key_of(&m.add_shaft(0, 1, s_bad, &steel(RHO))), "section");
    let (_, wn, _) = jeffcott_constants();
    check(
        "cross_coupling",
        key_of(&jeffcott(wn, 2).critical_speeds()),
        "cross_coupling",
    );
    let free = uniform(1.0, 4, beam::circle_section(0.05, KAPPA).unwrap(), RHO);
    check("free rotor", key_of(&free.critical_speeds()), "stiffness");
    check("damped massless unknown", key_of(&jeffcott(0.0, 0).whirl(1.0)), "mass");
    let j = jeffcott(0.0, 2);
    check("whirl(-1)", key_of(&j.whirl(-1.0)), "speed");
    check("whirl(NaN)", key_of(&j.whirl(f64::NAN)), "speed");
    check("campbell decreasing", key_of(&j.campbell(&[1.0, 0.5])), "speeds");
    let mut five = RotorModel::new(vec![0.0, 0.1, 0.2, 0.3, 0.4]).unwrap();
    check(
        "station 99",
        key_of(&five.add_disc(
            99,
            Disc {
                mass: 1.0,
                transverse: 0.0,
                polar: 0.0,
            },
        )),
        "node",
    );
    check(
        "stations not increasing",
        key_of(&RotorModel::new(vec![0.0, 0.2, 0.1])),
        "nodes",
    );
    check(
        "negative disc mass",
        key_of(&five.add_disc(
            1,
            Disc {
                mass: -1.0,
                transverse: 0.0,
                polar: 0.0,
            },
        )),
        "disc",
    );
    check(
        "NaN bearing",
        key_of(&five.add_bearing(1, Bearing::isotropic(f64::NAN, 0.0))),
        "bearing",
    );
    check("omega = 0", key_of(&j.harmonic_response(1.0, 0.0, &[])), "frequency");
    c.finish();
}
