//! F-257 checks of ADR 0213: prism layers on ADR 0090's piston (P0 to P7, the negative control N1), layering in the
//! incompressible PIMPLE path on ADR 0033's piston channel G5b mirrored in x (G5L: I1 to I6, the negative control N2),
//! layer additions in that path (G5X: X1 to X5) and `PimpleSolver::replace_mesh` (S1, S2). The limits are the
//! registered ones.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::compressible::{CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, StepInfo};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::layering::{remap, remap_face_flux, ExtrudedStack, LayerChange, LayerEvent, LayerStack, Section};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// P: prism layers on ADR 0090's piston (acceptance B1)
// ---------------------------------------------------------------------------------------------------------------

const T: f64 = 16.0;

/// The two stacks behind one piston loop.
trait Stack {
    fn set(&mut self, x: f64);
    fn event(&mut self) -> Option<LayerChange>;
    fn build(&self) -> Mesh;
    fn layers(&self) -> usize;
}

impl Stack for LayerStack {
    fn set(&mut self, x: f64) {
        self.set_piston(x).unwrap();
    }
    fn event(&mut self) -> Option<LayerChange> {
        self.adjust()
    }
    fn build(&self) -> Mesh {
        self.mesh().unwrap()
    }
    fn layers(&self) -> usize {
        self.n_layers()
    }
}

impl Stack for ExtrudedStack {
    fn set(&mut self, x: f64) {
        self.set_piston(x).unwrap();
    }
    fn event(&mut self) -> Option<LayerChange> {
        self.adjust()
    }
    fn build(&self) -> Mesh {
        self.mesh().unwrap()
    }
    fn layers(&self) -> usize {
        self.n_layers()
    }
}

fn geom<S: Stack>(s: &S) -> FvGeometry {
    FvGeometry::new(Arc::new(s.build())).unwrap()
}

fn rest() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    }
}

fn piston_x(t: f64) -> f64 {
    1.0 - 0.3 * (1.0 - (PI * t / T).cos())
}

/// `[sum V, sum rho V, sum rho E V]` of the solver state.
fn totals(s: &CompressibleSolver) -> [f64; 3] {
    let t = s.totals();
    [s.geom.mesh.cell_volumes().iter().sum(), t[0], t[4]]
}

/// Volume-averaged pressure and total volume.
fn mean_pressure(s: &CompressibleSolver) -> (f64, f64) {
    let v = s.geom.mesh.cell_volumes();
    let vol: f64 = v.iter().sum();
    let pv: f64 = s.primitives().iter().zip(v).map(|(p, v)| p.p * v).sum();
    (pv / vol, vol)
}

/// Results of one piston run on ADR 0090's loop.
#[derive(Debug)]
struct PistonRun {
    /// P1, P2
    a: [f64; 2],
    /// P3, P4, P5
    event_cons: f64,
    mass: f64,
    volume: f64,
    /// P6
    removed: usize,
    added: usize,
    /// P7
    opposite: usize,
    /// P0: cells per layer and faces per cell held after every step and event
    structure: bool,
    /// N1: largest relative mass change of the first-source remap over the removals
    n1: f64,
    /// p_bar at T
    p_bar_t: f64,
    steps: usize,
}

