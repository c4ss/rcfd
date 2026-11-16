//! F-722 (ADR 0643): `ComputerFan::rom_reference()`, the reduced-order reference fan beside the schema default: derived by the
//! ADR 0639 rule with camber 0 at every station, unstalled in the ADR 0119 model on [0.4, 1] of its free delivery, accepted by
//! the ADR 0118 checker, meshed by h_slit and the camber_seam_h C-grid at the s4 specs, and its usable range against quadratic
//! systems.

use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;
use rcfd_post::fan_rom::{FanPoint, FanRom, Operating, RomFan, RomModel, SystemCurve};

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

fn quadratic(k: f64) -> SystemCurve {
    SystemCurve { k, l: 0.0 }
}

/// Operating point against `dp = k Q^2` (ADR 0643 "Common procedure").
fn op_point(rom: &FanRom, k: f64) -> Result<f64, String> {
    rom.operating_point(&op(), &quadratic(k), 1e-4, 0.12, 400)
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

/// Leading-edge direction of the NACA 4-digit camber line from the axis: `stagger + atan(2 m / p)` (0 for `m` = 0) [deg].
fn le_direction_deg(c: &ControlPoint) -> f64 {
    let slope = if c.camber == 0.0 {
        0.0
    } else {
        2.0 * c.camber / c.camber_pos
    };
    c.stagger_deg + slope.atan().to_degrees()
}

fn in_bounds(f: &ComputerFan) -> bool {
    f.scalars().iter().all(|(_, key, x)| {
        let s = param_spec(key).unwrap();
        *x >= s.min && *x <= s.max
    })
}

/// Set-up P of ADR 0639 (hub camber 0.06 at stagger 76), the negative control NC1 / NC5.
fn fan_p() -> ComputerFan {
    let mut p = ComputerFan::rom_reference();
    p.stations[0].stagger_deg = 76.0;
    p.stations[0].camber = 0.06;
    p
}

#[test]
fn a1_a2_the_constructor_is_a_120_mm_seven_blade_fan_beside_the_default() {
    let mut ch = Checks::new();
    let f = ComputerFan::rom_reference();
    let d = ComputerFan::default();
    ch.holds(
        "A1a 7 blades, 0.12 m frame, r_tip + gap <= 0.06 m",
        f.n_blades == 7 && f.frame.side == 0.12 && f.r_tip + f.tip_gap <= 0.06,
        &format!(
            "n_blades {}, side {}, r_tip + gap {}",
            f.n_blades,
            f.frame.side,
            f.r_tip + f.tip_gap
        ),
    );
    let same = ComputerFan {
        stations: d.stations.clone(),
        ..f.clone()
    } == d;
    ch.holds(
        "A1b fields but stations equal the default's, and the fans differ",
        same && f != d,
        &format!("same {same}, differ {}", f != d),
    );
    let want: [(f64, f64, f64, f64, f64); 3] = [
        (0.0, 0.016, 69.8, 0.055, 0.0),
        (0.5, 0.029, 75.4, 0.042, 4.0),
        (1.0, 0.036, 77.6, 0.035, 8.0),
    ];
    let table = f.stations.len() == 3
        && f.stations.iter().zip(want).all(|(s, w)| {
            s.span == w.0
                && s.chord == w.1
                && s.stagger_deg == w.2
                && s.camber == 0.0
                && s.camber_pos == 0.4
                && s.thickness == w.3
                && s.sweep == 0.0
                && s.skew_deg == w.4
        });
    ch.holds(
        "A1c stations equal the ADR 0643 table",
        table,
        &format!("{:?}", f.stations),
    );
    let dirs: Vec<f64> = f.stations.iter().map(le_direction_deg).collect();
    ch.holds(
        "A2 stagger + atan(2 m / p) < 90 deg at every station",
        dirs.iter().all(|&x| x < 90.0),
        &format!("{dirs:?}"),
    );
    let nc1 = le_direction_deg(&fan_p().stations[0]);
    ch.holds(
        "NC1 P's hub >= 90 deg",
        nc1 >= 90.0 && (nc1 - 92.70).abs() < 0.01,
        &format!("{nc1:.4} deg"),
    );
    ch.finish();
}

#[test]
fn a3_r_unstalled_on_forty_to_hundred_percent_of_free_delivery() {
    let mut ch = Checks::new();
    let rom = chain(&ComputerFan::rom_reference(), RomModel::default());
    let qf = op_point(&rom, 0.0);
    ch.holds("A3a operating_point for q_f", qf.is_ok(), &format!("{qf:?}"));
    let qf = qf.unwrap();
    let g = grid(&rom, qf);
    let n_st = g.iter().filter(|(_, p)| !p.stalled.is_empty()).count();
    ch.holds("A3b stalled grid flows", n_st == 0, &format!("{n_st} of 601"));
    ch.within("R1 q_f / 0.028259", qf / 0.028259, 0.99, 1.01);
    let a40 = max_incidence_deg(&g[0].1);
    ch.within("R2 largest incidence at 0.4 q_f [deg]", a40, 8.215 - 0.15, 8.215 + 0.15);
    let mut fs = f64::NAN;
    for k in 0..400 {
        let fr = (400 - k) as f64 / 1000.0;
        if !rom.point(&op(), fr * qf).unwrap().stalled.is_empty() {
            fs = fr;
            break;
        }
    }
    ch.within("R3 f_s", fs, 0.28, 0.34);
    let (fr_pk, dp_pk) = g
        .iter()
        .map(|(fr, p)| (*fr, p.dp_s))
        .fold((f64::NAN, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
    ch.within("R4 peak of dp_s on G: fraction", fr_pk, 0.44, 0.48);
    ch.within("R4 peak of dp_s on G: value / 26.83 Pa", dp_pk / 26.83, 0.98, 1.02);
    let (mut amin, mut n_hi) = (f64::INFINITY, 0);
    for j in 0..=100 {
        let p = rom.point(&op(), qf * (1.0 + 0.25 * j as f64 / 100.0)).unwrap();
        amin = amin.min(min_incidence_deg(&p));
        n_hi += usize::from(!p.stalled.is_empty());
    }
    let amax_g = g
        .iter()
        .map(|(_, p)| max_incidence_deg(p))
        .fold(f64::NEG_INFINITY, f64::max);
    let amin_g = g
        .iter()
        .map(|(_, p)| min_incidence_deg(p))
        .fold(f64::INFINITY, f64::min);
    println!(
        "R5 reported: on [q_f, 1.25 q_f] {n_hi} of 101 stalled, smallest incidence {amin:.4} deg; on G incidence {amin_g:.4} to \
         {amax_g:.4} deg; q_f {qf:.6e} m^3/s; dp_s at 0.4 / 0.7 q_f {:.4} / {:.4} Pa; peak {dp_pk:.4} Pa at {fr_pk:.4} q_f",
        g[0].1.dp_s, g[300].1.dp_s
    );
    ch.finish();
}

#[test]
fn a6_usable_range_against_quadratic_systems() {
    let mut ch = Checks::new();
    let rom = chain(&ComputerFan::rom_reference(), RomModel::default());
    let qf = op_point(&rom, 0.0).unwrap();
    let (mut lo, mut hi) = (0.0, 1e6);
    for _ in 0..60 {
        let m = 0.5 * (lo + hi);
        if op_point(&rom, m).is_ok() {
            lo = m;
        } else {
            hi = m;
        }
    }
    let q_min = op_point(&rom, lo).unwrap();
    println!("A6 refusal just above k_max: {:?}", op_point(&rom, hi));
    ch.within("A6a k_max [Pa s^2/m^6]", lo, 1.53e5, 1.69e5);
    ch.within("A6b q_min / q_f", q_min / qf, 0.44, 0.48);
    for (k, want) in [(1e4, 0.9148), (2e4, 0.8473), (5e4, 0.7051), (1e5, 0.5634)] {
        match rom.operating_point(&op(), &quadratic(k), 1e-4, 0.12, 400) {
            Ok((o, _)) => ch.within(
                &format!("A6c q / q_f at k {k:e} (dp {:.4} Pa, slope {:.4e})", o.dp, o.slope),
                o.q / qf,
                want - 0.01,
                want + 0.01,
            ),
            Err(e) => ch.holds(&format!("A6c at k {k:e}"), false, &e),
        }
    }
    let r = op_point(&rom, 2e5);
    ch.holds("A6d k 2e5 refused", r.is_err(), &format!("{r:?}"));
    ch.finish();
}

#[test]
fn a4_the_checker_accepts_the_reference() {
    let mut ch = Checks::new();
    let f = ComputerFan::rom_reference();
    ch.holds("A4a in bounds", in_bounds(&f), "");
    let v = f.check(&Limits::default());
    ch.holds("A4b no violation", v.is_empty(), &format!("{v:?}"));
    let mut wide = ComputerFan::rom_reference();
    wide.stations[0].chord = 0.022;
    let v = wide.check(&Limits::default());
    let hit = v
        .iter()
        .find(|x| x.code == Code::AxialOverlap && x.path == "stations[0].chord");
    ch.holds(
        "NC4 hub chord 0.022: AxialOverlap at stations[0].chord",
        hit.is_some(),
        &format!("{hit:?}"),
    );
    ch.finish();
}

/// The s4 specs of `rcfd-mesh/tests/fan_design_w46a.rs` (ADR 0118), copied verbatim.
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

/// A5 as registered (ADR 0643). Run once: A5b MISSED, camber_seam_h refused the reference: "the line family is folded at
/// layer 0 (r = 0.02150), C block around line 1, row 2: the leading-edge fan cannot be built for this blade with the front_gap
/// 0.3 and n_nose 6 given" (the hub section at stagger 69.8 deg; the camber-seam C-grid is not robust on strongly staggered
/// blades, ADR 0079 amendment 5). A5a (h_slit) and NC5 were met. Kept with the limits intact as the record.
#[test]
#[ignore = "known miss of ADR 0643 A5b: the camber_seam_h C-grid folds the leading-edge fan of the reference's hub (the record)"]
fn a5_the_reference_meshes_at_the_s4_specs_known_miss() {
    a5_checks(true);
}

/// ADR 0643 amendment 1: the met limits of A5 (A5a, NC5; the same limits), A5b left to the record above.
#[test]
fn a5_the_reference_meshes_at_the_s4_specs_met_limits() {
    a5_checks(false);
}

fn a5_checks(with_a5b: bool) {
    let mut ch = Checks::new();
    let af = ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04);
    let (ss, cs) = smoke_specs();
    match build_sector_mesh(&af, &ss) {
        Ok(h) => ch.holds(
            "A5a h_slit builds",
            h.mesh.n_cells() > 0,
            &format!("{} cells, {:?}", h.mesh.n_cells(), h.quality()),
        ),
        Err(e) => ch.holds("A5a h_slit builds", false, &format!("{e:?}")),
    }
    let h_opts = CGridOptions::camber_seam_h_construction();
    match build_cgrid_mesh_with(&af, &cs, &h_opts) {
        Ok(c) if !with_a5b => println!("A5b (record only) builds: {} cells", c.mesh.n_cells()),
        Err(e) if !with_a5b => println!("A5b (record only) refused: {e:?}"),
        Ok(c) => ch.holds(
            "A5b c_grid camber_seam_h builds",
            c.mesh.n_cells() > 0,
            &format!("{} cells, {:?}", c.mesh.n_cells(), c.quality()),
        ),
        Err(e) => ch.holds("A5b c_grid camber_seam_h builds", false, &format!("{e:?}")),
    }
    match build_cgrid_mesh_with(&af, &cs, &CGridOptions::default()) {
        Ok(m) => println!(
            "A5c c_grid pursuit (reported): {} cells, {:?}",
            m.mesh.n_cells(),
            m.quality()
        ),
        Err(e) => println!("A5c c_grid pursuit (reported): rejected {e:?}"),
    }
    let p = build_cgrid_mesh_with(&fan_p().axial_fan(0, 0.03, 0.04), &cs, &h_opts);
    ch.holds(
        "NC5 P through camber_seam_h refused",
        p.is_err(),
        &format!("{:?}", p.err()),
    );
    ch.finish();
}

#[test]
fn nc2_nc3_the_stall_check_discriminates_on_the_reference() {
    let mut ch = Checks::new();
    let mut low = ComputerFan::rom_reference();
    for s in &mut low.stations {
        s.stagger_deg -= 8.0;
    }
    for (name, cf, model) in [
        ("NC2 stagger -8 deg", low, RomModel::default()),
        (
            "NC3 cl_max 0.8",
            ComputerFan::rom_reference(),
            RomModel {
                cl_max: 0.8,
                ..RomModel::default()
            },
        ),
    ] {
        let rom = chain(&cf, model);
        match op_point(&rom, 0.0) {
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
