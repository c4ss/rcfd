//! F-556 (ADR 0483): diagnosis of the Shell3 rest stiffness of the twisted strip of ADR 0473 against Hex20. The registered
//! variation matrix D3 (membrane, split, own assembly with the stabilisation and the drilling factor, twist, thickness,
//! refinement, energy partition) and the remedy check F1. Hard checks C1 to C7, F1 and the controls N1 to N3 are asserted; the
//! diagnostic predictions P1 to P7 and Q2 to Q4 are printed and classified (the decision rule of ADR 0483). Each registered run
//! is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::{frequency, modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::rotating::{CampbellTable, RotatingOptions, RotatingPart};
use rcfd_fem::shell::{self, homogeneous_section, SectionStiffness, ShearStiffness, ShellOptions, Shell4Membrane};
use rcfd_fem::shell3::{self, Shell3Membrane, Shell3Options};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, ResidualTest, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!("{name}: {value:.6e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!("{name}: {value:.6e} (limit >= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {value:e} < {limit:e}"));
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

/// A registered prediction: printed with its outcome, not asserted (ADR 0483, the decision rule).
fn predict(name: &str, value: f64, lo: f64, hi: f64) -> bool {
    let ok = value >= lo && value <= hi;
    println!("{name}: {value:.6e} in [{lo:e}, {hi:e}] {}", if ok { "holds" } else { "does not hold" });
    ok
}

const CHORD: f64 = 0.05;
const SPAN: f64 = 0.15;
const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const SPEEDS: [f64; 4] = [0.0, 120.0, 240.0, 360.0];

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn opts() -> ModalOptions {
    ModalOptions::new(6)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-10)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Split {
    Ac,
    Bd,
    Alt,
}

/// A set-up `G(nch, nsp, twist, t, split)` of ADR 0483 D2; `twist` the root-to-tip twist, rad.
#[derive(Clone, Copy, Debug)]
struct Geo {
    nch: usize,
    nsp: usize,
    twist: f64,
    t: f64,
    split: Split,
}

const BASE: Geo = Geo {
    nch: 12,
    nsp: 36,
    twist: PI / 6.0,
    t: 0.001,
    split: Split::Ac,
};

impl Geo {
    fn with(self, f: impl FnOnce(&mut Geo)) -> Geo {
        let mut g = self;
        f(&mut g);
        g
    }
    /// The mid-surface point and unit normal at span `r`, chord `s` (ADR 0473 arithmetic 7, the twist a parameter).
    fn surface(&self, r: f64, s: f64) -> (Vec3, Vec3) {
        let th = self.twist * r / SPAN;
        let dth = self.twist / SPAN;
        let x = [r, s * th.cos(), s * th.sin()];
        let n = [-s * dth, -th.sin(), th.cos()];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        (x, [n[0] / l, n[1] / l, n[2] / l])
    }
    fn idx(&self, i: usize, j: usize) -> usize {
        j * (self.nch + 1) + i
    }
    fn nodes(&self) -> Vec<Vec3> {
        let mut nodes = Vec::new();
        for j in 0..=self.nsp {
            for i in 0..=self.nch {
                let s = -0.5 * CHORD + CHORD * i as f64 / self.nch as f64;
                nodes.push(self.surface(SPAN * j as f64 / self.nsp as f64, s).0);
            }
        }
        nodes
    }
    fn cells(&self) -> Vec<(usize, usize, [usize; 4])> {
        let mut v = Vec::new();
        for j in 0..self.nsp {
            for i in 0..self.nch {
                let q = [self.idx(i, j), self.idx(i + 1, j), self.idx(i + 1, j + 1), self.idx(i, j + 1)];
                v.push((i, j, q));
            }
        }
        v
    }
    fn triangles(&self) -> Vec<[usize; 3]> {
        let mut v = Vec::new();
        for (i, j, [a, b, c, d]) in self.cells() {
            let ac = match self.split {
                Split::Ac => true,
                Split::Bd => false,
                Split::Alt => (i + j) % 2 == 0,
            };
            if ac {
                v.push([a, b, c]);
                v.push([a, c, d]);
            } else {
                v.push([a, b, d]);
                v.push([b, c, d]);
            }
        }
        v
    }
    fn quads(&self) -> Vec<[usize; 4]> {
        self.cells().into_iter().map(|c| c.2).collect()
    }
    fn root(&self) -> Vec<usize> {
        (0..=self.nch).map(|i| self.idx(i, 0)).collect()
    }
    /// The chord and normal directions of the tip section.
    fn tip_axes(&self) -> (Vec3, Vec3) {
        let th = self.twist;
        ([0.0, th.cos(), th.sin()], [0.0, -th.sin(), th.cos()])
    }
}

/// A shell strip: Shell3 of the split with a membrane, or Shell4 with a membrane.
#[derive(Clone, Copy, Debug)]
enum Shell {
    Tri(Shell3Membrane),
    Quad(Shell4Membrane),
}

fn shell_model(g: &Geo, kind: Shell) -> SolidModel {
    let mut mesh = SolidMesh::new(g.nodes()).unwrap();
    let ne = match kind {
        Shell::Tri(mem) => {
            let t = g.triangles();
            mesh.add_block(ElementKind::Shell3, t.concat()).unwrap();
            mesh.set_block_property(0, BlockProperty::shell_with_membrane(g.t, mem).unwrap())
                .unwrap();
            t.len()
        }
        Shell::Quad(mem) => {
            let q = g.quads();
            mesh.add_block(ElementKind::Shell4, q.concat()).unwrap();
            mesh.set_block_property(0, BlockProperty::shell4_with_membrane(g.t, mem).unwrap())
                .unwrap();
            q.len()
        }
    };
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", g.root()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m
}

fn solid_model(g: &Geo) -> SolidModel {
    let base = hex_box([-0.5 * CHORD, 0.0, -0.5 * g.t], [CHORD, SPAN, g.t], [g.nch, g.nsp, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&base, |p| {
        // ADR 0473 addendum 1: the thickness coordinate taken as -p[2] keeps the element orientation.
        let (x, n) = g.surface(p[1], p[0]);
        let z = -p[2];
        [x[0] + z * n[0], x[1] + z * n[1], x[2] + z * n[2]]
    })
    .unwrap()
    .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("ymin", c, 0.0)).unwrap();
    }
    m
}

/// The torsion mode of a run (ADR 0483 D2): the mode of largest `tau`.
#[derive(Clone, Copy, Debug)]
struct Torsion {
    index: usize,
    f: f64,
    tau: f64,
    second: f64,
}

fn torsion_of(g: &Geo, freqs: &[f64], tip: impl Fn(usize, usize) -> Vec3) -> Torsion {
    let (ec, en) = g.tip_axes();
    let dot = |a: Vec3, b: Vec3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut taus = Vec::new();
    for k in 0..freqs.len() {
        let (ul, ut) = (tip(k, 0), tip(k, 1));
        let (wl, wt) = (dot(ul, en), dot(ut, en));
        let (cl, ct) = (dot(ul, ec), dot(ut, ec));
        let tau = (wl - wt).abs() / ((wl + wt).abs() + (wl - wt).abs() + (cl + ct).abs());
        taus.push(tau);
    }
    let mut index = 0;
    for k in 0..taus.len() {
        if taus[k] > taus[index] {
            index = k;
        }
    }
    let second = (0..taus.len())
        .filter(|&k| k != index)
        .map(|k| taus[k])
        .fold(0.0, f64::max);
    Torsion {
        index,
        f: freqs[index],
        tau: taus[index],
        second,
    }
}

fn displacement(r: &ModalResult, k: usize, node: usize) -> Vec3 {
    let d = r.dofs();
    [0, 1, 2].map(|c| r.modes[k][d.equation(node, c).unwrap()])
}

/// The tip nodes of a model: the shell corner nodes, or the Hex20 mid-edge nodes on `S(L, -+c/2)`.
fn tip_nodes(g: &Geo, m: &SolidModel, solid: bool) -> [usize; 2] {
    if !solid {
        return [g.idx(0, g.nsp), g.idx(g.nch, g.nsp)];
    }
    let find = |p: Vec3| {
        let nodes = m.mesh().nodes();
        let mut best = (f64::INFINITY, 0);
        for (i, x) in nodes.iter().enumerate() {
            let d = ((x[0] - p[0]).powi(2) + (x[1] - p[1]).powi(2) + (x[2] - p[2]).powi(2)).sqrt();
            if d < best.0 {
                best = (d, i);
            }
        }
        assert!(best.0 < 1e-9, "no Hex20 node at {p:?} (nearest {:e})", best.0);
        best.1
    };
    [
        find(g.surface(SPAN, -0.5 * CHORD).0),
        find(g.surface(SPAN, 0.5 * CHORD).0),
    ]
}

/// A model run: its rest modes and its torsion mode.
struct Run {
    result: ModalResult,
    torsion: Torsion,
}

fn run(name: &str, g: &Geo, m: &SolidModel, solid: bool, c: &mut Checks) -> Run {
    let result = modal_analysis(m, &opts()).unwrap();
    let tips = tip_nodes(g, m, solid);
    let torsion = torsion_of(g, &result.frequencies, |k, w| displacement(&result, k, tips[w]));
    let f: Vec<String> = result.frequencies.iter().map(|v| format!("{v:.6}")).collect();
    println!(
        "{name}: Hz {}; torsion mode {} at {:.6} Hz, tau {:.6}, second {:.6}",
        f.join(", "),
        torsion.index,
        torsion.f,
        torsion.tau,
        torsion.second
    );
    c.at_least(&format!("C2 {name} tau"), torsion.tau, 0.8);
    Run { result, torsion }
}

/// The own assembly of ADR 0483 D2: global `K`, `M` from the element functions, the root dropped, six rest pairs.
struct Own {
    freqs: Vec<f64>,
    modes: Vec<Vec<f64>>,
}

fn own_assembly(g: &Geo, o: &Shell3Options) -> Own {
    let nodes = g.nodes();
    let n = 6 * nodes.len();
    let mut free = vec![None; n];
    let root = g.root();
    let mut nf = 0;
    for (node, slot) in free.chunks_mut(6).enumerate() {
        if !root.contains(&node) {
            for s in slot.iter_mut() {
                *s = Some(nf);
                nf += 1;
            }
        }
    }
    let mat = steel();
    let (mut kt, mut mt) = (Vec::new(), Vec::new());
    for tri in g.triangles() {
        let x = [nodes[tri[0]], nodes[tri[1]], nodes[tri[2]]];
        let ke = shell3::element_stiffness_with(&x, g.t, &mat, o).unwrap();
        let me = shell3::element_mass(&x, g.t, &mat).unwrap();
        for a in 0..18 {
            let Some(ia) = free[6 * tri[a / 6] + a % 6] else { continue };
            for b in 0..18 {
                let Some(ib) = free[6 * tri[b / 6] + b % 6] else { continue };
                kt.push((ia, ib, ke.values()[a * 18 + b]));
                mt.push((ia, ib, me.values()[a * 18 + b]));
            }
        }
    }
    let k = CsrMatrix::from_triplets(nf, kt);
    let m = CsrMatrix::from_triplets(nf, mt);
    let so = SubspaceOptions {
        shift: Shift::Value(0.0),
        tolerance: 1e-10,
        residual_test: ResidualTest::Backward,
        ..SubspaceOptions::default()
    };
    let e = lowest_eigenpairs(&k, &m, 6, &so).unwrap();
    let modes = e
        .vectors
        .iter()
        .map(|v| free.iter().map(|s| s.map_or(0.0, |i| v[i])).collect())
        .collect();
    Own {
        freqs: e.values.iter().map(|&l| frequency(l)).collect(),
        modes,
    }
}

fn own_run(name: &str, g: &Geo, o: &Shell3Options, c: &mut Checks) -> (Own, Torsion) {
    let own = own_assembly(g, o);
    let tips = [g.idx(0, g.nsp), g.idx(g.nch, g.nsp)];
    let t = torsion_of(g, &own.freqs, |k, w| {
        [0, 1, 2].map(|q| own.modes[k][6 * tips[w] + q])
    });
    let f: Vec<String> = own.freqs.iter().map(|v| format!("{v:.6}")).collect();
    println!(
        "{name}: Hz {}; torsion mode {} at {:.6} Hz, tau {:.6}",
        f.join(", "),
        t.index,
        t.f,
        t.tau
    );
    c.at_least(&format!("C2 {name} tau"), t.tau, 0.8);
    (own, t)
}

/// The energy parts (membrane, bending, shear, drilling) and the whole of a mode of a shell model (ADR 0483 D2).
fn partition(g: &Geo, kind: Shell, r: &ModalResult, k: usize) -> ([f64; 4], f64) {
    let nodes = g.nodes();
    let hs = homogeneous_section(&steel(), g.t);
    let z = [[0.0; 3]; 3];
    let parts = [
        SectionStiffness::new(hs.membrane, None, z, ShearStiffness::Isotropic(0.0), [0.0, g.t]),
        SectionStiffness::new(z, None, hs.bending, ShearStiffness::Isotropic(0.0), [0.0, g.t]),
        SectionStiffness::new(z, None, z, hs.shear, [0.0, g.t]),
        SectionStiffness::new(z, None, z, ShearStiffness::Isotropic(0.0), [hs.drilling[0], g.t]),
        hs,
    ];
    let d = r.dofs();
    let x = &r.modes[k];
    let conns: Vec<Vec<usize>> = match kind {
        Shell::Tri(_) => g.triangles().into_iter().map(|t| t.to_vec()).collect(),
        Shell::Quad(_) => g.quads().into_iter().map(|q| q.to_vec()).collect(),
    };
    let mut e = [0.0; 5];
    for conn in conns {
        let xs: Vec<Vec3> = conn.iter().map(|&i| nodes[i]).collect();
        let nd = 6 * conn.len();
        let xe: Vec<f64> = (0..nd).map(|a| x[d.equation(conn[a / 6], a % 6).unwrap()]).collect();
        for (p, sec) in parts.iter().enumerate() {
            let ke = match kind {
                Shell::Tri(mem) => {
                    shell3::element_stiffness_section(&xs, sec, &Shell3Options::default().membrane(mem)).unwrap()
                }
                Shell::Quad(mem) => {
                    let mut o = ShellOptions::default();
                    o.membrane = mem;
                    shell::element_stiffness_section(&xs, sec, &o).unwrap()
                }
            };
            let v = ke.values();
            let mut q = 0.0;
            for a in 0..nd {
                for b in 0..nd {
                    q += xe[a] * v[a * nd + b] * xe[b];
                }
            }
            e[p] += 0.5 * q;
        }
    }
    ([e[0], e[1], e[2], e[3]], e[4])
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| rel(*x, *y)).fold(0.0, f64::max)
}