/// ADR 0090's piston loop to `2T` on `stack` with `per_layer` cells per layer and `faces` faces per cell.
fn piston_run<S: Stack>(mut stack: S, per_layer: usize, faces: usize) -> PistonRun {
    let clock = std::time::Instant::now();
    let gamma = Gas::AIR.gamma;
    let mut s = CompressibleSolver::new(geom(&stack), Gas::AIR, CompSettings::default(), |_| rest()).unwrap();
    let (p0, v0) = mean_pressure(&s);
    let inv0 = p0 * v0.powf(gamma);
    let m0 = s.totals()[0];
    let shape = |s: &CompressibleSolver, layers: usize| {
        let m = &s.geom.mesh;
        m.n_cells() == per_layer * layers && (0..m.n_cells()).all(|c| m.cell_faces(c).len() == faces)
    };
    let mut r = PistonRun {
        a: [f64::NAN; 2],
        event_cons: 0.0,
        mass: 0.0,
        volume: 0.0,
        removed: 0,
        added: 0,
        opposite: 0,
        structure: shape(&s, stack.layers()),
        n1: 0.0,
        p_bar_t: f64::NAN,
        steps: 0,
    };
    let (mut t, mut diag) = (0.0, 0.0f64);
    let mut prev: Option<LayerEvent> = None;
    for (stroke, target) in [T, 2.0 * T].into_iter().enumerate() {
        while t < target {
            let mut dt = s.max_dt();
            let last = t + dt >= target - 1e-9;
            if last {
                dt = target - t;
            }
            let t_new = if last { target } else { t + dt };
            stack.set(piston_x(t_new));
            s.step_ale(geom(&stack), dt).unwrap();
            t = t_new;
            r.steps += 1;
            r.mass = r.mass.max((s.totals()[0] / m0 - 1.0).abs());
            let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
            r.volume = r.volume.max((vol / (0.01 * piston_x(t)) - 1.0).abs());
            let (pm, vm) = mean_pressure(&s);
            diag = diag.max((pm * vm.powf(gamma) / inv0 - 1.0).abs());
            r.structure &= shape(&s, stack.layers());
            let event = match stack.event() {
                Some(ch) => {
                    let before = totals(&s);
                    let g = geom(&stack);
                    if ch.event == LayerEvent::Removed {
                        // N1: the merged cell takes the density of its first source
                        let vn = g.mesh.cell_volumes();
                        let bad: f64 = ch.sources.iter().zip(vn).map(|(src, v)| s.u[src[0].0][0] * v).sum();
                        r.n1 = r.n1.max((bad / before[1] - 1.0).abs());
                    }
                    let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
                    s.replace_mesh(g, u).unwrap();
                    let after = totals(&s);
                    for k in 0..3 {
                        r.event_cons = r.event_cons.max((after[k] / before[k] - 1.0).abs());
                    }
                    match ch.event {
                        LayerEvent::Removed => r.removed += 1,
                        LayerEvent::Added => r.added += 1,
                    }
                    r.structure &= shape(&s, stack.layers());
                    Some(ch.event)
                }
                None => None,
            };
            if let (Some(e), Some(p)) = (event, prev) {
                if e != p {
                    r.opposite += 1;
                }
            }
            prev = event;
        }
        let (pm, vm) = mean_pressure(&s);
        r.a[stroke] = pm * vm.powf(gamma) / inv0 - 1.0;
        if stroke == 0 {
            r.p_bar_t = pm;
            let q: Vec<f64> = s.primitives().iter().map(|p| p.p / p.rho.powf(gamma)).collect();
            let (lo, hi) = q
                .iter()
                .fold((f64::INFINITY, 0.0f64), |(l, h), &x| (l.min(x), h.max(x)));
            println!(
                "  t = T: V/V0 = {:.6}, layers {}, p_bar = {pm:.6} (isentrope {:.6}), p/rho^gamma in [{lo:.6}, {hi:.6}]",
                vm / v0,
                stack.layers(),
                inv0 / vm.powf(gamma)
            );
        }
    }
    println!(
        "  largest |p_bar V^gamma / (p0 V0^gamma) - 1| over the steps {diag:.3e}; {} steps, {} layers at the end, {:.1} s",
        r.steps,
        stack.layers(),
        clock.elapsed().as_secs_f64()
    );
    r
}

