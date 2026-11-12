//! Droplet feedback in the transient two-way path (F-326, ADR 0348). The limits are those registered in ADR 0348 (with its
//! amendment 1) before this file was written; none was changed afterwards.
//!
//! CB: a closed adiabatic box of hot gas (16 cells) with a vapour species; ten parcels of drops injected at 60 m/s break up
//! (TAB) and evaporate (data E of ADR 0224). The carrier receives the vapour as a species source, the drag and vapour momentum
//! as a body acceleration and the sensible heat in its temperature equation; the identities IM, IP, IE of ADR 0348 are
//! checked at the end of every step.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_particles::State;
use std::f64::consts::PI;

const RHO: f64 = 0.5;
const CP: f64 = 1100.0;
const C_L: f64 = 2200.0;
const RHO_L: f64 = 700.0;
const D0: f64 = 2e-4;
const N0: f64 = 5.0;
const V_INJ: f64 = 60.0;
const T_INJ: f64 = 350.0;

type V3 = [f64; 3];

fn vnorm(v: V3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

const EVAP: &str = r#"
[particles.evaporation]
enabled = true
species = "vap"
latent_heat = 3.2e5
boiling_temperature = 371.5
boiling_pressure = 101325.0
pressure = 101325.0
vapour_molar_mass = 0.1
gas_molar_mass = 0.029
diffusivity = 3e-5
vapour_cp = 2500.0
min_diameter = 1e-7
max_step_fraction = 0.1
"#;

const BREAKUP: &str = r#"
[particles.breakup]
enabled = true
surface_tension = 0.01
liquid_viscosity = 4e-4
seed = 31
"#;

const SPECIES: &str = r#"
[species.vap]
diffusivity = 3e-5
initial = 0.0
[species.air]
remainder = true
initial = 1.0
"#;

/// The case text of ADR 0348: `solver` lines, `energy` on, `heat_transfer`, the species block and the tables appended.
fn text(solver: &str, energy: bool, species: bool, tables: &str) -> String {
    let energy_block = if energy {
        "[energy]\nenabled = true\nbuoyancy = false\n"
    } else {
        ""
    };
    let heat = if energy {
        "heat_transfer = true\nparticle_cp = 2200.0\n"
    } else {
        ""
    };
    let temp = if energy { "temperature = 350.0\n" } else { "" };
    let mut walls = String::new();
    for p in ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"] {
        walls.push_str(&format!(
            "[boundaries.{p}]\nvelocity = \"fixed\"\ntemperature = \"zero_gradient\"\n"
        ));
    }
    format!(
        r#"
title = "droplet feedback CB"
[mesh]
generator = "box_3d"
nx = 4
ny = 2
nz = 2
lx = 0.04
ly = 0.02
lz = 0.02
[physics]
nu = 6e-5
rho = 0.5
cp = 1100.0
conductivity = 0.05
gravity = [0.0, 0.0, -9.81]
[initial]
temperature = 800.0
{walls}{energy_block}[solver]
{solver}
[particles]
enabled = true
density = 700.0
drag = "schiller_naumann"
integration = "analytic"
gravity = true
restitution = 0.8
end_time = 5e-3
two_way = true
{heat}[particles.injection.a]
kind = "point"
position = [0.005, 0.0075, 0.0125]
velocity_mode = "given"
velocity = [60.0, 0.0, 0.0]
diameter = 2e-4
parcel_count = 5.0
rate = 1e4
start_time = 0.0
end_time = 1e-3
{temp}{species_block}{tables}"#,
        species_block = if species { SPECIES } else { "" },
    )
}

const PISO: &str = "algorithm = \"piso\"\ndt = 1e-5\nend_time = 5e-3\npressure_rel_tol = 1e-10";
const PIMPLE: &str = "algorithm = \"pimple\"\nn_outer = 2\ndt = 1e-5\nend_time = 1e-3\npressure_rel_tol = 1e-10";

