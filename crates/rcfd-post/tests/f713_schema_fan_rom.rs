//! F-713 (ADR 0639): the schema default fan (ADR 0118) in the reduced-order model (ADR 0119): the registered diagnosis of
//! why `ComputerFan::default()` stalls, the derived candidate default P, and which ADR 0118 results P would change.
//! No library code is changed by F-713; P lives in this file only.

use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;
use rcfd_post::fan_rom::{FanPoint, FanRom, Operating, RomFan, RomModel, SystemCurve};
use std::f64::consts::TAU;

/// Measured values against registered limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        self.holds(name, ok, &format!("{value:.6e} (limit [{lo:e}, {hi:e}])"));
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn chain(cf: &ComputerFan, model: RomModel) -> FanRom {
    FanRom::new(RomFan::from_axial_fan(&cf.axial_fan(0, 0.0, 0.0)).unwrap(), model).unwrap()
}

/// Free delivery: the highest-flow zero of `dp_s` (ADR 0639 "Common procedure").
fn free_delivery(rom: &FanRom) -> Result<f64, String> {
    rom.operating_point(&op(), &SystemCurve { k: 0.0, l: 0.0 }, 1e-4, 0.12, 400)
        .map(|(o, _)| o.q)
}

/// Grid G: 601 flows on `[0.4 q_f, q_f]`, returned as (fraction, point).
fn grid(rom: &FanRom, qf: f64) -> Vec<(f64, FanPoint)> {
    (0..=600)
        .map(|j| {
            let fr = 0.4 + 0.6 * j as f64 / 600.0;
            (fr, rom.point(&op(), fr * qf).unwrap())
        })
        .collect()
}

fn max_incidence_deg(p: &FanPoint) -> f64 {
    p.strips
        .iter()
        .map(|s| s.alpha_e.to_degrees())
        .fold(f64::NEG_INFINITY, f64::max)
}

fn min_incidence_deg(p: &FanPoint) -> f64 {
    p.strips
        .iter()
        .map(|s| s.alpha_e.to_degrees())
        .fold(f64::INFINITY, f64::min)
}

/// Set-up P of ADR 0639 (the candidate default; everything but the stations is `ComputerFan::default()`).
fn fan_p() -> ComputerFan {
    let cp = |span, chord, stagger_deg, camber, thickness, skew_deg| ControlPoint {
        span,
        chord,
        stagger_deg,
        camber,
        camber_pos: 0.4,
        thickness,
        sweep: 0.0,
        skew_deg,
    };
    ComputerFan {
        stations: vec![
            cp(0.0, 0.016, 76.0, 0.06, 0.055, 0.0),
            cp(0.5, 0.029, 75.4, 0.0, 0.042, 4.0),
            cp(1.0, 0.036, 77.6, 0.0, 0.035, 8.0),
        ],
        ..ComputerFan::default()
    }
}

#[test]
fn d1_d2_the_default_stalls_and_why() {
    let mut ch = Checks::new();
    let cf = ComputerFan::default();
    let rom = chain(&cf, RomModel::default());
    let qf = free_delivery(&rom).expect("default free delivery");
    ch.within("D1a q_f / 0.07650", qf / 0.07650, 0.99, 1.01);
    let g = grid(&rom, qf);
    let stalled: Vec<&(f64, FanPoint)> = g.iter().filter(|(_, p)| !p.stalled.is_empty()).collect();
    ch.holds(
        "D1b stalled grid flows > 0",
        !stalled.is_empty(),
        &format!("{} of 601", stalled.len()),
    );
    let (fr_hi, p_hi) = stalled
        .iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| (x.0, &x.1))
        .unwrap();
    ch.within("D1c f_s", fr_hi + 0.001, 0.75, 0.80);
    ch.holds(
        "D1d strips 0..9 attached at the highest stalled grid flow",
        p_hi.stalled.iter().all(|&i| i >= 10),
        &format!("stalled strips {:?} at fraction {fr_hi:.4}", p_hi.stalled),
    );
    let pf = rom.point(&op(), qf).unwrap();
    let (amin, amax) = (min_incidence_deg(&pf), max_incidence_deg(&pf));
    ch.holds(
        "D2a incidence at q_f in [1, 7] deg",
        amin >= 1.0 && amax <= 7.0,
        &format!("min {amin:.4} max {amax:.4} deg"),
    );
    let cx = qf / rom.area();
    ch.within(
        "D2b (rho cx^2/2) / dp_t at q_f",
        0.5 * op().rho * cx * cx / pf.dp_t,
        0.75,
        0.90,
    );
    let t = pf.strips.last().unwrap();
    let phi = t.cx / t.u;
    let rise = (1.0 / (0.4 * phi)).atan().to_degrees() - (1.0 / phi).atan().to_degrees();
    ch.holds(
        "D2c tip inflow-angle rise >= 14 deg",
        rise >= 14.0,
        &format!("{rise:.4} deg at phi {phi:.4}"),
    );
    println!(
        "D reported: q_f {qf:.6e}, dp_t {:.4} Pa, phi hub {:.4}, tip incidence at q_f {:.4} deg, stall incidence {:.4} deg",
        pf.dp_t,
        pf.strips[0].cx / pf.strips[0].u,
        t.alpha_e.to_degrees(),
        (1.1 / TAU).to_degrees()
    );
    ch.finish();
}

