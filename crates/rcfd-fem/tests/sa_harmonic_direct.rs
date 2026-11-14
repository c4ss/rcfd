//! F-412 (ADR 0448): the direct harmonic solve. Checks H1 to H9, S1 (as amendment 1), SW1 and the refusals R1 to R11 of
//! the registration.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicOptions, HarmonicResponse, HarmonicSolver, ModalBasis, ModalDamping};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor, ModulusTable};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
use std::f64::consts::PI;

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const B: f64 = 0.04;
const A: f64 = B * B;
const L: f64 = 1.0;
const ETA: f64 = 0.02;
/// The registered mesh of the bar (ADR 0448, from the 1-D chain).
const N: usize = 80;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!("{name}: {value:.6e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
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

// ------------------------------------------------------------------------------------------------ complex functions

fn csin(z: C64) -> C64 {
    C64::new(z.re.sin() * z.im.cosh(), z.re.cos() * z.im.sinh())
}

fn ccos(z: C64) -> C64 {
    C64::new(z.re.cos() * z.im.cosh(), -z.re.sin() * z.im.sinh())
}

fn csqrt(z: C64) -> C64 {
    let r = z.abs();
    let re = ((r + z.re) / 2.0).sqrt();
    let im = ((r - z.re) / 2.0).sqrt().copysign(z.im);
    C64::new(re, im)
}

fn wrap(x: f64) -> f64 {
    let mut y = x % (2.0 * PI);
    if y > PI {
        y -= 2.0 * PI;
    }
    if y <= -PI {
        y += 2.0 * PI;
    }
    y
}

// ------------------------------------------------------------------------------------------------------------ bars

fn c0() -> f64 {
    (E_MOD / RHO).sqrt()
}

fn w1() -> f64 {
    PI * c0() / (2.0 * L)
}

fn steel(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, RHO, 0.0).unwrap()
}

fn bar_with(cells: usize, e: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [L, B, B], [cells, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(e, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn bar() -> SolidModel {
    bar_with(N, E_MOD)
}

fn tip_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, 0).unwrap()] = 0.25;
    }
    HarmonicLoad::real(f).unwrap()
}

fn zero_load(model: &SolidModel) -> HarmonicLoad {
    HarmonicLoad::real(vec![0.0; model.dof_map().n_equations()]).unwrap()
}

fn tip_equation(model: &SolidModel) -> usize {
    let node = model.mesh().node_set("xmax").unwrap()[0];
    model.dof_map().equation(node, 0).unwrap()
}

fn tip(model: &SolidModel, r: &HarmonicResponse) -> C64 {
    let j = tip_equation(model);
    C64::new(r.re[j], r.im[j])
}

fn tip_dashpots(model: &SolidModel, c: f64, mut o: DirectHarmonicOptions) -> DirectHarmonicOptions {
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    o
}