fn cb_text() -> String {
    text(PISO, true, true, &format!("{EVAP}{BREAKUP}"))
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

fn build(t: &str) -> Simulation {
    Simulation::new(&parse(t)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(t: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(t) {
        Err(e) => e,
        Ok(c) => c.semantic_errors(),
    }
}

/// Residuals of IM, IP, IE (relative to the largest term of each) and the terms, from the state of `sim` after a step.
struct Ident {
    rm: f64,
    rp: f64,
    re: f64,
    m_terms: [f64; 3],
    p_terms: [f64; 6],
    e_terms: [f64; 8],
    /// IE with `H_v` left out (NC1), IM without the carrier's vapour (NC2), IE with the carrier inventory replaced by the
    /// record's `sum energy_heat` (NC3).
    nc: [f64; 3],
}

fn identities(sim: &Simulation, vol: &[f64], t0: &[f64], y0: Option<&[f64]>, heat: bool) -> Ident {
    let pr = sim.particle_run().expect("particle run");
    let tw = pr.coupling().expect("two-way");
    let led = tw.droplets.as_ref().expect("droplet ledger");
    let tr = &pr.tracker;
    let n = tr.injected() as f64;
    let m0 = N0 * RHO_L * PI / 6.0 * D0 * D0 * D0;
    let (m_inj, p_inj, e_inj) = (
        n * m0,
        [n * m0 * V_INJ, 0.0, 0.0],
        n * m0 * (0.5 * V_INJ * V_INJ + if heat { C_L * T_INJ } else { 0.0 }),
    );
    let (mut m_liq, mut p_liq, mut e_liq) = (0.0, [0.0; 3], 0.0);
    for (i, p) in tr.particles.iter().enumerate() {
        let m = p.parcel * p.rho * PI / 6.0 * p.d * p.d * p.d;
        m_liq += m;
        for k in 0..3 {
            p_liq[k] += m * p.v[k];
        }
        e_liq += 0.5 * m * (p.v[0] * p.v[0] + p.v[1] * p.v[1] + p.v[2] * p.v[2]);
        if heat {
            e_liq += m * C_L * tr.temperature(i);
        }
    }
    let m_gas = match y0 {
        Some(y0) => {
            let y = sim.field("vap").unwrap();
            RHO * (0..vol.len()).map(|c| vol[c] * (y[c] - y0[c])).sum::<f64>()
        }
        None => 0.0,
    };
    let m_scale = m_liq.abs().max(m_gas.abs()).max(m_inj);
    let rm = (m_liq + m_gas - m_inj).abs() / m_scale;
    let nc2 = (m_liq - m_inj).abs() / m_scale;
    let mut res = [0.0; 3];
    for k in 0..3 {
        res[k] = p_liq[k] + tw.delivered[k]
            - led.gravity_impulse[k]
            - led.wall_impulse[k]
            - led.breakup_impulse[k]
            - p_inj[k];
    }
    let p_terms = [
        vnorm(p_liq),
        vnorm(tw.delivered),
        vnorm(led.gravity_impulse),
        vnorm(led.wall_impulse),
        vnorm(led.breakup_impulse),
        vnorm(p_inj),
    ];
    let rp = vnorm(res) / p_terms.iter().cloned().fold(0.0, f64::max);
    let e_gas = if heat {
        let t = sim.field("T").unwrap();
        RHO * CP * (0..vol.len()).map(|c| vol[c] * (t[c] - t0[c])).sum::<f64>()
    } else {
        0.0
    };
    let e_terms = [
        e_liq,
        e_gas,
        led.vapour_held,
        led.mechanical,
        led.gravity_work,
        led.wall_energy,
        led.breakup_energy,
        e_inj,
    ];
    let e_scale = e_terms.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let ext = led.gravity_work + led.wall_energy + led.breakup_energy;
    let re = (e_liq + e_gas + led.vapour_held + led.mechanical - ext - e_inj).abs() / e_scale;
    let nc1 = (e_liq + e_gas + led.mechanical - ext - e_inj).abs() / e_scale;
    let nc3 = (e_liq + led.heat_raw + led.vapour_held + led.mechanical - ext - e_inj).abs() / e_scale;
    Ident {
        rm,
        rp,
        re,
        m_terms: [m_liq, m_gas, m_inj],
        p_terms,
        e_terms,
        nc: [nc1, nc2, nc3],
    }
}

/// [`identities`] with ADR 0348 A2.1: a residual is 0 when every term of its identity is 0 (no parcel injected yet).
fn identities_a2(sim: &Simulation, vol: &[f64], t0: &[f64], y0: Option<&[f64]>, heat: bool) -> Ident {
    let mut id = identities(sim, vol, t0, y0, heat);
    if id.m_terms.iter().all(|x| *x == 0.0) {
        id.rm = 0.0;
    }
    if id.p_terms.iter().all(|x| *x == 0.0) {
        id.rp = 0.0;
    }
    if id.e_terms.iter().all(|x| *x == 0.0) {
        id.re = 0.0;
    }
    id
}

/// The identities of the first run (`amended = false`) or of amendment 2 (A2.1).
fn ident(amended: bool, sim: &Simulation, vol: &[f64], t0: &[f64], y0: Option<&[f64]>, heat: bool) -> Ident {
    if amended {
        identities_a2(sim, vol, t0, y0, heat)
    } else {
        identities(sim, vol, t0, y0, heat)
    }
}

/// C1, C1b, C2, C3, C3b, SR, R1, CV and NC1 to NC3 on CB (one run, 500 steps), as amended by ADR 0348 amendments 2 and 3.
#[test]
fn cb_closed_box_conserves_mass_momentum_and_energy_a3() {
    cb_run(true, true);
}

/// The run of amendment 2, kept as a record (ADR 0348 amendment 3): "C3 IE step 1: 1e0" (before the first parcel the only
/// non-zero term is the round-off of the carrier inventory).
#[test]
#[ignore = "SETUP DEFECT record (ADR 0348 amendment 3): C3 IE step 1 = 1, the round-off of the carrier inventory over itself"]
fn cb_closed_box_conserves_mass_momentum_and_energy_setup_defect2() {
    cb_run(true, false);
}

/// The first run of CB, kept as a record (ADR 0348 amendment 2): it stopped at "C3b step 1: 7.99e-4" (the floor as a
/// denominator).
#[test]
#[ignore = "SETUP DEFECT record (ADR 0348 amendment 2): C3b step 1 7.99e-4, the round-off floor used as a denominator"]
fn cb_closed_box_conserves_mass_momentum_and_energy_setup_defect() {
    cb_run(false, false);
}

/// `amended`: A2.1 and A2.2; `injected_only`: A3.1 (the identities once a parcel is injected).
fn cb_run(amended: bool, injected_only: bool) {
    let mut sim = build(&cb_text());
    let vol = sim.geom.mesh.cell_volumes().to_vec();
    let vsum: f64 = vol.iter().sum();
    let t0 = sim.field("T").unwrap();
    let y0 = sim.field("vap").unwrap();
    let (mut wm, mut wp, mut we, mut w1b, mut w3b, mut w2b) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut steps = 0;
    let (mut y_min, mut y_max, mut t_min) = (f64::MAX, f64::MIN, f64::MAX);
    while !sim.is_done() {
        let t_before = sim.field("T").unwrap();
        let y_before = sim.field("vap").unwrap();
        sim.step();
        steps += 1;
        assert!(sim.failure().is_none(), "step {steps}: {:?}", sim.failure());
        let pr = sim.particle_run().unwrap();
        let tw = pr.coupling().unwrap();
        let led = tw.droplets.as_ref().unwrap();
        // R1: the tracker read the carrier temperature and the vapour fraction of the step
        let th = pr.tracker.thermal().expect("thermal");
        let tb: Vec<u64> = t_before.iter().map(|x| x.to_bits()).collect();
        let tt: Vec<u64> = th.t_fluid.iter().map(|x| x.to_bits()).collect();
        assert_eq!(tt, tb, "R1 temperature, step {steps}");
        let yb: Vec<u64> = y_before.iter().map(|x| x.max(0.0).to_bits()).collect();
        let yt: Vec<u64> = pr
            .tracker
            .evaporation()
            .expect("evaporation")
            .vapour_fraction()
            .iter()
            .map(|x| x.to_bits())
            .collect();
        assert_eq!(yt, yb, "R1 vapour fraction, step {steps}");
        // C1b: the species received the step's vapour
        let y = sim.field("vap").unwrap();
        let dy = RHO * (0..vol.len()).map(|c| vol[c] * (y[c] - y_before[c])).sum::<f64>();
        let r1b = if amended {
            // A2.2: the registered floor as a round-off allowance; the printed number is the error over the bound
            (dy - led.last_vapour).abs() / (1e-8 * led.last_vapour_abs + 1e-13 * RHO * vsum)
        } else {
            (dy - led.last_vapour).abs() / (led.last_vapour_abs + 1e-13 * RHO * vsum)
        };
        w1b = w1b.max(r1b);
        if amended {
            assert!(r1b <= 1.0, "C1b (A2.2) step {steps}: error over bound {r1b:e}");
        } else {
            assert!(r1b <= 1e-8, "C1b step {steps}: {r1b:e}");
        }
        // C3b: the temperature equation received the step's sensible heat
        let t = sim.field("T").unwrap();
        let dt = RHO * CP * (0..vol.len()).map(|c| vol[c] * (t[c] - t_before[c])).sum::<f64>();
        let inv = RHO * CP * (0..vol.len()).map(|c| vol[c] * t[c].abs()).sum::<f64>();
        let r3b = if amended {
            // A2.2: the registered floor as a round-off allowance; the printed number is the error over the bound
            (dt - led.last_heat).abs() / (1e-8 * led.last_heat_abs + 1e-13 * inv)
        } else {
            (dt - led.last_heat).abs() / (led.last_heat_abs + 1e-13 * inv)
        };
        w3b = w3b.max(r3b);
        if amended {
            assert!(r3b <= 1.0, "C3b (A2.2) step {steps}: error over bound {r3b:e}");
        } else {
            assert!(r3b <= 1e-8, "C3b step {steps}: {r3b:e}");
        }
        // C2 (second part): the delivered momentum against the record
        let dif = [
            tw.delivered[0] - tw.exchanged[0],
            tw.delivered[1] - tw.exchanged[1],
            tw.delivered[2] - tw.exchanged[2],
        ];
        let ex = vnorm(tw.exchanged);
        if ex > 0.0 {
            let r = vnorm(dif) / ex;
            w2b = w2b.max(r);
            assert!(r <= 1e-12, "C2 delivered step {steps}: {r:e}");
        }
        // C1, C2, C3: the identities
        if !injected_only || sim.particle_run().unwrap().tracker.injected() > 0 {
            let id = ident(amended, &sim, &vol, &t0, Some(&y0), true);
            wm = wm.max(id.rm);
            wp = wp.max(id.rp);
            we = we.max(id.re);
            assert!(id.rm <= 1e-10, "C1 IM step {steps}: {:e}", id.rm);
            assert!(id.rp <= 1e-10, "C2 IP step {steps}: {:e}", id.rp);
            assert!(id.re <= 1e-10, "C3 IE step {steps}: {:e}", id.re);
        }
        for c in 0..vol.len() {
            y_min = y_min.min(y[c]);
            y_max = y_max.max(y[c]);
            t_min = t_min.min(t[c]);
        }
    }
    let pr = sim.particle_run().unwrap();
    let tw = pr.coupling().unwrap();
    let led = tw.droplets.as_ref().unwrap();
    let id = ident(amended, &sim, &vol, &t0, Some(&y0), true);
    // SR: the split of the energy source, and the vapour given to the species
    let sr_e =
        (tw.delivered_heat + led.mechanical + led.vapour_held - led.energy_record).abs() / led.energy_record.abs();
    let sr_m = (led.vapour_delivered - led.vapour_record).abs() / led.vapour_record.abs();
    let events = pr.tracker.breakup().expect("breakup").events();
    let counts: Vec<usize> = [
        State::Active,
        State::Deposited,
        State::Escaped,
        State::Stuck,
        State::Film,
        State::Evaporated,
    ]
    .iter()
    .map(|s| pr.tracker.count(*s))
    .collect();
    let hv_ratio = led.vapour_held / id.e_terms[7];
    eprintln!(
        "CB: {steps} steps, {} parcels, states active/deposited/escaped/stuck/film/evaporated {counts:?}, breakup events {events}",
        pr.tracker.injected()
    );
    eprintln!(
        "CB worst (amended {amended}): C1 IM {wm:.3e}, C1b {w1b:.3e}, C2 IP {wp:.3e}, C2 delivered {w2b:.3e}, C3 IE {we:.3e}, C3b {w3b:.3e} (limits 1e-10, 1e-8 [A2.2: error over the bound, 1], 1e-10, 1e-12, 1e-10, 1e-8 [A2.2: 1])"
    );
    eprintln!("CB SR energy split {sr_e:.3e}, vapour {sr_m:.3e} (limits 1e-12)");
    eprintln!(
        "CB end: IM terms [liquid, carrier vapour, injected] {:?} kg; IP terms [liquid, delivered, gravity, wall, breakup, injected] {:?} N s; IE terms [liquid, carrier heat, H_v, K, gravity, wall, breakup, injected] {:?} J",
        id.m_terms, id.p_terms, id.e_terms
    );
    eprintln!(
        "CB end: vapour record {:.6e} kg, H_v/E_inj {hv_ratio:.4e}, heat_raw {:.6e} J, delivered heat {:.6e} J, Y in [{y_min:.3e}, {y_max:.3e}], T_min {t_min:.4} K",
        led.vapour_record, led.heat_raw, tw.delivered_heat
    );
    eprintln!(
        "CB negative controls: NC1 IE without H_v {:.4e} (>= 1e-3), NC2 IM without the carrier vapour {:.4e} (>= 1e-3), NC3 IE with energy_heat alone {:.4e} (>= 1e-3)",
        id.nc[0], id.nc[1], id.nc[2]
    );
    assert!(sr_e <= 1e-12, "SR energy {sr_e:e}");
    assert!(sr_m <= 1e-12, "SR vapour {sr_m:e}");
    assert!(events >= 1, "CV breakup events {events}");
    assert!(led.vapour_record > 0.0, "CV vapour");
    assert!(hv_ratio >= 1e-3, "CV H_v/E_inj {hv_ratio:e}");
    assert!(id.nc[0] >= 1e-3, "NC1 {:e}", id.nc[0]);
    assert!(id.nc[1] >= 1e-3, "NC2 {:e}", id.nc[1]);
    assert!(id.nc[2] >= 1e-3, "NC3 {:e}", id.nc[2]);
}

/// C4: CB with pimple (two outer iterations), 100 steps, as amended (A2.1, A3.1).
#[test]
fn c4_pimple_closed_box_a3() {
    c4_run(true, true);
}

/// The run of amendment 2, kept as a record (ADR 0348 amendment 3): "C4 IE step 1: 1e0".
#[test]
#[ignore = "SETUP DEFECT record (ADR 0348 amendment 3): C4 IE step 1 = 1, the round-off of the carrier inventory over itself"]
fn c4_pimple_closed_box_setup_defect2() {
    c4_run(true, false);
}

/// The first run of C4, kept as a record (ADR 0348 amendment 2): "C4 IM step 1: NaN" (no parcel yet, 0/0).
#[test]
#[ignore = "SETUP DEFECT record (ADR 0348 amendment 2): C4 IM step 1 NaN, all terms zero before the first injection"]
fn c4_pimple_closed_box_setup_defect() {
    c4_run(false, false);
}

fn c4_run(amended: bool, injected_only: bool) {
    let mut sim = build(&text(PIMPLE, true, true, &format!("{EVAP}{BREAKUP}")));
    let vol = sim.geom.mesh.cell_volumes().to_vec();
    let t0 = sim.field("T").unwrap();
    let y0 = sim.field("vap").unwrap();
    let (mut wm, mut wp, mut we) = (0.0f64, 0.0f64, 0.0f64);
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
        assert!(sim.failure().is_none(), "step {steps}: {:?}", sim.failure());
        if injected_only && sim.particle_run().unwrap().tracker.injected() == 0 {
            continue;
        }
        let id = ident(amended, &sim, &vol, &t0, Some(&y0), true);
        wm = wm.max(id.rm);
        wp = wp.max(id.rp);
        we = we.max(id.re);
        assert!(id.rm <= 1e-10, "C4 IM step {steps}: {:e}", id.rm);
        assert!(id.rp <= 1e-10, "C4 IP step {steps}: {:e}", id.rp);
        assert!(id.re <= 1e-10, "C4 IE step {steps}: {:e}", id.re);
    }
    let led = sim
        .particle_run()
        .unwrap()
        .coupling()
        .unwrap()
        .droplets
        .clone()
        .unwrap();
    eprintln!(
        "C4 pimple: {steps} steps; worst IM {wm:.3e}, IP {wp:.3e}, IE {we:.3e} (limits 1e-10); vapour {:.4e} kg",
        led.vapour_record
    );
    assert_eq!(steps, 100, "C4 steps");
}

/// C5: breakup alone (no evaporation, no heat transfer, no energy, no species), 200 steps, as amended (A2.1).
#[test]
fn c5_breakup_alone_a2() {
    c5_run(true);
}

/// The first run of C5, kept as a record (ADR 0348 amendment 2): "C5 IP step 1: NaN" (no parcel yet, 0/0).
#[test]
#[ignore = "SETUP DEFECT record (ADR 0348 amendment 2): C5 IP step 1 NaN, all terms zero before the first injection"]
fn c5_breakup_alone_setup_defect() {
    c5_run(false);
}

fn c5_run(amended: bool) {
    let t = text(
        &PISO.replace("end_time = 5e-3", "end_time = 2e-3"),
        false,
        false,
        BREAKUP,
    );
    let mut sim = build(&t);
    let vol = sim.geom.mesh.cell_volumes().to_vec();
    let mut wp = 0.0f64;
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
        assert!(sim.failure().is_none(), "step {steps}: {:?}", sim.failure());
        let id = ident(amended, &sim, &vol, &[], None, false);
        wp = wp.max(id.rp);
        assert!(id.rp <= 1e-10, "C5 IP step {steps}: {:e}", id.rp);
    }
    let pr = sim.particle_run().unwrap();
    let events = pr.tracker.breakup().unwrap().events();
    let led = pr.coupling().unwrap().droplets.clone().unwrap();
    eprintln!(
        "C5 breakup alone: {steps} steps, worst IP {wp:.3e} (limit 1e-10), breakup events {events}, vapour {:e}",
        led.vapour_record
    );
    assert!(events >= 1, "C5 events {events}");
    assert_eq!(led.vapour_record, 0.0, "C5 vapour");
}

fn bits(sim: &Simulation) -> Vec<u64> {
    let mut v = Vec::new();
    for u in &sim.velocity().values {
        v.extend(u.iter().map(|x| x.to_bits()));
    }
    v.extend(sim.pressure().values.iter().map(|x| x.to_bits()));
    v.extend(sim.flux().iter().map(|x| x.to_bits()));
    v.extend(sim.field("T").unwrap().iter().map(|x| x.to_bits()));
    v
}

/// Z1: the heated two-way path of main on the closed box without the tables, against the same with the two tables written
/// with `enabled = false`.
#[test]
fn z1_disabled_tables_are_bit_identical() {
    let solver = PISO.replace("end_time = 5e-3", "end_time = 1e-3");
    let plain = text(&solver, true, false, "");
    let off = text(
        &solver,
        true,
        false,
        &format!(
            "{}{}",
            EVAP.replace("enabled = true", "enabled = false"),
            BREAKUP.replace("enabled = true", "enabled = false")
        ),
    );
    let mut a = build(&plain);
    let mut b = build(&off);
    let mut steps = 0;
    while !a.is_done() {
        a.step();
        b.step();
        steps += 1;
        assert!(a.failure().is_none(), "{:?}", a.failure());
        assert_eq!(bits(&a), bits(&b), "Z1 step {steps}");
    }
    let (pa, pb) = (a.particle_run().unwrap(), b.particle_run().unwrap());
    assert!(pa.coupling().unwrap().droplets.is_none() && pb.coupling().unwrap().droplets.is_none());
    assert_eq!(pa.tracker.fingerprint(), pb.tracker.fingerprint(), "Z1 fingerprint");
    let moved = a.velocity().values.iter().any(|u| u.iter().any(|x| *x != 0.0));
    eprintln!(
        "Z1: {steps} steps equal bit for bit; {} parcels; the gas moved: {moved}",
        pa.tracker.injected()
    );
    assert_eq!(steps, 100, "Z1 steps");
}

/// Z2a (amendment 1): the tables are absent from the texts of a case that does not write them.
#[test]
fn z2a_tables_absent_from_case_texts() {
    let without = parse(&text(PISO, true, true, ""));
    let with = parse(&cb_text());
    let has_table = |c: &Case| {
        let toml = c.to_toml();
        let json: serde_json::Value = serde_json::from_str(&c.to_json_string()).unwrap();
        let p = &json["particles"];
        (
            toml.lines().any(|l| l.trim() == "[particles.evaporation]"),
            toml.lines().any(|l| l.trim() == "[particles.breakup]"),
            p.get("evaporation").is_some(),
            p.get("breakup").is_some(),
        )
    };
    let (a, b) = (has_table(&without), has_table(&with));
    eprintln!(
        "Z2a: without {a:?}, with {b:?}; hashes {:016x} {:016x}",
        without.tree.content_hash(),
        with.tree.content_hash()
    );
    assert_eq!(a, (false, false, false, false), "Z2a without");
    assert_eq!(b, (true, true, true, true), "Z2a with");
    assert_ne!(without.tree.content_hash(), with.tree.content_hash(), "Z2a hash");
}

fn has(e: &[Diagnostic], path: &str) -> bool {
    e.iter().any(|d| d.path == path)
}

fn table_diags(e: &[Diagnostic]) -> Vec<&Diagnostic> {
    e.iter()
        .filter(|d| d.path.starts_with("particles.evaporation") || d.path.starts_with("particles.breakup"))
        .collect()
}

/// S: the located errors S1 to S7 of ADR 0348 D2.
#[test]
fn s_located_errors() {
    let base = cb_text();
    let e = errors(&base);
    assert!(e.is_empty(), "the valid CB: {e:?}");
    let ev = "particles.evaporation.enabled";
    let bu = "particles.breakup.enabled";
    // (text, path, which table to disable for the inert control)
    let cases: Vec<(String, &str, &str)> = vec![
        (base.replace("two_way = true\n", ""), ev, "both"),
        (base.replace("two_way = true\n", ""), bu, "both"),
        (
            base.replace("algorithm = \"piso\"", "algorithm = \"simplec\""),
            ev,
            "both",
        ),
        (
            base.replace("algorithm = \"piso\"", "algorithm = \"simplec\""),
            bu,
            "both",
        ),
        (base.replace("heat_transfer = true\n", ""), ev, "evap"),
        (
            base.replace("species = \"vap\"", "species = \"\""),
            "particles.evaporation.species",
            "evap",
        ),
        (
            base.replace("species = \"vap\"", "species = \"steam\""),
            "particles.evaporation.species",
            "evap",
        ),
        (
            base.replace("species = \"vap\"", "species = \"air\""),
            "particles.evaporation.species",
            "evap",
        ),
        (
            base.replace("latent_heat = 3.2e5", "latent_heat = -1.0"),
            "particles.evaporation.latent_heat",
            "evap",
        ),
        (
            base.replace("vapour_cp = 2500.0", "vapour_cp = 0.0"),
            "particles.evaporation.vapour_cp",
            "evap",
        ),
        (
            base.replace("max_step_fraction = 0.1", "max_step_fraction = 1.5"),
            "particles.evaporation.max_step_fraction",
            "evap",
        ),
        (
            base.replace("min_diameter = 1e-7", "min_diameter = 0.0"),
            "particles.evaporation.min_diameter",
            "evap",
        ),
        (
            base.replace("surface_tension = 0.01", "surface_tension = -1.0"),
            "particles.breakup.surface_tension",
            "bu",
        ),
        (
            base.replace("liquid_viscosity = 4e-4", "liquid_viscosity = -1.0"),
            "particles.breakup.liquid_viscosity",
            "bu",
        ),
        (
            base.replace("seed = 31", "seed = 31\nk = 0.5"),
            "particles.breakup",
            "bu",
        ),
        (
            base.replacen("enabled = true\ndensity", "enabled = false\ndensity", 1),
            ev,
            "both",
        ),
        (
            base.replacen("enabled = true\ndensity", "enabled = false\ndensity", 1),
            bu,
            "both",
        ),
    ];
    for (t, path, which) in &cases {
        let e = errors(t);
        assert!(has(&e, path), "S: {path} expected, got {e:?}");
        let mut off = t.clone();
        if *which == "evap" || *which == "both" {
            off = off.replace(
                "[particles.evaporation]\nenabled = true",
                "[particles.evaporation]\nenabled = false",
            );
        }
        if *which == "bu" || *which == "both" {
            off = off.replace(
                "[particles.breakup]\nenabled = true",
                "[particles.breakup]\nenabled = false",
            );
        }
        let e = errors(&off);
        let left = table_diags(&e);
        assert!(left.is_empty(), "S inert {path}: {left:?}");
    }
    eprintln!(
        "S: {} bad texts located; each inert with its table disabled",
        cases.len()
    );
}