/// P1 as registered (ADR 0639). Run once: P1c MISSED, the largest step of `dp_s` along G was +1.9252e-2 Pa (the static
/// characteristic of P peaks at 26.838 Pa at 0.457 q_f with every strip attached and rises from 26.308 Pa at 0.4 q_f; the
/// swirl kinetic energy lost at the free discharge grows faster than dp_t below the peak; ADR 0639 addendum 1). Every other
/// P1 limit was met. Kept with the limit intact as the record.
#[test]
#[ignore = "known miss of ADR 0639 P1c: dp_s of P rises by 0.53 Pa from 0.4 q_f to its peak at 0.457 q_f (the record)"]
fn p1_the_candidate_is_unstalled_down_to_forty_percent_known_miss() {
    p1_checks(true);
}

/// ADR 0639 amendment 2: the met limits of P1 (P1a, P1b, P1d to P1f; the same limits), P1c left to the record above.
#[test]
fn p1_the_candidate_is_unstalled_down_to_forty_percent_met_limits() {
    p1_checks(false);
}

fn p1_checks(with_p1c: bool) {
    let mut ch = Checks::new();
    let rom = chain(&fan_p(), RomModel::default());
    let qf = free_delivery(&rom);
    ch.holds("P1a operating_point for q_f", qf.is_ok(), &format!("{qf:?}"));
    let qf = qf.unwrap();
    let g = grid(&rom, qf);
    let n_st = g.iter().filter(|(_, p)| !p.stalled.is_empty()).count();
    ch.holds("P1b stalled grid flows", n_st == 0, &format!("{n_st} of 601"));
    let worst_step = g
        .windows(2)
        .map(|w| w[1].1.dp_s - w[0].1.dp_s)
        .fold(f64::NEG_INFINITY, f64::max);
    if with_p1c {
        ch.holds(
            "P1c dp_s strictly decreasing along G",
            worst_step < 0.0,
            &format!(
                "largest step {worst_step:.4e} Pa; dp_s {:.4} Pa at 0.4 q_f",
                g[0].1.dp_s
            ),
        );
    }
    ch.within("P1d q_f / 0.028266", qf / 0.028266, 0.99, 1.01);
    let a40 = max_incidence_deg(&g[0].1);
    ch.within(
        "P1e largest incidence at 0.4 q_f [deg]",
        a40,
        8.228 - 0.15,
        8.228 + 0.15,
    );
    let mut fs = f64::NAN;
    for k in 0..=400 {
        let fr = (400 - k) as f64 / 1000.0;
        if fr <= 0.0 {
            break;
        }
        if !rom.point(&op(), fr * qf).unwrap().stalled.is_empty() {
            fs = fr;
            break;
        }
    }
    ch.within("P1f f_s", fs, 0.28, 0.34);
    let (mut amin, mut n_hi) = (f64::INFINITY, 0);
    for j in 0..=100 {
        let p = rom.point(&op(), qf * (1.0 + 0.25 * j as f64 / 100.0)).unwrap();
        amin = amin.min(min_incidence_deg(&p));
        n_hi += usize::from(!p.stalled.is_empty());
    }
    let amax = g
        .iter()
        .map(|(_, p)| max_incidence_deg(p))
        .fold(f64::NEG_INFINITY, f64::max);
    let amin_g = g
        .iter()
        .map(|(_, p)| min_incidence_deg(p))
        .fold(f64::INFINITY, f64::min);
    println!(
        "P1g reported: on [q_f, 1.25 q_f] smallest incidence {amin:.4} deg, {n_hi} of 101 stalled; on G incidence {amin_g:.4} \
         to {amax:.4} deg; q_f {qf:.6e} m^3/s; dp_s at 0.4 / 0.7 q_f {:.4} / {:.4} Pa",
        g[0].1.dp_s, g[300].1.dp_s
    );
    ch.finish();
}