fn rel(a: C64, b: C64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The response at the tip of the 1-D chain of `N` linear elements with consistent mass (the reference of H2): complex
/// modulus `es`, tip dashpot `c`, tip force `f`, base displacement `g` at node 0; solved by the Thomas algorithm.
fn chain_tip(cells: usize, es: C64, w: f64, c: f64, f: f64, g: C64) -> C64 {
    let h = L / cells as f64;
    let kk = es.scale(A / h);
    let mm = RHO * A * h / 6.0;
    let n = cells; // unknowns 1..=cells
    let diag: Vec<C64> = (1..=n)
        .map(|j| {
            if j < n {
                kk.scale(2.0) - C64::from_real(w * w * 4.0 * mm)
            } else {
                kk - C64::from_real(w * w * 2.0 * mm) + C64::new(0.0, w * c)
            }
        })
        .collect();
    let off = -kk - C64::from_real(w * w * mm);
    let mut rhs = vec![C64::ZERO; n];
    rhs[n - 1] += C64::from_real(f);
    rhs[0] -= off * g;
    // forward elimination
    let mut dp = diag.clone();
    let mut rp = rhs.clone();
    for j in 1..n {
        let m = off / dp[j - 1];
        dp[j] = dp[j] - m * off;
        rp[j] = rp[j] - m * rp[j - 1];
    }
    let mut x = vec![C64::ZERO; n];
    x[n - 1] = rp[n - 1] / dp[n - 1];
    for j in (0..n - 1).rev() {
        x[j] = (rp[j] - off * x[j + 1]) / dp[j];
    }
    x[n - 1]
}

fn bits(r: &HarmonicResponse) -> Vec<(u64, u64)> {
    r.re.iter().zip(&r.im).map(|(a, b)| (a.to_bits(), b.to_bits())).collect()
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused<T>(c: &mut Checks, name: &str, r: rcfd_fem::Result<T>, key: &str, contains: &str) {
    match r {
        Ok(_) => c.holds(name, false, "accepted"),
        Err(e) => {
            let m = e.to_string();
            c.holds(name, key_of(&e) == Some(key) && m.contains(contains), &m);
        }
    }
}

// ----------------------------------------------------------------------------------------------------- H1, H2, H7

#[test]
fn h1_h2_h7_loss_factor_bar() {
    let mut c = Checks::new();
    let model = bar();
    let load = tip_load(&model);
    let lossy = DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA));
    let solver = DirectHarmonic::new(&model, &lossy).unwrap();
    let table = ModulusTable::new(vec![0.0, 1e5], vec![E_MOD, E_MOD], vec![ETA, ETA]).unwrap();
    let tabled = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(table)),
    )
    .unwrap();
    let es = C64::new(E_MOD, E_MOD * ETA);
    for (tag, r) in [("a", 0.5), ("b", 1.0), ("c", 1.5)] {
        let w = r * w1();
        let resp = solver.response(&load, w).unwrap();
        let xh = tip(&model, &resp);
        let ks = csqrt(C64::from_real(RHO) / es).scale(w);
        let exact = csin(ks.scale(L)) / (ccos(ks.scale(L)) * es.scale(A) * ks);
        let amp = (xh.abs() - exact.abs()).abs() / exact.abs();
        let dphi = wrap(xh.arg() - exact.arg()).abs();
        println!("H1 {r} w1: tip {xh} exact {exact} (|X| {:e}, arg {:.6})", exact.abs(), exact.arg());
        c.at_most(&format!("H1{tag} amplitude error at {r} w1"), amp, 5e-3);
        c.at_most(&format!("H1 phase error / |arg X| at {r} w1"), dphi / exact.arg().abs(), 5e-3);
        c.at_most(&format!("H1 complex error at {r} w1"), rel(xh, exact), 5e-3);
        println!("H1 (report) phase error at {r} w1: {dphi:e} rad");
        let chain = chain_tip(N, es, w, 0.0, 1.0, C64::ZERO);
        c.at_most(&format!("H2 loss-factor bar against the chain at {r} w1"), rel(xh, chain), 1e-8);
        // H7: the constant table
        let rt = tabled.response(&load, w).unwrap();
        let dmax = rt
            .re
            .iter()
            .zip(&rt.im)
            .zip(resp.re.iter().zip(&resp.im))
            .fold(0.0f64, |m, ((a, b), (p, q))| m.max(C64::new(a - p, b - q).abs()));
        let xmax = (0..resp.re.len()).fold(0.0f64, |m, j| m.max(resp.amplitude(j)));
        c.at_most(&format!("H7 constant table against Constant at {r} w1"), dmax / xmax, 1e-12);
    }
    c.finish();
}