/// The closed form of ADR 0483 arithmetic 2, Hz.
fn closed_form(g: &Geo) -> (f64, f64) {
    let gm = E / (2.0 * (1.0 + NU));
    let thp = g.twist / SPAN;
    let sv = gm * CHORD * g.t.powi(3) / 3.0;
    let pt = E * g.t * CHORD.powi(5) * thp * thp / 180.0;
    let ip = RHO * CHORD.powi(3) * g.t / 12.0;
    let w = PI / (2.0 * SPAN) * ((sv + pt) / ip).sqrt();
    (w / (2.0 * PI), pt / (sv + pt))
}

const CST: Shell = Shell::Tri(Shell3Membrane::ConstantStrain);
const ANDES: Shell = Shell::Tri(Shell3Membrane::AndesOpt);
const ALLMAN: Shell = Shell::Tri(Shell3Membrane::Allman);
const Q4: Shell = Shell::Quad(Shell4Membrane::Bilinear);
const Q4_ANDES: Shell = Shell::Quad(Shell4Membrane::AndesOpt);

/// The record of the first run (ADR 0483 addendum 1): the rule "largest `tau`" took the second torsion of Shell3 at twist 0
/// (N1 missed, 0.98395); C3 (3.69e-8), C7 at twist 0 (9.29e-2) and N3 (1.9414 against 2.0736) missed. Corrected by amendment 1
/// in `d1b_variation_matrix_first_torsion`.
#[test]
#[ignore = "record of the first run of ADR 0483 (addendum 1); corrected by amendment 1"]
fn d1_variation_matrix_base_twist_thickness_energy() {
    let mut c = Checks::new();
    // ---- the base group
    let h20 = run("base Hex20", &BASE, &solid_model(&BASE), true, &mut c);
    let s4 = run("base Shell4", &BASE, &shell_model(&BASE, Q4), false, &mut c);
    let s4a = run("base Shell4 ANDES", &BASE, &shell_model(&BASE, Q4_ANDES), false, &mut c);
    let s3 = run("base Shell3 CST AC", &BASE, &shell_model(&BASE, CST), false, &mut c);
    let bd = BASE.with(|g| g.split = Split::Bd);
    let alt = BASE.with(|g| g.split = Split::Alt);
    let s3bd = run("base Shell3 CST BD", &bd, &shell_model(&bd, CST), false, &mut c);
    let s3alt = run("base Shell3 CST ALT", &alt, &shell_model(&alt, CST), false, &mut c);
    let s3a = run("base Shell3 ANDES AC", &BASE, &shell_model(&BASE, ANDES), false, &mut c);
    let s3a_alt = run("base Shell3 ANDES ALT", &alt, &shell_model(&alt, ANDES), false, &mut c);
    let s3al = run("base Shell3 Allman AC", &BASE, &shell_model(&BASE, ALLMAN), false, &mut c);
    let fh = h20.torsion.f;
    let d = |r: &Run| r.torsion.f / fh - 1.0;
    for (n, r) in [
        ("Shell4", &s4),
        ("Shell4 ANDES", &s4a),
        ("Shell3 CST AC", &s3),
        ("Shell3 CST BD", &s3bd),
        ("Shell3 CST ALT", &s3alt),
        ("Shell3 ANDES AC", &s3a),
        ("Shell3 ANDES ALT", &s3a_alt),
        ("Shell3 Allman AC", &s3al),
    ] {
        println!("base delta {n}: {:.6e}", d(r));
    }
    // C1, C1b, C2b
    c.at_most("C1 Hex20 torsion against 442.797470 Hz", rel(fh, 442.797470), 1e-7);
    c.at_most("C1 Shell4 torsion against 445.161813 Hz", rel(s4.torsion.f, 445.161813), 1e-7);
    c.at_most("C1 Shell3 torsion against 460.730207 Hz", rel(s3.torsion.f, 460.730207), 1e-7);
    c.at_most("C1b delta_3 against 4.0499e-2", (d(&s3) - 4.0499e-2).abs(), 1e-5);
    c.at_most("C1b delta_4 against 5.3396e-3", (d(&s4) - 5.3396e-3).abs(), 1e-5);
    c.holds(
        "C2b torsion is the third mode",
        h20.torsion.index == 2 && s4.torsion.index == 2 && s3.torsion.index == 2,
        &format!("{} {} {}", h20.torsion.index, s4.torsion.index, s3.torsion.index),
    );
    // N1
    c.at_most("N1 base Shell3 second-largest tau (expected < 0.5)", s3.torsion.second, 0.5);
    // C3
    c.at_most(
        "C3 AC against BD, six frequencies",
        max_rel(&s3.result.frequencies, &s3bd.result.frequencies),
        1e-8,
    );
    // ---- the own assembly
    let o = Shell3Options::default();
    let (own, ot) = own_run("OA base default", &BASE, &o, &mut c);
    c.at_most("C4 OA default against the model", max_rel(&own.freqs, &s3.result.frequencies), 1e-8);
    let (_, o_stab) = own_run("OA base stabilisation 0", &BASE, &o.stabilisation(0.0), &mut c);
    let (_, o_d4) = own_run("OA base drilling 1e-4", &BASE, &o.drilling(1e-4), &mut c);
    let (_, o_d2) = own_run("OA base drilling 1e-2", &BASE, &o.drilling(1e-2), &mut c);
    let sh_stab = rel(o_stab.f, ot.f);
    let sh_d4 = rel(o_d4.f, ot.f);
    let sh_d2 = rel(o_d2.f, ot.f);
    println!("OA base shifts: stabilisation 0 {sh_stab:.6e}, drilling 1e-4 {sh_d4:.6e}, drilling 1e-2 {sh_d2:.6e}");
    // ---- the energy partition
    let mut shares = Vec::new();
    for (n, kind, r) in [("Shell3 CST", CST, &s3), ("Shell3 ANDES", ANDES, &s3a), ("Shell4", Q4, &s4)] {
        let k = r.torsion.index;
        let (p, whole) = partition(&BASE, kind, &r.result, k);
        let sum: f64 = p.iter().sum();
        let lam = r.result.eigenvalues[k];
        println!(
            "energy {n}: membrane {:.6}, bending {:.6}, shear {:.6}, drilling {:.6} of the whole {whole:.9e}",
            p[0] / whole,
            p[1] / whole,
            p[2] / whole,
            p[3] / whole
        );
        c.at_most(&format!("C5 {n} sum of parts against the whole"), rel(sum, whole), 1e-11);
        c.at_most(&format!("C5 {n} whole against lambda/2"), rel(whole, 0.5 * lam), 1e-8);
        let no_mem = p[1] + p[2] + p[3];
        let n2 = rel(no_mem, whole);
        println!("N2 {n} without the membrane part: {n2:.6e} (expected > 0.5)");
        c.holds(&format!("N2 {n}"), n2 > 0.5, &format!("{n2:.6e}"));
        shares.push((p[0] / whole, p[3] / whole));
    }
    // ---- twist and thickness
    let mut groups = Vec::new();
    let mut h20_t = Vec::new();
    for (label, g) in [
        ("twist 0", BASE.with(|g| g.twist = 0.0)),
        ("twist 15", BASE.with(|g| g.twist = PI / 12.0)),
        ("t 0.5 mm", BASE.with(|g| g.t = 0.0005)),
        ("t 2 mm", BASE.with(|g| g.t = 0.002)),
    ] {
        let rh = run(&format!("{label} Hex20"), &g, &solid_model(&g), true, &mut c);
        let r4 = run(&format!("{label} Shell4"), &g, &shell_model(&g, Q4), false, &mut c);
        let r3 = run(&format!("{label} Shell3 CST AC"), &g, &shell_model(&g, CST), false, &mut c);
        let r3a = run(&format!("{label} Shell3 ANDES AC"), &g, &shell_model(&g, ANDES), false, &mut c);
        let fh = rh.torsion.f;
        let (d4, d3, d3a) = (r4.torsion.f / fh - 1.0, r3.torsion.f / fh - 1.0, r3a.torsion.f / fh - 1.0);
        println!("{label}: delta_4 {d4:.6e}, delta_3 CST {d3:.6e}, delta_3 ANDES {d3a:.6e}; Delta CST {:.6e}, Delta ANDES {:.6e}", d3 - d4, d3a - d4);
        let (cf, m) = closed_form(&g);
        c.at_most(&format!("C7 {label} Hex20 against the closed form {cf:.3} Hz"), rel(fh, cf), 6e-2);
        groups.push((label, g, d3 - d4, d3a - d4, m));
        h20_t.push(fh);
    }
    let (cf, m_b) = closed_form(&BASE);
    c.at_most(&format!("C7 base Hex20 against the closed form {cf:.3} Hz"), rel(fh, cf), 6e-2);
    // N3
    let ratio = fh / h20_t[0];
    let cf_ratio = closed_form(&BASE).0 / closed_form(&BASE.with(|g| g.twist = 0.0)).0;
    println!("N3 Hex20 twist 30/twist 0 {ratio:.6} against the closed form {cf_ratio:.6}");
    c.holds(
        "N3",
        ratio > 1.5 && (ratio - cf_ratio).abs() <= 1e-1,
        &format!("{ratio:.6} against {cf_ratio:.6}"),
    );
    // ---- the twist-0 own assembly
    let g0 = groups[0].1;
    let (_, o0) = own_run("OA twist 0 default", &g0, &o, &mut c);
    let (_, o0s) = own_run("OA twist 0 stabilisation 0", &g0, &o.stabilisation(0.0), &mut c);
    let sh0 = rel(o0s.f, o0.f);
    println!("OA twist 0 stabilisation shift {sh0:.6e}");
    // ---- classification (printed)
    let delta_b = d(&s3) - d(&s4);
    let delta_a = d(&s3a) - d(&s4);
    let delta_alt = d(&s3alt) - d(&s4);
    let e_m = 2.0 * delta_b / m_b;
    println!("Delta_b {delta_b:.6e}, membrane share {m_b:.4}, e_m {e_m:.6e}");
    let p1 = predict("P1 twist 0 |Delta|", groups[0].2.abs(), 0.0, 5e-3);
    let p2 = predict("P2 ANDES |Delta|/Delta_b", delta_a.abs() / delta_b, 0.0, 0.25);
    let mut p3 = true;
    for (label, _, dl, _, m) in groups.iter().skip(1) {
        p3 &= predict(&format!("P3 {label} Delta/(m e_m/2), m {m:.4}"), dl / (0.5 * m * e_m), 0.6, 1.4);
    }
    let p5 = predict("P5 ALT Delta/Delta_b", delta_alt / delta_b, 0.5, 1.5);
    let p6 = predict("P6 OA stabilisation 0", sh_stab, 0.0, 5e-3)
        & predict("P6 OA drilling 1e-4", sh_d4, 0.0, 5e-3)
        & predict("P6 OA drilling 1e-2", sh_d2, 0.0, 5e-3);
    let p7 = predict("P7 CST membrane share", shares[0].0, 0.65, 0.90)
        & predict("P7 CST drilling share", shares[0].1, 0.0, 2e-2);
    let q2 = delta_a > 0.5 * delta_b && !(0.5..=1.5).contains(&(delta_alt / delta_b));
    let q3 = groups[0].2.abs() > 1e-2 || sh_stab > 1e-2 || sh0 > 1e-2;
    let q4 = sh_d4 > 1e-2 || sh_d2 > 1e-2 || shares[0].1 > 2e-2;
    println!("H1 predictions P1 {p1} P2 {p2} P3 {p3} P5 {p5} P6 {p6} P7 {p7} (P4 in d2); Q2 {q2} Q3 {q3} Q4 {q4}");
    c.finish();
}

