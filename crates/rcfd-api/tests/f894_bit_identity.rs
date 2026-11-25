//! ADR 0792 (F-894), rows F1 to F4: what the repair of the one-sided normal must not move, against the record made on main
//! 1889caad before it (`f894_bit_identity.tsv`: group, number of values, FNV-1a hash of their bits or of the text's bytes).
//! F1: `erfc`, `erf`, `erfc_inv`, `norm_cdf` and `norm_inv` at finite arguments; F2: `Dist` with two finite or two open
//! bounds, uniform and beta, and the texts of its refusals; F3: both drivers; F4: the other callers of `erfc`.
//!
//! `F894_DUMP=<folder>` also writes the table and every value there. The hashes are of one mathematical library (`exp`), so
//! there is one record per platform, each made from the library of main (amendment 1): Windows x86_64 and Linux x86_64
//! (`f894_bit_identity.linux.tsv`). Another platform prints its table and compares the groups of IEEE operations alone.

use rcfd_api::design::bayes::{expected_improvement, prob_feasible};
use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition};
use rcfd_api::design::ledger::{Fidelity, FnModel, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::uq::{self, norm_cdf, norm_inv, Dist, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_post::fan_rom::{free_vortex_design, FanRom, Operating, RomFan, RomModel, RomSection, SystemCurve};
use rcfd_reactor::flamelet::chi_profile;
use rcfd_reactor::mixfrac::{erf, erfc, erfc_inv};
use std::fmt::Write;
use std::path::PathBuf;

const INF: f64 = f64::INFINITY;
const SEED: u64 = 792_003;
const WINDOWS: &str = include_str!("f894_bit_identity.tsv");
const LINUX: &str = include_str!("f894_bit_identity.linux.tsv");
/// The groups without a call of the mathematical library: the same on every platform.
const EVERYWHERE: &str = "F2 Uniform ";

/// The record of this platform.
fn record() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some(WINDOWS)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(LINUX)
    } else {
        None
    }
}

fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The groups of one row: a line of the table each, and every value for the dump.
#[derive(Default)]
struct Groups {
    table: Vec<String>,
    dump: String,
}

impl Groups {
    fn values(&mut self, name: &str, v: &[f64]) {
        let h = fnv(v.iter().flat_map(|x| x.to_bits().to_le_bytes()));
        self.table.push(format!("{name}\t{}\t{h:016x}", v.len()));
        for (i, x) in v.iter().enumerate() {
            writeln!(self.dump, "{name}\t{i}\t{:016x}\t{x:e}", x.to_bits()).unwrap();
        }
    }

    fn text(&mut self, name: &str, t: &str) {
        self.table
            .push(format!("{name}\t{}\t{:016x}", t.lines().count(), fnv(t.bytes())));
        for l in t.lines() {
            writeln!(self.dump, "{name}\t{l}").unwrap();
        }
    }

    /// Compare with the lines of the record that start with `row`, and write the dump when asked to.
    fn check(self, row: &str) {
        if let Ok(dir) = std::env::var("F894_DUMP") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{row}.table.tsv")), self.table.join("\n") + "\n").unwrap();
            std::fs::write(dir.join(format!("{row}.values.txt")), &self.dump).unwrap();
        }
        let of = |record: &'static str| {
            record
                .lines()
                .filter(|l| l.starts_with(row) && l[row.len()..].starts_with(' '))
                .collect::<Vec<&str>>()
        };
        // the names and the numbers of values, on every platform
        let head = |l: &str| l.rsplit_once('\t').unwrap().0.to_string();
        let heads: Vec<String> = self.table.iter().map(|l| head(l)).collect();
        let want: Vec<String> = of(WINDOWS).iter().map(|l| head(l)).collect();
        assert_eq!(heads, want, "{row}: groups of the record");
        let asserted = record().map_or_else(
            || {
                eprintln!("{row}: no record for this platform: {:#?}", self.table);
                let all = of(WINDOWS);
                all.into_iter().filter(|l| l.starts_with(EVERYWHERE)).collect()
            },
            of,
        );
        if record().is_some() {
            assert_eq!(asserted.len(), self.table.len(), "{row}: groups of the record");
        }
        let differ: Vec<&str> = asserted
            .iter()
            .filter(|l| !self.table.iter().any(|t| t == *l))
            .copied()
            .collect();
        eprintln!(
            "{row}: {} groups, {} compared, {} differ from main",
            self.table.len(),
            asserted.len(),
            differ.len()
        );
        assert!(
            differ.is_empty(),
            "{row}: not the record of main: {differ:#?} in {:#?}",
            self.table
        );
    }
}

