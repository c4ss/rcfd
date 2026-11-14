//! F-609 (ADR 0529): the roots flagged pseudo (ADR 0467) are left at the reference frequency by the own-frequency iteration
//! of the damped modes when `OwnFrequency::skip_pseudo(true)` is asked. Checks PA, PF, PB, PD, PS, PE, PG, PH, PC, PN, PR,
//! PT, NC1, NC2 and RF of the registration; N1 is in `sa_damped_visco_own_pseudo_default_bits.rs`.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions, OwnFrequency};
use rcfd_fem::harmonic_direct::Dashpot;
use rcfd_fem::material::PronyTerm;
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
use std::f64::consts::PI;

const L: f64 = 1.0;
const B: f64 = 0.05;
const RHO: f64 = 1200.0;
const E0: f64 = 2.0e9;

fn g0(nu: f64) -> f64 {
    E0 / (2.0 * (1.0 + nu))
}

fn k0(nu: f64) -> f64 {
    E0 / (3.0 * (1.0 - 2.0 * nu))
}

/// The rod of ADR 0520 (axial, or in shear when `shear`) with one term of weights `g`, `k`; the model and `omega_c`.
fn rod(shear: bool, nu: f64, g: f64, k: f64) -> (SolidModel, f64) {
    let stored = if shear {
        g0(nu) * (1.0 - g / 2.0)
    } else {
        k0(nu) * (1.0 - k / 2.0) + 4.0 / 3.0 * g0(nu) * (1.0 - g / 2.0)
    };
    let wc = PI / (2.0 * L) * (stored / RHO).sqrt();
    let base = Material::isotropic_elastic(E0, nu, RHO, 0.0).unwrap();
    let visco = Material::viscoelastic(base, vec![PronyTerm::new(g, k, 1.0 / wc).unwrap()], None).unwrap();
    let mut m = SolidModel::new(hex_box([0.0; 3], [L, B, B], [40, 1, 1]).unwrap());
    m.add_material("all", visco).unwrap();
    let (free, held) = if shear { (1, [0, 2]) } else { (0, [1, 2]) };
    for c in held {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("xmin", free, 0.0)).unwrap();
    (m, wc)
}

struct Checks(Vec<String>);

impl Checks {
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn worst(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0, |a, &v| if v.is_nan() { f64::INFINITY } else { a.max(v) })
}

/// Every bit of the modes and the own-frequency records.
fn bits(d: &DampedModes) -> Vec<u64> {
    let mut v = Vec::new();
    for m in &d.modes {
        v.push(m.eigenvalue.re.to_bits());
        v.push(m.eigenvalue.im.to_bits());
        v.push(u64::from(m.pseudo));
        for z in &m.shape {
            v.push(z.re.to_bits());
            v.push(z.im.to_bits());
        }
    }
    for r in &d.own_frequency {
        v.push(r.frozen.re.to_bits());
        v.push(r.frozen.im.to_bits());
        v.push(r.omega.to_bits());
        v.push(r.iterations as u64);
        v.push(r.change.to_bits());
        v.push(u64::from(r.skipped));
    }
    v
}

fn rv_options(wc: f64, tip: usize, own: Option<OwnFrequency>) -> DampedOptions {
    let o = DampedOptions::new(4)
        .viscoelastic_reference(wc, None)
        .dashpot(Dashpot::ground(tip, 0, 1.0e3))
        .residual_vectors(true);
    match own {
        Some(own) => o.viscoelastic_own_frequency(own),
        None => o,
    }
}