#[test]
fn p_prism_layers_on_the_adr0090_piston() {
    println!("P-Δ (prisms):");
    let stack =
        ExtrudedStack::uniform(Section::rect_triangles([2, 2], [0.1, 0.1]).unwrap(), 40, 1.0, 0.4, 1.6).unwrap();
    let r = piston_run(stack, 8, 5);
    println!("ADR 0090's P (hexahedra), printed for comparison:");
    let hex = piston_run(
        LayerStack::uniform(40, 1.0, [2, 2], [0.1, 0.1], 0.4, 1.6).unwrap(),
        4,
        6,
    );
    println!(
        "P0 structure (8 cells per layer, 5 faces each) after every step and event: {}",
        r.structure
    );
    println!(
        "P1 p_bar V^gamma / (p0 V0^gamma) - 1 at t = T: {:.3e} (limit 2e-2)",
        r.a[0]
    );
    println!("P2 the same at t = 2T: {:.3e} (limit 2e-2)", r.a[1]);
    println!(
        "P3 max relative change of (V, mass, energy) over an event: {:.2e} (limit 1e-13)",
        r.event_cons
    );
    println!("P4 max relative mass drift: {:.2e} (limit 1e-12)", r.mass);
    println!(
        "P5 max relative volume error against 0.01 x_p: {:.2e} (limit 1e-13)",
        r.volume
    );
    println!("P6 removals {}, additions {} (each >= 5)", r.removed, r.added);
    println!("P7 opposite events in consecutive steps: {}", r.opposite);
    println!(
        "N1 first-source remap: largest relative mass change over the removals {:.3e} (>= 1e-10)",
        r.n1
    );
    println!(
        "printed: p_bar at T prisms {:.9}, hexahedra {:.9}, difference {:.3e}; hexahedra P1 {:.3e}, P2 {:.3e}, {} steps",
        r.p_bar_t,
        hex.p_bar_t,
        r.p_bar_t - hex.p_bar_t,
        hex.a[0],
        hex.a[1],
        hex.steps
    );
    assert!(r.structure, "P0");
    assert!(r.a[0].abs() <= 2e-2, "P1 {}", r.a[0]);
    assert!(r.a[1].abs() <= 2e-2, "P2 {}", r.a[1]);
    assert!(r.event_cons <= 1e-13, "P3 {}", r.event_cons);
    assert!(r.mass <= 1e-12, "P4 {}", r.mass);
    assert!(r.volume <= 1e-13, "P5 {}", r.volume);
    assert!(r.removed >= 5 && r.added >= 5, "P6 {} {}", r.removed, r.added);
    assert_eq!(r.opposite, 0, "P7");
    assert!(r.n1 >= 1e-10, "N1 {}", r.n1);
}

// ---------------------------------------------------------------------------------------------------------------
// I, X, S: layering in the incompressible PIMPLE path (acceptance B2)
// ---------------------------------------------------------------------------------------------------------------

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};
const A0: f64 = 2.0;
const DT: f64 = 0.005;

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.01);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

/// Net outward flux of every cell divided by the largest face area (the measure of `StepInfo::continuity`).
fn continuity(m: &Mesh, phi: &[f64]) -> f64 {
    let mut d = vec![0.0; m.n_cells()];
    for (f, &o) in m.owner().iter().enumerate() {
        d[o] += phi[f];
    }
    for (f, &nb) in m.neighbour().iter().enumerate() {
        d[nb] -= phi[f];
    }
    let s = m.face_area_vectors().iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
    d.iter().fold(0.0f64, |a, b| a.max(b.abs())) / s
}

/// What one layer event did to the channel.
struct EventRecord {
    kind: LayerEvent,
    /// relative change of `sum V`
    volume: f64,
    /// relative change of `sum V u` (largest component, against `|sum V u|` before)
    momentum: f64,
    /// continuity of the remapped flux on the new mesh
    continuity: f64,
    /// the `xmin` fluxes kept their bits
    outlet_kept: bool,
}

/// G5L (`sign = -1`, the piston pushed in) or G5X (`sign = +1`, pulled out): ADR 0033 G5b mirrored in x on a layer
/// stack.
struct Channel {
    stack: ExtrudedStack,
    solver: PimpleSolver,
    sign: f64,
    t: f64,
}