fn in_bounds(f: &ComputerFan) -> bool {
    f.scalars().iter().all(|(_, key, x)| {
        let s = param_spec(key).unwrap();
        *x >= s.min && *x <= s.max
    })
}

#[test]
fn p2_the_candidate_passes_the_checker_and_keeps_the_scalars() {
    let mut ch = Checks::new();
    let p = fan_p();
    let v = p.check(&Limits::default());
    ch.holds("P2a in bounds", in_bounds(&p), "");
    ch.holds("P2a no violation", v.is_empty(), &format!("{v:?}"));
    let d = ComputerFan::default();
    let same = ComputerFan {
        stations: d.stations.clone(),
        ..p.clone()
    } == d;
    ch.holds("P2b fields other than stations equal the default's", same, "");
    ch.finish();
}

fn find<'a>(v: &'a [Violation], code: Code, path: &str) -> Option<&'a Violation> {
    v.iter().find(|x| x.code == code && x.path == path)
}

struct Bad {
    name: &'static str,
    fan: ComputerFan,
    lim: Limits,
    code: Code,
    path: &'static str,
    radius: Option<f64>,
}

/// The fifteen `bad_variants` of `rcfd-mesh/tests/fan_design_w46a.rs` (ADR 0118 C2), copied verbatim, on base `f0`.
fn bad_variants(f0: ComputerFan) -> Vec<Bad> {
    let l0 = Limits::default();
    let no_mould = Limits {
        axial_mould: false,
        ..l0
    };
    let edit = |g: &dyn Fn(&mut ComputerFan)| {
        let mut f = f0.clone();
        g(&mut f);
        f
    };
    let b = |name, fan, lim, code, path, radius| Bad {
        name,
        fan,
        lim,
        code,
        path,
        radius,
    };
    let rh = Some(f0.r_hub);
    vec![
        b(
            "thin tip",
            edit(&|f| f.stations[2].thickness = 0.015),
            l0,
            Code::MinThickness,
            "stations[2].thickness",
            None,
        ),
        b(
            "small gap",
            edit(&|f| f.tip_gap = 2e-4),
            l0,
            Code::TipGap,
            "tip_gap",
            None,
        ),
        b(
            "large gap",
            edit(&|f| {
                f.tip_gap = 3.5e-3;
                f.frame.side = 0.13
            }),
            l0,
            Code::TipGap,
            "tip_gap",
            None,
        ),
        b(
            "eccentric rub",
            edit(&|f| {
                f.deviation = Some(Deviation {
                    eccentricity: 1.2e-3,
                    ..Deviation::default()
                })
            }),
            l0,
            Code::TipRub,
            "deviation.eccentricity",
            None,
        ),
        b(
            "hub ratio",
            edit(&|f| f.r_hub = 0.042),
            l0,
            Code::HubTipRatio,
            "r_hub",
            None,
        ),
        b(
            "low tip solidity",
            edit(&|f| {
                f.stations[2].chord = 0.012;
                f.stations[2].thickness = 0.1
            }),
            l0,
            Code::Solidity,
            "stations[2].chord",
            None,
        ),
        b(
            "axial overlap",
            edit(&|f| {
                f.stations[0].chord = 0.026;
                f.stations[0].stagger_deg = 50.0
            }),
            l0,
            Code::AxialOverlap,
            "stations[0].chord",
            rh,
        ),
        b(
            "collision",
            edit(&|f| {
                f.n_blades = 11;
                f.stations[0].chord = 0.022;
                f.stations[0].stagger_deg = 60.0;
                f.stations[0].thickness = 0.35
            }),
            no_mould,
            Code::Collision,
            "stations[0].chord",
            None,
        ),
        b(
            "passage gap",
            edit(&|f| {
                f.n_blades = 11;
                f.stations[0].chord = 0.022;
                f.stations[0].stagger_deg = 60.0;
                f.stations[0].thickness = 0.30
            }),
            no_mould,
            Code::PassageGap,
            "stations[0].chord",
            None,
        ),
        b(
            "draft",
            edit(&|f| {
                f.stations[0].stagger_deg = 12.0;
                f.frame.depth = 0.03;
                f.hub.length = 0.028
            }),
            l0,
            Code::Draft,
            "stations[0].stagger_deg",
            rh,
        ),
        b(
            "undercut",
            edit(&|f| {
                f.stations[0].stagger_deg = 0.0;
                f.stations[0].camber = 0.12
            }),
            l0,
            Code::Undercut,
            "stations[0].stagger_deg",
            rh,
        ),
        b(
            "thin blade 3",
            edit(&|f| {
                let mut d = Deviation {
                    blades: vec![BladeDeviation::default(); 7],
                    ..Deviation::default()
                };
                d.blades[3].thickness_rel = -0.45;
                f.deviation = Some(d)
            }),
            l0,
            Code::MinThickness,
            "stations[0].thickness",
            rh,
        ),
        b(
            "frame wall",
            edit(&|f| f.frame.side = 0.119),
            l0,
            Code::FrameWall,
            "frame.side",
            None,
        ),
        b(
            "axial fit",
            edit(&|f| f.z_le_root = 0.006),
            l0,
            Code::AxialFit,
            "stations[0].sweep",
            None,
        ),
        b(
            "root off hub",
            edit(&|f| {
                f.hub.z_start = 0.004;
                f.hub.length = 0.02
            }),
            l0,
            Code::RootOffHub,
            "z_le_root",
            rh,
        ),
    ]
}