/// F1: the special functions at finite arguments.
#[test]
fn f1_special_functions_at_finite_arguments() {
    let mut g = Groups::default();
    let mut x: Vec<f64> = (-1920..=1920).map(|k| k as f64 / 64.0).collect();
    let two = 2.0f64.to_bits();
    for v in [
        0.0,
        5e-324,
        f64::MIN_POSITIVE,
        1e-300,
        1e-8,
        f64::from_bits(two - 1),
        f64::from_bits(two + 1),
        1e10,
        1e100,
        1e200,
        f64::MAX,
    ] {
        x.extend([v, -v]);
    }
    g.values("F1 erfc", &x.iter().map(|&x| erfc(x)).collect::<Vec<_>>());
    g.values("F1 erf", &x.iter().map(|&x| erf(x)).collect::<Vec<_>>());
    let mut y: Vec<f64> = (1..2048).map(|j| j as f64 / 1024.0).collect();
    y.extend((0..=1021).map(|k| 2f64.powi(-k)));
    y.extend((0..=52).map(|k| 2.0 - 2f64.powi(-k)));
    g.values("F1 erfc_inv", &y.iter().map(|&y| erfc_inv(y)).collect::<Vec<_>>());
    g.values("F1 norm_inv", &y.iter().map(|&y| norm_inv(0.5 * y)).collect::<Vec<_>>());
    let z: Vec<f64> = (-1280..=1280).map(|k| norm_cdf(k as f64 / 32.0)).collect();
    g.values("F1 norm_cdf", &z);
    g.check("F1");
}

fn normal(mean: f64, sd: f64, lo: f64, hi: f64) -> Dist {
    Dist::Normal { mean, sd, lo, hi }
}

/// F2: the distributions the repair must not move, and the refusals.
#[test]
fn f2_distributions_and_refusals() {
    let mut g = Groups::default();
    let mut u = vec![
        0.0,
        2f64.powi(-54),
        1e-9,
        1e-3,
        0.1,
        0.25,
        0.5,
        0.75,
        0.9,
        0.999,
        1.0 - 1e-9,
        1.0 - 2f64.powi(-53),
        1.0,
    ];
    let mut rng = Rng::new(SEED);
    u.extend((0..400).map(|_| rng.uniform()));
    let beta = |a: f64, b: f64, lo: f64, hi: f64| Dist::Beta { a, b, lo, hi };
    for d in [
        normal(0.0, 1.0, -1.0, 2.0),
        normal(0.0, 1.0, 5.0, 8.0),
        normal(0.0, 1.0, -8.0, -5.0),
        normal(0.0, 1.0, 6.9, 7.5),
        normal(0.0, 1.0, -1e300, 1e300),
        normal(0.2, 0.4, -0.6, 0.9),
        normal(0.1, 0.3, -0.2, 8.0),
        normal(0.1, 0.3, -8.0, 0.25),
        normal(1013.25, 0.037, 1013.2, 1013.3),
        normal(0.0, 1.0, -INF, INF),
        normal(0.1, 0.3, -INF, INF),
        normal(1013.25, 0.037, -INF, INF),
        Dist::Uniform { lo: 2.0, hi: 5.0 },
        Dist::Uniform {
            lo: -1.5e-4,
            hi: 2.5e-4,
        },
        beta(2.0, 5.0, 0.0, 1.0),
        beta(2.0, 3.5, -0.012, 0.018),
        beta(0.5, 0.7, -1.0, 4.0),
    ] {
        let name = format!("F2 {d:?}");
        assert_eq!(d.validate(), Ok(()), "{name}");
        g.values(&name, &u.iter().map(|&u| d.inv_cdf(u)).collect::<Vec<_>>());
    }
    let refused = [
        normal(0.0, 1.0, 7.1, 9.0),
        normal(0.0, 1.0, -9.0, -7.1),
        normal(0.0, 1.0, 50.0, 51.0),
        normal(0.0, 1.0, f64::NAN, 1.0),
        normal(0.0, 1.0, -1.0, f64::NAN),
        normal(0.0, 1.0, 1.0, 1.0),
        normal(0.0, 1.0, 2.0, 1.0),
        normal(0.0, 0.0, -1.0, 1.0),
        normal(INF, 1.0, -1.0, 1.0),
        Dist::Uniform { lo: -INF, hi: 1.0 },
        beta(2.0, 3.0, 0.0, INF),
    ];
    let texts: Vec<String> = refused.iter().map(|d| d.validate().unwrap_err()).collect();
    g.text("F2 refusals", &texts.join("\n"));
    g.check("F2");
}