#[test]
fn d2_refinement() {
    let mut c = Checks::new();
    let h12 = run("12 x 36 Hex20", &BASE, &solid_model(&BASE), true, &mut c);
    let g24 = BASE.with(|g| {
        g.nch = 24;
        g.nsp = 72;
    });
    let g48 = BASE.with(|g| {
        g.nch = 48;
        g.nsp = 144;
    });
    let h24 = run("24 x 72 Hex20", &g24, &solid_model(&g24), true, &mut c);
    c.at_most("C6 Hex20 24 x 72 against 12 x 36", rel(h24.torsion.f, h12.torsion.f), 5e-3);
    let fr = h24.torsion.f;
    let mut deltas = Vec::new();
    for (label, g) in [("12 x 36", BASE), ("24 x 72", g24), ("48 x 144", g48)] {
        let r4 = run(&format!("{label} Shell4"), &g, &shell_model(&g, Q4), false, &mut c);
        let r3 = run(&format!("{label} Shell3 CST AC"), &g, &shell_model(&g, CST), false, &mut c);
        let r3a = run(&format!("{label} Shell3 ANDES AC"), &g, &shell_model(&g, ANDES), false, &mut c);
        let (d4, d3, d3a) = (r4.torsion.f / fr - 1.0, r3.torsion.f / fr - 1.0, r3a.torsion.f / fr - 1.0);
        println!("{label} against Hex20 24 x 72: delta_4 {d4:.6e}, delta_3 CST {d3:.6e}, delta_3 ANDES {d3a:.6e}; Delta CST {:.6e}, Delta ANDES {:.6e}", d3 - d4, d3a - d4);
        deltas.push((d3 - d4, d3a - d4));
    }
    let p4a = predict("P4 Delta(24 x 72)/Delta(12 x 36)", deltas[1].0 / deltas[0].0, 0.15, 0.40);
    let p4b = predict("P4 Delta(48 x 144)/Delta(24 x 72)", deltas[2].0 / deltas[1].0, 0.15, 0.40);
    println!("H1 prediction P4 {}", p4a && p4b);
    c.finish();
}