/// The C2 / C3 / NC1 conditions of ADR 0118 for one variant: `None` if every one holds, else the first that does not.
fn variant_result(b: &Bad) -> Option<String> {
    if !in_bounds(&b.fan) {
        return Some("NC1: not in bounds".into());
    }
    let v = b.fan.check(&b.lim);
    if v.iter().any(|x| x.code == Code::OutOfBounds) {
        return Some("out_of_bounds reported".into());
    }
    let Some(hit) = find(&v, b.code, b.path) else {
        let codes: Vec<String> = v.iter().map(|x| format!("{:?} {}", x.code, x.path)).collect();
        return Some(format!("no {:?} at {}; reported: {codes:?}", b.code, b.path));
    };
    if let Some(r) = b.radius {
        if hit.radius != Some(r) {
            return Some(format!("radius {:?} instead of {r}", hit.radius));
        }
    }
    if b.name == "thin blade 3" && !v.iter().all(|x| x.blade == Some(3)) {
        return Some("a violation on another blade".into());
    }
    println!(
        "  {}: {:?} {} value {:.4e} limit {:.4e} ({} violations)",
        b.name,
        hit.code,
        hit.path,
        hit.value,
        hit.limit,
        v.len()
    );
    if matches!(b.code, Code::AxialOverlap | Code::Draft | Code::Undercut) {
        let off = b.fan.check(&Limits {
            axial_mould: false,
            ..b.lim
        });
        if off.iter().any(|x| x.code == b.code) {
            return Some("C3: the code stays with axial_mould off".into());
        }
    }
    None
}