impl Channel {
    fn new(sign: f64) -> Self {
        let stack = ExtrudedStack::uniform(Section::rect([2, 2], [0.2, 0.2]).unwrap(), 20, 1.0, 0.4, 1.6).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(stack.mesh().unwrap())).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(0.0))).unwrap();
        Channel {
            stack,
            solver: PimpleSolver::new(geom, u, p, settings()),
            sign,
            t: 0.0,
        }
    }

    fn piston(&self, t: f64) -> f64 {
        1.0 + self.sign * 0.5 * A0 * t * t
    }

    /// One step: the piston's swept flux and the step record.
    fn step(&mut self) -> (f64, StepInfo) {
        let t1 = self.t + DT;
        self.stack.set_piston(self.piston(t1)).unwrap();
        let st = move_mesh(&self.solver.geom, self.stack.points(), DT).unwrap();
        let old = self.solver.geom.mesh.clone();
        let walls = set_follow_conditions(&mut self.solver.u, &st, &old, &["xmax".to_string()], DT).unwrap();
        let m_p: f64 = st
            .geom
            .mesh
            .patch("xmax")
            .unwrap()
            .faces()
            .map(|f| st.mesh_flux[f])
            .sum();
        let info = self
            .solver
            .step_ale_with_walls(DT, st.geom, st.mesh_flux, walls)
            .unwrap();
        self.t = t1;
        (m_p, info)
    }

    fn outlet(&self) -> Vec<f64> {
        let g = &self.solver.geom;
        g.mesh
            .patch("xmin")
            .unwrap()
            .faces()
            .map(|f| self.solver.phi[f])
            .collect()
    }

    /// The layering check after a step: on an event the new mesh, the remapped state and `replace_mesh`.
    fn layer_event(&mut self) -> Option<EventRecord> {
        let change = self.stack.adjust()?;
        let old = self.solver.geom.clone();
        let mesh = self.stack.mesh().unwrap();
        let sources = self.stack.face_sources(&change, &old.mesh, &mesh).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let (vo, vn) = (old.mesh.cell_volumes(), geom.mesh.cell_volumes());
        let u = remap(&change, &self.solver.u.values, vo, vn);
        let p1: Vec<[f64; 1]> = self.solver.p.values.iter().map(|x| [*x]).collect();
        let p: Vec<f64> = remap(&change, &p1, vo, vn).into_iter().map(|x| x[0]).collect();
        let phi = remap_face_flux(&sources, &self.solver.phi, &geom.mesh);
        let mom = |u: &[Vec3], v: &[f64]| -> Vec3 {
            u.iter()
                .zip(v)
                .fold(vec3::ZERO, |a, (u, v)| vec3::add(a, vec3::scale(*u, *v)))
        };
        let (m_old, m_new) = (mom(&self.solver.u.values, vo), mom(&u, vn));
        let m_scale = vec3::norm(m_old).max(1e-300);
        let (s_old, s_new): (f64, f64) = (vo.iter().sum(), vn.iter().sum());
        let outlet_old = self.outlet();
        let record_cont = continuity(&geom.mesh, &phi);
        let outlet_new: Vec<f64> = geom.mesh.patch("xmin").unwrap().faces().map(|f| phi[f]).collect();
        let rec = EventRecord {
            kind: change.event,
            volume: (s_new / s_old - 1.0).abs(),
            momentum: (0..3)
                .map(|k| (m_new[k] - m_old[k]).abs() / m_scale)
                .fold(0.0, f64::max),
            continuity: record_cont,
            outlet_kept: outlet_old.len() == outlet_new.len()
                && outlet_old
                    .iter()
                    .zip(&outlet_new)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
        };
        self.solver.replace_mesh(geom, u, p, phi).unwrap();
        Some(rec)
    }
}

/// Results of a channel run.
struct ChannelRun {
    continuity: f64,
    flux: f64,
    /// N2: smallest one-step-late flux difference from the second step on
    late: f64,
    events: Vec<EventRecord>,
    u_err: f64,
    transverse: f64,
    p_err: f64,
}