fn rel_diff(a: C64, b: C64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn f609_checks() {
    let mut c = Checks(Vec::new());
    let (model, wc) = rod(false, 0.3, 0.15, 0.05);
    let tip = model.mesh().node_set("xmax").unwrap()[0];

    // The frozen solve (no own frequency): the flags and the reference modes.
    let frozen = damped_modes(&model, &rv_options(wc, tip, None)).unwrap();
    let flags: Vec<bool> = frozen.modes.iter().take(3).map(|m| m.pseudo).collect();
    c.holds(
        "PF root 0 pseudo, roots 1 and 2 not",
        flags == [true, false, false],
        &format!("{flags:?}"),
    );

    // PA: the restricted run at the default 50 iterations.
    let restricted = damped_modes(
        &model,
        &rv_options(wc, tip, Some(OwnFrequency::new(3).skip_pseudo(true))),
    );
    c.holds(
        "PA restricted run at the default 50 iterations",
        restricted.is_ok(),
        &format!(
            "{:?}",
            restricted.as_ref().map(|d| d.modes.len()).map_err(|e| e.to_string())
        ),
    );
    let restricted = restricted.unwrap();
    c.holds(
        "PA 3 modes, every root finite",
        restricted.modes.len() == 3
            && restricted
                .modes
                .iter()
                .all(|m| m.eigenvalue.re.is_finite() && m.eigenvalue.im.is_finite()),
        "",
    );

    // PB: the skipped record.
    let mut pb_ok = !restricted.own_frequency.is_empty();
    let mut n_skipped = 0;
    for (m, r) in restricted.modes.iter().zip(&restricted.own_frequency) {
        if r.skipped {
            n_skipped += 1;
            let f = frozen.modes.iter().find(|f| f.eigenvalue == r.frozen);
            pb_ok &= m.pseudo
                && r.iterations == 0
                && r.omega.to_bits() == wc.to_bits()
                && f.is_some_and(|f| {
                    f.eigenvalue == m.eigenvalue
                        && f.shape.len() == m.shape.len()
                        && f.shape.iter().zip(&m.shape).all(|(a, b)| a == b)
                });
        }
    }
    pb_ok &= n_skipped == 1;
    c.holds(
        "PB one skipped record, frozen bits, omega_ref, 0 iterations",
        pb_ok,
        &format!("{n_skipped}"),
    );

    // PD, PS: the physical roots.
    let phys: Vec<_> = restricted.own_frequency.iter().filter(|r| !r.skipped).collect();
    c.holds(
        "PD physical roots iterated within 50",
        phys.len() == 2 && phys.iter().all(|r| r.iterations >= 1 && r.iterations <= 50),
        &format!("{:?}", phys.iter().map(|r| r.iterations).collect::<Vec<_>>()),
    );
    c.at_most(
        "PS iterations of the physical roots",
        worst(&phys.iter().map(|r| r.iterations as f64).collect::<Vec<_>>()),
        15.0,
    );

    // PE, PH, PG, NC1, NC2: the unrestricted run.
    let free = damped_modes(
        &model,
        &rv_options(wc, tip, Some(OwnFrequency::new(3).max_iterations(1000))),
    )
    .unwrap();
    let mut diffs = Vec::new();
    let mut counts_equal = true;
    for (m, r) in restricted.modes.iter().zip(&restricted.own_frequency) {
        if r.skipped {
            continue;
        }
        match free
            .modes
            .iter()
            .zip(&free.own_frequency)
            .find(|(_, q)| q.frozen == r.frozen)
        {
            Some((fm, q)) => {
                diffs.push(rel_diff(m.eigenvalue, fm.eigenvalue));
                counts_equal &= q.iterations == r.iterations;
            }
            None => diffs.push(f64::INFINITY),
        }
    }
    if diffs.is_empty() {
        diffs.push(f64::INFINITY);
    }
    c.at_most("PE physical roots against the unrestricted run", worst(&diffs), 1e-12);
    c.holds("PE iteration counts equal", counts_equal, "");
    let pseudo_free = free
        .modes
        .iter()
        .zip(&free.own_frequency)
        .find(|(_, q)| frozen.modes.iter().any(|f| f.pseudo && f.eigenvalue == q.frozen));
    let (ph_it, ph_rel) = match pseudo_free {
        Some((fm, q)) => (q.iterations, rel_diff(fm.eigenvalue, q.frozen)),
        None => (0, f64::NAN),
    };
    c.holds(
        "PH unrestricted pseudo-mode iterations in [100, 300]",
        (100..=300).contains(&ph_it),
        &format!("{ph_it}"),
    );
    c.at_least(
        "PH / NC2 unrestricted pseudo-mode moves from the frozen root",
        ph_rel,
        1e-3,
    );
    let refused = damped_modes(&model, &rv_options(wc, tip, Some(OwnFrequency::new(3))));
    let key = matches!(
        &refused,
        Err(FemError::Solver { solver, .. }) if *solver == "viscoelastic own frequency"
    );
    c.holds(
        "PG / NC1 unrestricted at the default 50 is refused",
        key,
        &format!("{:?}", refused.as_ref().err().map(|e| e.to_string())),
    );

    // PC: the notes.
    let joined = restricted.notes.join("\n");
    c.holds(
        "PC restricted notes",
        joined.contains("pseudo") && joined.contains("left at the reference frequency") && joined.contains("ADR 0529"),
        &joined,
    );
    c.holds(
        "PC unrestricted notes without ADR 0529",
        !free.notes.join("\n").contains("ADR 0529"),
        "",
    );

    // PN: explicit false equals the default.
    c.holds("PN default is false", !OwnFrequency::new(3).skip_pseudo, "");
    let a = damped_modes(
        &model,
        &rv_options(wc, tip, Some(OwnFrequency::new(3).max_iterations(500))),
    )
    .unwrap();
    let b = damped_modes(
        &model,
        &rv_options(
            wc,
            tip,
            Some(OwnFrequency::new(3).max_iterations(500).skip_pseudo(false)),
        ),
    )
    .unwrap();
    c.holds(
        "PN explicit false equals the default",
        bits(&a) == bits(&b) && a.notes == b.notes,
        "",
    );

    // PR: rods without flagged roots.
    for (name, shear, nu, g, k) in [("A", false, 0.0, 2.0 / 21.0, 2.0 / 21.0), ("C", true, 0.3, 0.15, 0.05)] {
        let (m, w) = rod(shear, nu, g, k);
        let run = |skip: bool| {
            damped_modes(
                &m,
                &DampedOptions::new(6)
                    .viscoelastic_reference(w, None)
                    .viscoelastic_own_frequency(OwnFrequency::new(4).skip_pseudo(skip)),
            )
            .unwrap()
        };
        let (off, on) = (run(false), run(true));
        c.holds(
            &format!("PR rod {name} skip on equals off"),
            bits(&off) == bits(&on)
                && on.own_frequency.iter().all(|r| !r.skipped)
                && on.notes.join("\n").contains("no root flagged pseudo"),
            "",
        );
    }

    // PT: threads.
    let t = |n: usize| {
        damped_modes(
            &model,
            &rv_options(wc, tip, Some(OwnFrequency::new(3).skip_pseudo(true))).threads(n),
        )
        .unwrap()
    };
    c.holds("PT threads 1 and 4 bit equal", bits(&t(1)) == bits(&t(4)), "");

    // RF: refusals keep their key.
    let own_key = |own: OwnFrequency, reference: bool| {
        let o = DampedOptions::new(4);
        let o = if reference {
            o.viscoelastic_reference(wc, None)
        } else {
            o
        };
        matches!(
            damped_modes(&model, &o.viscoelastic_own_frequency(own)),
            Err(FemError::InvalidValue {
                key: "own frequency",
                ..
            })
        )
    };
    c.holds(
        "RF no reference frequency",
        own_key(OwnFrequency::new(3).skip_pseudo(true), false),
        "",
    );
    c.holds("RF roots 0", own_key(OwnFrequency::new(0).skip_pseudo(true), true), "");
    c.holds(
        "RF tolerance 0",
        own_key(OwnFrequency::new(3).tolerance(0.0).skip_pseudo(true), true),
        "",
    );
    c.holds(
        "RF max_iterations 0",
        own_key(OwnFrequency::new(3).max_iterations(0).skip_pseudo(true), true),
        "",
    );
    c.finish();
}