#[test]
fn h7b_table_interpolation() {
    let mut c = Checks::new();
    let model = bar();
    let load = tip_load(&model);
    let w = 0.5 * w1();
    let fq = w / (2.0 * PI);
    let table = ModulusTable::new(vec![0.0, 2.0 * fq], vec![E_MOD, 2.0 * E_MOD], vec![0.0, 2.0 * ETA]).unwrap();
    let rt = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(table)))
        .unwrap()
        .response(&load, w)
        .unwrap();
    let stiff = bar_with(N, 1.5 * E_MOD);
    let rc = DirectHarmonic::new(&stiff, &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)))
        .unwrap()
        .response(&load, w)
        .unwrap();
    let dmax = (0..rc.re.len()).fold(0.0f64, |m, j| m.max(C64::new(rt.re[j] - rc.re[j], rt.im[j] - rc.im[j]).abs()));
    let xmax = (0..rc.re.len()).fold(0.0f64, |m, j| m.max(rc.amplitude(j)));
    c.at_most("H7b table at the midpoint against 1.5 E with Constant(0.02)", dmax / xmax, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- S1, SW1

#[test]
fn s1_sw1_stress_and_sweep() {
    let mut c = Checks::new();
    let model = bar();
    let load = tip_load(&model);
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)))
        .unwrap();
    let w = 0.5 * w1();
    let resp = solver.response(&load, w).unwrap();
    let st = resp.stress(&model).unwrap();
    let es = C64::new(E_MOD, E_MOD * ETA);
    let ks = csqrt(C64::from_real(RHO) / es).scale(w);
    let h = L / N as f64;
    let (mut dmax, mut smax) = (0.0f64, 0.0f64);
    for (p, pos) in st.positions.iter().enumerate() {
        let xc = h * ((pos[0] / h).floor() + 0.5);
        let exact = ccos(ks.scale(xc)) / ccos(ks.scale(L)).scale(A);
        let got = C64::new(st.re[p][0], st.im[p][0]);
        dmax = dmax.max((got - exact).abs());
        smax = smax.max(exact.abs());
    }
    c.at_most("S1 stress at the element centres (amendment 1)", dmax / smax, 1e-3);
    let ws = [0.5 * w1(), w1(), 1.5 * w1()];
    let sweep = solver.sweep(&load, &ws).unwrap();
    let same = ws
        .iter()
        .zip(&sweep)
        .all(|(&w, s)| bits(s) == bits(&solver.response(&load, w).unwrap()));
    c.holds("SW1 sweep bit-equal to response", same, "three frequencies");
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- H3

fn cantilever() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(E_MOD, 0.3)).unwrap();
    for k in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    m
}

fn cantilever_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    for &a in model.mesh().node_set("xmax").unwrap() {
        re[dofs.equation(a, 0).unwrap()] = 0.3;
        re[dofs.equation(a, 1).unwrap()] = 1.0;
        re[dofs.equation(a, 2).unwrap()] = -0.5;
        im[dofs.equation(a, 1).unwrap()] = 0.2;
    }
    HarmonicLoad::new(re, im).unwrap()
}