fn channel_run(sign: f64, steps: usize) -> ChannelRun {
    let mut ch = Channel::new(sign);
    let mut r = ChannelRun {
        continuity: 0.0,
        flux: 0.0,
        late: f64::INFINITY,
        events: Vec::new(),
        u_err: 0.0,
        transverse: 0.0,
        p_err: 0.0,
    };
    let mut prev_m: Option<f64> = None;
    for _ in 0..steps {
        let t0 = ch.t;
        let (m_p, info) = ch.step();
        r.continuity = r.continuity.max(info.continuity);
        let q_out: f64 = ch.outlet().iter().sum();
        r.flux = r.flux.max((q_out + m_p).abs() / m_p.abs());
        if let Some(m) = prev_m {
            r.late = r.late.min((q_out + m).abs() / m_p.abs());
        }
        prev_m = Some(m_p);
        let v_mean = sign * A0 * (t0 + 0.5 * DT);
        for u in &ch.solver.u.values {
            r.u_err = r.u_err.max((u[0] - v_mean).abs());
            r.transverse = r.transverse.max(u[1].abs()).max(u[2].abs());
        }
        if let Some(e) = ch.layer_event() {
            r.events.push(e);
        }
    }
    // the lab-frame plug flow with the outlet at x = 0: p = -sign a0 x
    let xc = ch.solver.geom.mesh.cell_centres();
    r.p_err = ch
        .solver
        .p
        .values
        .iter()
        .zip(xc)
        .map(|(p, c)| (p + sign * A0 * c[0]).abs())
        .fold(0.0, f64::max);
    println!(
        "  {} steps to t = {:.3}, {} layers at the end; printed: G5 (iii) u_x error {:.3e}, transverse {:.2e}, G5 (v) pressure error {:.3e} (G5's limit {:.3e})",
        steps,
        ch.t,
        ch.stack.n_layers(),
        r.u_err,
        r.transverse,
        r.p_err,
        2e-2 * A0
    );
    r
}

fn worst(events: &[EventRecord], f: impl Fn(&EventRecord) -> f64) -> f64 {
    events.iter().map(f).fold(0.0, f64::max)
}

#[test]
fn i_layering_in_the_incompressible_piston_channel() {
    println!("G5L (piston pushed in):");
    let r = channel_run(-1.0, 120);
    let removed = r.events.iter().filter(|e| e.kind == LayerEvent::Removed).count();
    let added = r.events.len() - removed;
    let (vol, mom, cont) = (
        worst(&r.events, |e| e.volume),
        worst(&r.events, |e| e.momentum),
        worst(&r.events, |e| e.continuity),
    );
    let kept = r.events.iter().all(|e| e.outlet_kept);
    println!("I1 max step continuity {:.2e} (limit 1e-9)", r.continuity);
    println!("I2 max |Q_out + m_p| / |m_p| {:.2e} (limit 1e-10)", r.flux);
    println!("I3 max relative volume change over an event {vol:.2e} (limit 1e-13)");
    println!("I4 removals {removed} (>= 5), additions {added}");
    println!("I5 max continuity of the remapped flux {cont:.2e} (limit 1e-9); outlet fluxes kept bit for bit: {kept}");
    println!("I6 max relative momentum change over an event {mom:.2e} (limit 1e-13)");
    println!(
        "N2 smallest |Q_out + m_p(previous step)| / |m_p| from the second step {:.3e} (>= 1e-3)",
        r.late
    );
    assert!(r.continuity <= 1e-9, "I1 {}", r.continuity);
    assert!(r.flux <= 1e-10, "I2 {}", r.flux);
    assert!(vol <= 1e-13, "I3 {vol}");
    assert!(removed >= 5, "I4 {removed}");
    assert!(cont <= 1e-9 && kept, "I5 {cont} {kept}");
    assert!(mom <= 1e-13, "I6 {mom}");
    assert!(r.late >= 1e-3, "N2 {}", r.late);
}