fn table_of(m: &SolidModel) -> CampbellTable {
    RotatingPart::new(m, AXIS, ORIGIN, &RotatingOptions::new(6).modal(opts()))
        .unwrap()
        .campbell(&SPEEDS)
        .unwrap()
}

#[test]
fn f1_andes_shell3_twisted_strip_campbell() {
    let mut c = Checks::new();
    let solid = table_of(&solid_model(&BASE));
    let shell = table_of(&shell_model(&BASE, ANDES));
    let mut worst: f64 = 0.0;
    for k in 0..4 {
        let (a, b): (Vec<String>, Vec<String>) = (0..SPEEDS.len())
            .map(|j| {
                (
                    format!("{:.6}", shell.tracks[k].frequencies[j].unwrap_or(f64::NAN)),
                    format!("{:.6}", solid.tracks[k].frequencies[j].unwrap_or(f64::NAN)),
                )
            })
            .unzip();
        println!("F1 track {k} Hz: ANDES Shell3 {}; Hex20 {}", a.join(", "), b.join(", "));
        for j in 0..SPEEDS.len() {
            let v = match (shell.tracks[k].frequencies[j], solid.tracks[k].frequencies[j]) {
                (Some(a), Some(b)) => rel(a, b),
                _ => f64::INFINITY,
            };
            println!("F1 track {k} speed {}: {v:.4e}", SPEEDS[j]);
            worst = worst.max(v);
        }
    }
    c.at_most("F1 ANDES Shell3 max |omega_shell/omega_solid - 1|", worst, 4e-2);
    let mut ok = shell.losses.is_empty();
    let mut low: f64 = 1.0;
    for tr in shell.tracks.iter().take(4) {
        for m in tr.mac.iter().skip(1) {
            let v = m.unwrap_or(0.0);
            low = low.min(v);
            ok &= v > 0.9;
        }
    }
    c.holds("F1 E3 ANDES Shell3", ok, &format!("losses {}, lowest MAC {low:.6}", shell.losses.len()));
    c.finish();
}