#[test]
fn h3_rayleigh_against_mode_superposition() {
    let mut c = Checks::new();
    let model = cantilever();
    let load = cantilever_load(&model);
    let basis = ModalBasis::all_modes(&model, Some(4)).unwrap();
    println!("H3 basis of {} modes", basis.len());
    let wb = basis.eigenvalues()[0].sqrt();
    let (alpha, beta) = (0.02 * wb, 0.02 / wb);
    let modal = HarmonicSolver::new(
        &model,
        basis,
        &HarmonicOptions::new(ModalDamping::Rayleigh {
            mass: alpha,
            stiffness: beta,
        })
        .static_correction(false),
    )
    .unwrap();
    let direct = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().rayleigh(alpha, beta)).unwrap();
    for r in [0.5, 1.0, 1.7, 2.5, 4.0] {
        let w = r * wb;
        let xm = modal.response(&load, w).unwrap();
        let xd = direct.response(&load, w).unwrap();
        let dmax = (0..xm.re.len()).fold(0.0f64, |m, j| m.max(C64::new(xd.re[j] - xm.re[j], xd.im[j] - xm.im[j]).abs()));
        let xmax = (0..xm.re.len()).fold(0.0f64, |m, j| m.max(xm.amplitude(j)));
        c.at_most(&format!("H3 direct against all modes at {r} w_1"), dmax / xmax, 1e-8);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ H4 to H6

#[test]
fn h4_h5_h6_dashpots_and_base() {
    let mut c = Checks::new();
    let model = bar();
    let load = tip_load(&model);
    let cm = A * (E_MOD * RHO).sqrt();
    let es = C64::from_real(E_MOD);
    // H4
    let c4 = 0.1 * cm;
    let s4 = DirectHarmonic::new(&model, &tip_dashpots(&model, c4, DirectHarmonicOptions::new())).unwrap();
    for (tag, r) in [("a", 0.5), ("b", 1.0), ("c", 1.5)] {
        let w = r * w1();
        let k = w / c0();
        let exact = C64::ONE / C64::new(E_MOD * A * k / (k * L).tan(), w * c4);
        let xh = tip(&model, &s4.response(&load, w).unwrap());
        c.at_most(&format!("H4{tag} dashpot 0.1 matched at {r} w1"), rel(xh, exact), 5e-3);
        c.at_most(&format!("H2 dashpot bar against the chain at {r} w1"), rel(xh, chain_tip(N, es, w, c4, 1.0, C64::ZERO)), 1e-8);
    }
    // H5
    let s5 = DirectHarmonic::new(&model, &tip_dashpots(&model, cm, DirectHarmonicOptions::new())).unwrap();
    for (tag, r) in [("a", 0.5), ("b", 1.0), ("c", 1.5), ("d", 2.5), ("e", 3.0)] {
        let w = r * w1();
        let k = w / c0();
        let exact = (k * L).sin().abs() / (E_MOD * A * k);
        let xh = tip(&model, &s5.response(&load, w).unwrap());
        c.at_most(&format!("H5{tag} matched dashpot amplitude at {r} w1"), (xh.abs() - exact).abs() / exact, 5e-3);
        let full = C64::ONE / C64::new(E_MOD * A * k / (k * L).tan(), w * cm);
        println!("H5 (report) complex error against F/(EAk cot kL + i w c) at {r} w1: {:e}", rel(xh, full));
        c.at_most(&format!("H2 matched bar against the chain at {r} w1"), rel(xh, chain_tip(N, es, w, cm, 1.0, C64::ZERO)), 1e-8);
    }
    // H6
    let s6 = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let mut base = BaseMotion::new();
    base.add_set(&model, "xmin", 0, C64::ONE).unwrap();
    let w = 0.5 * w1();
    let r6 = s6.response_with_base(&zero_load(&model), &base, w).unwrap();
    let xh = tip(&model, &r6);
    let exact = 1.0 / (PI / 4.0).cos();
    c.at_most("H6 base motion tip amplitude at 0.5 w1", (xh.abs() - exact).abs() / exact, 5e-3);
    c.at_most("H2 base bar against the chain", rel(xh, chain_tip(N, es, w, 0.0, 0.0, C64::ONE)), 1e-8);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- H8

#[test]
fn h8_undamped_resonance_refused() {
    let mut c = Checks::new();
    let model = bar_with(10, E_MOD);
    let load = tip_load(&model);
    let basis = ModalBasis::all_modes(&model, Some(4)).unwrap();
    let w = basis.eigenvalues()[0].sqrt();
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    refused(&mut c, "H8 undamped solve at the resonance", solver.response(&load, w), "frequency", &format!("{w:e}"));
    match solver.response(&load, w * (1.0 + 1e-3)) {
        Ok(r) => c.holds(
            "H8n control at 1.001 w_1 accepted",
            r.re.iter().chain(&r.im).all(|v| v.is_finite()),
            &format!("tip amplitude {:e}", r.amplitude(tip_equation(&model))),
        ),
        Err(e) => c.holds("H8n control at 1.001 w_1 accepted", false, &e.to_string()),
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- H9

#[test]
fn h9_threads_bit_identical() {
    let mut c = Checks::new();
    let model = bar();
    let load = tip_load(&model);
    let cm = A * (E_MOD * RHO).sqrt();
    let mut base = BaseMotion::new();
    base.add_set(&model, "xmin", 0, C64::ONE).unwrap();
    let cant = cantilever();
    let cload = cantilever_load(&cant);
    let wb = ModalBasis::all_modes(&cant, Some(4)).unwrap().eigenvalues()[0].sqrt();
    let run = |threads: usize| -> Vec<Vec<(u64, u64)>> {
        let s1 = DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)).threads(threads),
        )
        .unwrap();
        let s3 = DirectHarmonic::new(&cant, &DirectHarmonicOptions::new().rayleigh(0.02 * wb, 0.02 / wb).threads(threads))
            .unwrap();
        let s4 = DirectHarmonic::new(&model, &tip_dashpots(&model, 0.1 * cm, DirectHarmonicOptions::new().threads(threads)))
            .unwrap();
        let s6 = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().threads(threads)).unwrap();
        vec![
            bits(&s1.response(&load, w1()).unwrap()),
            bits(&s3.response(&cload, wb).unwrap()),
            bits(&s4.response(&load, w1()).unwrap()),
            bits(&s6.response_with_base(&zero_load(&model), &base, 0.5 * w1()).unwrap()),
        ]
    };
    let (a, b) = (run(1), run(8));
    for (k, name) in ["H1 bar", "H3 cantilever", "H4 dashpot", "H6 base"].iter().enumerate() {
        c.holds(&format!("H9 {name} bits for 1 and 8 threads"), a[k] == b[k], "compared");
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ refusals

#[test]
fn r1_to_r11_refusals() {
    let mut c = Checks::new();
    let model = bar_with(10, E_MOD);
    let load = tip_load(&model);
    refused(
        &mut c,
        "R1 negative loss factor",
        DirectHarmonic::new(&model, &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(-0.1))),
        "loss factor",
        "'all'",
    );
    refused(
        &mut c,
        "R2 loss factor on a set without material",
        DirectHarmonic::new(&model, &DirectHarmonicOptions::new().loss_factor("nosuch", LossFactor::Constant(0.1))),
        "loss factor",
        "nosuch",
    );
    refused(
        &mut c,
        "R3 table not ascending",
        ModulusTable::new(vec![10.0, 5.0], vec![E_MOD, E_MOD], vec![0.0, 0.0]),
        "modulus table",
        "5e0",
    );
    let short = ModulusTable::new(vec![0.0, 100.0], vec![E_MOD, E_MOD], vec![0.0, 0.0]).unwrap();
    let st = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(short))).unwrap();
    let w = w1();
    refused(
        &mut c,
        "R4 frequency outside the table",
        st.response(&load, w),
        "modulus table",
        &format!("{:e}", w / (2.0 * PI)),
    );
    refused(
        &mut c,
        "R5 dashpot on a missing node",
        DirectHarmonic::new(&model, &DirectHarmonicOptions::new().dashpot(Dashpot::ground(99_999, 0, 1.0))),
        "dashpot",
        "dashpot 0",
    );
    refused(
        &mut c,
        "R6 negative dashpot",
        DirectHarmonic::new(&model, &DirectHarmonicOptions::new().dashpot(Dashpot::ground(0, 0, -1.0))),
        "dashpot",
        "dashpot 0",
    );
    let plain = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let tip_node = model.mesh().node_set("xmax").unwrap()[0];
    let mut free_base = BaseMotion::new();
    free_base.add(tip_node, 0, C64::ONE);
    refused(
        &mut c,
        "R7 base motion on a free equation",
        plain.response_with_base(&load, &free_base, 0.5 * w1()),
        "base motion",
        &format!("node {tip_node} component 0"),
    );
    let mut moved = bar_with(10, E_MOD);
    moved.add_constraint(Constraint::displacement("xmax", 1, 1e-3)).unwrap();
    refused(
        &mut c,
        "R8 non-zero prescribed value",
        DirectHarmonic::new(&moved, &DirectHarmonicOptions::new()),
        "constraint",
        "prescribed value",
    );
    let mut sprung = bar_with(10, E_MOD);
    sprung.add_support(ElasticSupport::nodal_spring("xmax", 0, 1e6)).unwrap();
    refused(
        &mut c,
        "R9 elastic support (transformation path)",
        DirectHarmonic::new(&sprung, &DirectHarmonicOptions::new()),
        "constraint",
        "transformation path",
    );
    refused(&mut c, "R10 negative frequency", plain.response(&load, -1.0), "frequency", "-1e0");
    refused(&mut c, "R10 non-finite frequency", plain.response(&load, f64::NAN), "frequency", "NaN");
    refused(
        &mut c,
        "R11 load of the wrong length",
        plain.response(&HarmonicLoad::real(vec![0.0; 3]).unwrap(), 1.0),
        "harmonic load",
        "length 3",
    );
    c.finish();
}