fn param(name: &str, unit: &str, dist: Dist) -> UncertainParam {
    UncertainParam {
        name: name.into(),
        unit: unit.into(),
        dist,
    }
}

/// F(naca4) with the model Mo and the condition of ADR 0791.
fn fan() -> FanRomModel {
    let mo = RomModel {
        n_strips: 8,
        cl_max: 1.05,
        cd_normal: 1.7,
        friction: false,
        annulus_drag: 0.025,
        secondary_drag: 0.015,
        tip_cd: 0.5,
    };
    let op = Operating {
        rpm: 2950.0,
        rho: 1.18,
        nu: 1.55e-5,
    };
    let q = 0.045;
    let s = |radius: f64, chord: f64, camber: f64, camber_pos: f64| RomSection {
        radius,
        chord,
        stagger: 0.0,
        camber,
        camber_pos,
    };
    let base = RomFan {
        n_blades: 7,
        r_hub: 0.0305,
        r_tip: 0.0575,
        tip_gap: 0.0011,
        family: serde_json::from_value(serde_json::json!("naca4")).unwrap(),
        sections: vec![s(0.029, 0.032, 0.045, 0.42), s(0.059, 0.027, 0.035, 0.46)],
    };
    let fan = free_vortex_design(&base, &mo, &op, q, 45.0 / 1.2 / op.omega()).unwrap();
    let dp = FanRom::new(fan.clone(), mo).unwrap().point(&op, q).unwrap().dp_s;
    FanRomModel {
        fan,
        model: mo,
        cond: RomCondition {
            op,
            system: SystemCurve {
                k: 0.7 * dp / (q * q),
                l: 0.3 * dp / q,
            },
            c0: 345.0,
            observer_r: 1.5,
            observer_theta: 1.1,
            n_scan: 24,
        },
    }
}

fn spec(params: &[UncertainParam], sampler: Sampler, batch: usize) -> UqSpec {
    UqSpec {
        name: "f3".into(),
        params: params.to_vec(),
        sampler,
        seed: SEED,
        stop: StopRule {
            batch,
            min_batches: 2,
            max_samples: 4 * batch,
            se_abs: 0.0,
            se_rel: 0.0,
            metrics: Vec::new(),
        },
        quantiles: vec![0.05, 0.5, 0.95],
        confidence: 0.95,
        requirements: Vec::new(),
        workers: 1,
    }
}