// ------------------------------------------------------------------------------------------------- amendment 1 (ADR 0483)

/// The first torsion of amendment 1: the lowest mode with `tau >= 0.8`; `second` is the largest `tau` of the modes below it
/// (corrected N1).
fn first_torsion_of(g: &Geo, freqs: &[f64], tip: impl Fn(usize, usize) -> Vec3) -> (Torsion, Vec<f64>) {
    let (ec, en) = g.tip_axes();
    let dot = |a: Vec3, b: Vec3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut taus = Vec::new();
    for k in 0..freqs.len() {
        let (ul, ut) = (tip(k, 0), tip(k, 1));
        let (wl, wt) = (dot(ul, en), dot(ut, en));
        let (cl, ct) = (dot(ul, ec), dot(ut, ec));
        taus.push((wl - wt).abs() / ((wl + wt).abs() + (wl - wt).abs() + (cl + ct).abs()));
    }
    let index = taus.iter().position(|&t| t >= 0.8).unwrap_or(0);
    let second = taus[..index].iter().copied().fold(0.0, f64::max);
    (
        Torsion {
            index,
            f: freqs[index],
            tau: taus[index],
            second,
        },
        taus,
    )
}

fn report(name: &str, freqs: &[f64], t: &Torsion, taus: &[f64], c: &mut Checks) {
    let f: Vec<String> = freqs.iter().map(|v| format!("{v:.6}")).collect();
    let ta: Vec<String> = taus.iter().map(|v| format!("{v:.4}")).collect();
    println!(
        "{name}: Hz {}; tau {}; first torsion mode {} at {:.6} Hz",
        f.join(", "),
        ta.join(", "),
        t.index,
        t.f
    );
    c.at_least(&format!("C2 {name} tau"), t.tau, 0.8);
    c.holds(
        &format!("N1 {name} modes below the first torsion"),
        t.second < 0.5,
        &format!("largest tau below {:.6}", t.second),
    );
}