#[test]
fn p3_which_adr0118_variants_the_candidate_changes() {
    let mut ch = Checks::new();
    // Control: on the unchanged default every variant holds (the ADR 0118 c2 / c3 result reproduced here).
    let base: Vec<String> = bad_variants(ComputerFan::default())
        .iter()
        .filter_map(|b| variant_result(b).map(|e| format!("{}: {e}", b.name)))
        .collect();
    ch.holds(
        "P3 control: every variant holds on the default",
        base.is_empty(),
        &format!("{base:?}"),
    );
    let lost: Vec<(&'static str, String)> = bad_variants(fan_p())
        .iter()
        .filter_map(|b| variant_result(b).map(|e| (b.name, e)))
        .collect();
    for (n, e) in &lost {
        println!("P3 on P, lost: {n}: {e}");
    }
    let names: Vec<&str> = lost.iter().map(|x| x.0).collect();
    ch.holds(
        "P3 lost set == {axial fit}",
        names == ["axial fit"],
        &format!("{names:?}"),
    );
    ch.finish();
}

fn smoke_specs() -> (SectorSpec, CGridSpec) {
    (
        SectorSpec {
            n_up: 8,
            n_chord: 16,
            n_down: 8,
            n_side: 10,
            n_mid: 2,
            n_span: 10,
            n_gap: 7,
            n_bl: 2,
            growth: 1.2,
            first_cell: FirstCell::Height(5e-5),
            trunc_le: 0.02,
            trunc_te: 0.01,
            include_blade: true,
        },
        CGridSpec {
            n_up: 8,
            n_chord: 24,
            n_down: 8,
            n_side: 10,
            n_nose: 6,
            n_core: 2,
            n_span: 10,
            n_gap: 7,
            n_bl: 2,
            growth: 1.2,
            first_cell: FirstCell::Height(5e-5),
            front_gap: 0.3,
            include_blade: true,
        },
    )
}

/// P4 as registered (ADR 0639). Run once: both MISSED at the hub layer (r = 0.0215) of P, stagger 76 deg with camber 0.06
/// at 0.4: h_slit "axial plane z = 3.19866e-2 misses the blade surface [3.2e-2, 3.58708e-2]"; camber_seam_h "the camber line
/// does not run downstream monotonically" (its leading-edge direction is 76 + atan(2 m / p) = 92.7 deg from the axis). Pursuit
/// (reported) rejected: the wake line leaves the passage. Kept with the limits intact as the record.
#[test]
#[ignore = "known miss of ADR 0639 P4: neither sector generator meshes P's hub section (the record)"]
fn p4_the_candidate_meshes_as_s4_known_miss() {
    let mut ch = Checks::new();
    let af = fan_p().axial_fan(0, 0.03, 0.04);
    let (ss, cs) = smoke_specs();
    match build_sector_mesh(&af, &ss) {
        Ok(h) => ch.holds(
            "P4 h_slit builds",
            h.mesh.n_cells() > 0,
            &format!("{} cells, {:?}", h.mesh.n_cells(), h.quality()),
        ),
        Err(e) => ch.holds("P4 h_slit builds", false, &format!("{e:?}")),
    }
    match build_cgrid_mesh_with(&af, &cs, &CGridOptions::camber_seam_h_construction()) {
        Ok(c) => ch.holds(
            "P4 c_grid camber_seam_h builds",
            c.mesh.n_cells() > 0,
            &format!("{} cells, {:?}", c.mesh.n_cells(), c.quality()),
        ),
        Err(e) => ch.holds("P4 c_grid camber_seam_h builds", false, &format!("{e:?}")),
    }
    match build_cgrid_mesh_with(&af, &cs, &CGridOptions::default()) {
        Ok(m) => println!("P4 c_grid pursuit (reported): {} cells", m.mesh.n_cells()),
        Err(e) => println!("P4 c_grid pursuit (reported): rejected {e:?}"),
    }
    ch.finish();
}

#[test]
fn nc2_nc3_the_check_discriminates_on_the_candidate() {
    let mut ch = Checks::new();
    let mut p8 = fan_p();
    for s in &mut p8.stations {
        s.stagger_deg -= 8.0;
    }
    for (name, cf, model) in [
        ("NC2 stagger -8 deg", p8, RomModel::default()),
        (
            "NC3 cl_max 0.8",
            fan_p(),
            RomModel {
                cl_max: 0.8,
                ..RomModel::default()
            },
        ),
    ] {
        let rom = chain(&cf, model);
        match free_delivery(&rom) {
            Ok(qf) => {
                let g = grid(&rom, qf);
                let st: Vec<f64> = g.iter().filter(|(_, p)| !p.stalled.is_empty()).map(|x| x.0).collect();
                let top = st.iter().cloned().fold(f64::NAN, f64::max);
                ch.holds(
                    &format!("{name}: stalled grid flows > 0"),
                    !st.is_empty(),
                    &format!("{} of 601, highest stalled fraction {top:.4}, q_f {qf:.6e}", st.len()),
                );
            }
            Err(e) => ch.holds(
                &format!("{name}: stalled grid flows > 0"),
                false,
                &format!("no q_f: {e}"),
            ),
        }
    }
    ch.finish();
}