#[test]
fn x_layer_additions_in_the_incompressible_path() {
    println!("G5X (piston pulled out):");
    let r = channel_run(1.0, 120);
    let added = r.events.iter().filter(|e| e.kind == LayerEvent::Added).count();
    let (vol, cont) = (worst(&r.events, |e| e.volume), worst(&r.events, |e| e.continuity));
    println!("X1 max step continuity {:.2e} (limit 1e-9)", r.continuity);
    println!("X2 max |Q_out + m_p| / |m_p| {:.2e} (limit 1e-10)", r.flux);
    println!("X3 max relative volume change over an event {vol:.2e} (limit 1e-13)");
    println!("X4 additions {added} (>= 5), removals {}", r.events.len() - added);
    println!("X5 max continuity of the remapped flux {cont:.2e} (limit 1e-9)");
    assert!(r.continuity <= 1e-9, "X1 {}", r.continuity);
    assert!(r.flux <= 1e-10, "X2 {}", r.flux);
    assert!(vol <= 1e-13, "X3 {vol}");
    assert!(added >= 5, "X4 {added}");
    assert!(cont <= 1e-9, "X5 {cont}");
}

fn bits(s: &PimpleSolver) -> Vec<u64> {
    let mut out: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    out.extend(s.p.values.iter().map(|x| x.to_bits()));
    out.extend(s.phi.iter().map(|x| x.to_bits()));
    out
}

#[test]
fn s1_replace_mesh_with_the_same_mesh_is_bit_identical() {
    let mut a = Channel::new(-1.0);
    for _ in 0..6 {
        a.step();
    }
    let mut b = Channel::new(-1.0);
    for _ in 0..3 {
        b.step();
    }
    let s = &mut b.solver;
    s.replace_mesh(s.geom.clone(), s.u.values.clone(), s.p.values.clone(), s.phi.clone())
        .unwrap();
    for _ in 0..3 {
        b.step();
    }
    let same = bits(&a.solver) == bits(&b.solver);
    println!("S1 six steps against three, replace_mesh, three: bit for bit {same}");
    assert!(same, "S1");
}

#[test]
fn s2_replace_mesh_refusals() {
    let mut ch = Channel::new(-1.0);
    ch.step();
    ch.step();
    let s = &mut ch.solver;
    let before = bits(s);
    let (g, u, p, phi) = (s.geom.clone(), s.u.values.clone(), s.p.values.clone(), s.phi.clone());
    let short = s.replace_mesh(g.clone(), u[..u.len() - 1].to_vec(), p.clone(), phi.clone());
    let other =
        Arc::new(FvGeometry::new(Arc::new(rcfd_mesh::gen::box_2d(4, 4, 1.0, 1.0, false, false).unwrap())).unwrap());
    let n = other.n_cells();
    let patches = s.replace_mesh(
        other.clone(),
        vec![vec3::ZERO; n],
        vec![0.0; n],
        vec![0.0; other.mesh.n_faces()],
    );
    s.settings.time_scheme = TimeScheme::Bdf2;
    let bdf2 = s.replace_mesh(g.clone(), u.clone(), p.clone(), phi.clone());
    s.settings.time_scheme = TimeScheme::ImplicitEuler;
    s.nu_face = Some(vec![0.01; g.mesh.n_faces()]);
    let nu = s.replace_mesh(g.clone(), u.clone(), p.clone(), phi.clone());
    s.nu_face = None;
    let unchanged = bits(s) == before;
    for (name, r) in [
        ("short", &short),
        ("patches", &patches),
        ("bdf2", &bdf2),
        ("nu_face", &nu),
    ] {
        println!("S2 {name}: {r:?}");
    }
    println!("S2 state unchanged: {unchanged}");
    assert!(
        short.is_err() && patches.is_err() && bdf2.is_err() && nu.is_err(),
        "S2 refusals"
    );
    assert!(unchanged, "S2 unchanged");
}