fn run_ft(name: &str, g: &Geo, m: &SolidModel, solid: bool, c: &mut Checks) -> Run {
    let result = modal_analysis(m, &opts()).unwrap();
    let tips = tip_nodes(g, m, solid);
    let (torsion, taus) = first_torsion_of(g, &result.frequencies, |k, w| displacement(&result, k, tips[w]));
    report(name, &result.frequencies, &torsion, &taus, c);
    Run { result, torsion }
}

fn own_run_ft(name: &str, g: &Geo, o: &Shell3Options, c: &mut Checks) -> (Own, Torsion) {
    let own = own_assembly(g, o);
    let tips = [g.idx(0, g.nsp), g.idx(g.nch, g.nsp)];
    let (t, taus) = first_torsion_of(g, &own.freqs, |k, w| {
        [0, 1, 2].map(|q| own.modes[k][6 * tips[w] + q])
    });
    report(name, &own.freqs, &t, &taus, c);
    (own, t)
}

#[test]
fn d1b_variation_matrix_first_torsion() {
    let mut c = Checks::new();
    // ---- the base group
    let h20 = run_ft("base Hex20", &BASE, &solid_model(&BASE), true, &mut c);
    let s4 = run_ft("base Shell4", &BASE, &shell_model(&BASE, Q4), false, &mut c);
    let s4a = run_ft("base Shell4 ANDES", &BASE, &shell_model(&BASE, Q4_ANDES), false, &mut c);
    let s3 = run_ft("base Shell3 CST AC", &BASE, &shell_model(&BASE, CST), false, &mut c);
    let bd = BASE.with(|g| g.split = Split::Bd);
    let alt = BASE.with(|g| g.split = Split::Alt);
    let s3bd = run_ft("base Shell3 CST BD", &bd, &shell_model(&bd, CST), false, &mut c);
    let s3alt = run_ft("base Shell3 CST ALT", &alt, &shell_model(&alt, CST), false, &mut c);
    let s3a = run_ft("base Shell3 ANDES AC", &BASE, &shell_model(&BASE, ANDES), false, &mut c);
    let s3a_alt = run_ft("base Shell3 ANDES ALT", &alt, &shell_model(&alt, ANDES), false, &mut c);
    let s3al = run_ft("base Shell3 Allman AC", &BASE, &shell_model(&BASE, ALLMAN), false, &mut c);
    let fh = h20.torsion.f;
    let d = |r: &Run| r.torsion.f / fh - 1.0;
    for (n, r) in [
        ("Shell4", &s4),
        ("Shell4 ANDES", &s4a),
        ("Shell3 CST AC", &s3),
        ("Shell3 CST BD", &s3bd),
        ("Shell3 CST ALT", &s3alt),
        ("Shell3 ANDES AC", &s3a),
        ("Shell3 ANDES ALT", &s3a_alt),
        ("Shell3 Allman AC", &s3al),
    ] {
        println!("base delta {n}: {:.6e}", d(r));
    }
    c.at_most("C1 Hex20 torsion against 442.797470 Hz", rel(fh, 442.797470), 1e-7);
    c.at_most("C1 Shell4 torsion against 445.161813 Hz", rel(s4.torsion.f, 445.161813), 1e-7);
    c.at_most("C1 Shell3 torsion against 460.730207 Hz", rel(s3.torsion.f, 460.730207), 1e-7);
    c.at_most("C1b delta_3 against 4.0499e-2", (d(&s3) - 4.0499e-2).abs(), 1e-5);
    c.at_most("C1b delta_4 against 5.3396e-3", (d(&s4) - 5.3396e-3).abs(), 1e-5);
    c.holds(
        "C2b torsion is the third mode",
        h20.torsion.index == 2 && s4.torsion.index == 2 && s3.torsion.index == 2,
        &format!("{} {} {}", h20.torsion.index, s4.torsion.index, s3.torsion.index),
    );
    println!(
        "C3 (known miss, addendum 1; not asserted here) AC against BD, six frequencies: {:.6e}; torsion {:.6e}",
        max_rel(&s3.result.frequencies, &s3bd.result.frequencies),
        rel(s3.torsion.f, s3bd.torsion.f)
    );
    // ---- the own assembly
    let o = Shell3Options::default();
    let (own, ot) = own_run_ft("OA base default", &BASE, &o, &mut c);
    c.at_most("C4 OA default against the model", max_rel(&own.freqs, &s3.result.frequencies), 1e-8);
    let (_, o_stab) = own_run_ft("OA base stabilisation 0", &BASE, &o.stabilisation(0.0), &mut c);
    let (_, o_d4) = own_run_ft("OA base drilling 1e-4", &BASE, &o.drilling(1e-4), &mut c);
    let (_, o_d2) = own_run_ft("OA base drilling 1e-2", &BASE, &o.drilling(1e-2), &mut c);
    let sh_stab = rel(o_stab.f, ot.f);
    let sh_d4 = rel(o_d4.f, ot.f);
    let sh_d2 = rel(o_d2.f, ot.f);
    println!("OA base shifts: stabilisation 0 {sh_stab:.6e}, drilling 1e-4 {sh_d4:.6e}, drilling 1e-2 {sh_d2:.6e}");
    // ---- the energy partition
    let mut shares = Vec::new();
    for (n, kind, r) in [("Shell3 CST", CST, &s3), ("Shell3 ANDES", ANDES, &s3a), ("Shell4", Q4, &s4)] {
        let k = r.torsion.index;
        let (p, whole) = partition(&BASE, kind, &r.result, k);
        let sum: f64 = p.iter().sum();
        let lam = r.result.eigenvalues[k];
        println!(
            "energy {n}: membrane {:.6}, bending {:.6}, shear {:.6}, drilling {:.6} of the whole {whole:.9e}",
            p[0] / whole,
            p[1] / whole,
            p[2] / whole,
            p[3] / whole
        );
        c.at_most(&format!("C5 {n} sum of parts against the whole"), rel(sum, whole), 1e-11);
        c.at_most(&format!("C5 {n} whole against lambda/2"), rel(whole, 0.5 * lam), 1e-8);
        let n2 = rel(p[1] + p[2] + p[3], whole);
        c.holds(&format!("N2 {n} without the membrane part"), n2 > 0.5, &format!("{n2:.6e}"));
        shares.push((p[0] / whole, p[3] / whole));
    }
    // ---- twist and thickness
    let mut groups = Vec::new();
    let mut h20_t = Vec::new();
    for (label, g) in [
        ("twist 0", BASE.with(|g| g.twist = 0.0)),
        ("twist 15", BASE.with(|g| g.twist = PI / 12.0)),
        ("t 0.5 mm", BASE.with(|g| g.t = 0.0005)),
        ("t 2 mm", BASE.with(|g| g.t = 0.002)),
    ] {
        let rh = run_ft(&format!("{label} Hex20"), &g, &solid_model(&g), true, &mut c);
        let r4 = run_ft(&format!("{label} Shell4"), &g, &shell_model(&g, Q4), false, &mut c);
        let r3 = run_ft(&format!("{label} Shell3 CST AC"), &g, &shell_model(&g, CST), false, &mut c);
        let r3a = run_ft(&format!("{label} Shell3 ANDES AC"), &g, &shell_model(&g, ANDES), false, &mut c);
        let fh = rh.torsion.f;
        let (d4, d3, d3a) = (r4.torsion.f / fh - 1.0, r3.torsion.f / fh - 1.0, r3a.torsion.f / fh - 1.0);
        println!(
            "{label}: delta_4 {d4:.6e}, delta_3 CST {d3:.6e}, delta_3 ANDES {d3a:.6e}; Delta CST {:.6e}, Delta ANDES {:.6e}",
            d3 - d4,
            d3a - d4
        );
        let (cf, m) = closed_form(&g);
        if g.twist == 0.0 {
            println!(
                "C7 (known miss, addendum 1; not asserted here) {label} Hex20 against the closed form {cf:.3} Hz: {:.6e}",
                rel(fh, cf)
            );
        } else {
            c.at_most(&format!("C7 {label} Hex20 against the closed form {cf:.3} Hz"), rel(fh, cf), 6e-2);
        }
        groups.push((label, g, d3 - d4, d3a - d4, m));
        h20_t.push(fh);
    }
    let (cf, m_b) = closed_form(&BASE);
    c.at_most(&format!("C7 base Hex20 against the closed form {cf:.3} Hz"), rel(fh, cf), 6e-2);
    let cf_ratio = cf / closed_form(&BASE.with(|g| g.twist = 0.0)).0;
    println!(
        "N3 (known miss, addendum 1; not asserted here) Hex20 twist 30/twist 0 {:.6} against the closed form {cf_ratio:.6}",
        fh / h20_t[0]
    );
    // ---- the twist-0 own assembly
    let g0 = groups[0].1;
    let (_, o0) = own_run_ft("OA twist 0 default", &g0, &o, &mut c);
    let (_, o0s) = own_run_ft("OA twist 0 stabilisation 0", &g0, &o.stabilisation(0.0), &mut c);
    let sh0 = rel(o0s.f, o0.f);
    println!("OA twist 0 stabilisation shift {sh0:.6e}");
    // ---- classification (printed)
    let delta_b = d(&s3) - d(&s4);
    let delta_a = d(&s3a) - d(&s4);
    let delta_alt = d(&s3alt) - d(&s4);
    let e_m = 2.0 * delta_b / m_b;
    println!("Delta_b {delta_b:.6e}, membrane share {m_b:.4}, e_m {e_m:.6e}");
    let p1 = predict("P1 twist 0 |Delta|", groups[0].2.abs(), 0.0, 5e-3);
    let p2 = predict("P2 ANDES |Delta|/Delta_b", delta_a.abs() / delta_b, 0.0, 0.25);
    let mut p3 = true;
    for (label, _, dl, _, m) in groups.iter().skip(1) {
        p3 &= predict(&format!("P3 {label} Delta/(m e_m/2), m {m:.4}"), dl / (0.5 * m * e_m), 0.6, 1.4);
    }
    let p5 = predict("P5 ALT Delta/Delta_b", delta_alt / delta_b, 0.5, 1.5);
    let p6 = predict("P6 OA stabilisation 0", sh_stab, 0.0, 5e-3)
        & predict("P6 OA drilling 1e-4", sh_d4, 0.0, 5e-3)
        & predict("P6 OA drilling 1e-2", sh_d2, 0.0, 5e-3);
    let p7 = predict("P7 CST membrane share", shares[0].0, 0.65, 0.90)
        & predict("P7 CST drilling share", shares[0].1, 0.0, 2e-2);
    let q2 = delta_a > 0.5 * delta_b && !(0.5..=1.5).contains(&(delta_alt / delta_b));
    let q3 = groups[0].2.abs() > 1e-2 || sh_stab > 1e-2 || sh0 > 1e-2;
    let q4 = sh_d4 > 1e-2 || sh_d2 > 1e-2 || shares[0].1 > 2e-2;
    println!("H1 predictions P1 {p1} P2 {p2} P3 {p3} P5 {p5} P6 {p6} P7 {p7} (P4 in d2); Q2 {q2} Q3 {q3} Q4 {q4}");
    c.finish();
}