/// The text of `result` and of the ledger's lines.
fn study(tag: &str, f: impl FnOnce(&Ledger) -> String) -> String {
    let dir = std::env::temp_dir().join(format!("rcfd-f894-bits-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut l = Ledger::new(&dir.join("ledger.jsonl"));
    l.clock = || 1;
    let result = f(&l);
    let lines = std::fs::read_to_string(l.path()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    format!("{result}\n{lines}")
}

/// F3: both drivers on an analytic model and on the fan of ADR 0791.
#[test]
fn f3_both_drivers() {
    let analytic = FnModel {
        id: "f894".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| {
            let x: Vec<f64> = p.iter().map(|q| q.value).collect();
            let y = x[0] + 2.0 * x[1] * x[2] + x[3] * x[3];
            Outcome::ok(vec![Quantity::new("y", y, "1"), Quantity::new("w", y * x[0], "1")])
        },
    };
    let four = vec![
        param("x0", "1", normal(0.2, 0.4, -0.6, 0.9)),
        param("x1", "1", normal(0.1, 0.3, -INF, INF)),
        param("x2", "1", Dist::Uniform { lo: 2.0, hi: 5.0 }),
        param(
            "x3",
            "1",
            Dist::Beta {
                a: 2.0,
                b: 3.5,
                lo: -0.5,
                hi: 1.5,
            },
        ),
    ];
    let d3 = vec![
        param(
            "tip_gap_delta",
            "m",
            Dist::Uniform {
                lo: -1.5e-4,
                hi: 2.5e-4,
            },
        ),
        param("stagger_common_deg", "deg", normal(0.2, 0.4, -0.6, 0.9)),
        param(
            "chord_rel",
            "1",
            Dist::Beta {
                a: 2.0,
                b: 3.5,
                lo: -0.012,
                hi: 0.018,
            },
        ),
    ];
    let d1 = vec![param("stagger_common_deg", "deg", normal(0.1, 0.3, -INF, INF))];
    let fan = fan();
    let cases: [(&str, &[UncertainParam], &dyn Model, usize); 3] = [
        ("analytic", &four, &analytic, 50),
        ("fan-d3", &d3, &fan, 5),
        ("fan-d1", &d1, &fan, 5),
    ];
    let mut g = Groups::default();
    for (name, params, model, batch) in cases {
        for (s, sampler) in [("mc", Sampler::MonteCarlo), ("lhs", Sampler::Lhs)] {
            let sp = spec(params, sampler, batch);
            let t = study(&format!("{name}-{s}"), |l| {
                let r = uq::run(&sp, model, Some(l)).unwrap();
                assert!(r.n_ok >= 2, "{name} {s}: set-up: {} usable samples", r.n_ok);
                uq::result_json(&sp, &r).to_string()
            });
            g.text(&format!("F3 uq {name} {s}"), &t);
        }
        let sp = SobolSpec {
            name: "f3".into(),
            params: params.to_vec(),
            n: batch,
            seed: SEED,
            workers: 1,
        };
        let t = study(&format!("{name}-sobol"), |l| {
            let r = sensitivity::run(&sp, model, Some(l)).unwrap();
            assert!(r.n_rows_used >= 2, "{name}: set-up: {} usable rows", r.n_rows_used);
            sensitivity::result_json(&sp, &r).to_string()
        });
        g.text(&format!("F3 sobol {name}"), &t);
    }
    g.check("F3");
}

/// F4: the other callers of `erfc`.
#[test]
fn f4_other_callers() {
    let mut g = Groups::default();
    let chi: Vec<f64> = (0..=128).map(|k| chi_profile(k as f64 / 128.0, 10.0, 0.055)).collect();
    g.values("F4 chi_profile", &chi);
    let (mut ei, mut pf) = (Vec::new(), Vec::new());
    for i in -24..=24 {
        for j in -8..=8 {
            let (m, sd) = (0.37 * i as f64, 10f64.powi(j));
            ei.push(expected_improvement(m, sd, 0.2));
            pf.push(prob_feasible(m, sd));
        }
    }
    g.values("F4 expected_improvement", &ei);
    g.values("F4 prob_feasible", &pf);
    g.check("F4");
}