/// C3 missed on the first run (ADR 0483 addendum 1): 3.686e-8 against 1e-8, on modes 5 and 6 (the pairs converged to the
/// backward error 1e-10, not to rounding); the first four modes agree to the printed 6 decimals.
#[test]
#[ignore = "known miss of ADR 0483 check C3, recorded in addendum 1"]
fn c3_ac_bd_congruence_known_miss() {
    let mut c = Checks::new();
    let bd = BASE.with(|g| g.split = Split::Bd);
    let a = modal_analysis(&shell_model(&BASE, CST), &opts()).unwrap();
    let b = modal_analysis(&shell_model(&bd, CST), &opts()).unwrap();
    c.at_most("C3 AC against BD, six frequencies", max_rel(&a.frequencies, &b.frequencies), 1e-8);
    c.finish();
}

/// C7 at twist 0 and N3 missed on the first run (ADR 0483 addendum 1): Hex20 228.084 Hz against the closed form 208.690 Hz
/// (9.29e-2 against 6e-2), the ratio twist 30/twist 0 1.9414 against 2.0736 (0.132 against 0.1). The closed form neglects the
/// root restraint `phi'(0) = 0` of the clamped plate strip.
#[test]
#[ignore = "known miss of ADR 0483 checks C7 (twist 0) and N3, recorded in addendum 1"]
fn c7_n3_twist0_closed_form_known_miss() {
    let mut c = Checks::new();
    let g0 = BASE.with(|g| g.twist = 0.0);
    let r0 = run_ft("twist 0 Hex20", &g0, &solid_model(&g0), true, &mut c);
    let rb = run_ft("base Hex20", &BASE, &solid_model(&BASE), true, &mut c);
    let cf0 = closed_form(&g0).0;
    c.at_most("C7 twist 0 Hex20 against the closed form", rel(r0.torsion.f, cf0), 6e-2);
    let ratio = rb.torsion.f / r0.torsion.f;
    let cf_ratio = closed_form(&BASE).0 / cf0;
    c.holds(
        "N3",
        ratio > 1.5 && (ratio - cf_ratio).abs() <= 1e-1,
        &format!("{ratio:.6} against {cf_ratio:.6}"),
    );
    c.finish();
}
