//! F-289 (ADR 0202): the conjugate `heat_flow` report follows the gradient options the energy equation was solved with
//! (ADR 0190 Q3 (b)), checked by a heat balance on the skewed conjugate set-up C (mesh W of ADR 0190, a solid zone with a
//! volumetric source, fixed temperatures on the ramp and the inlet side). The limits are those registered in ADR 0202.
//!
//! For a finished run the test rebuilds, with `rcfd_fv` calls only, the field (the tree's final `T` with the case's boundary
//! conditions), the face conductivity and the residual `r` of the solved equation (`laplacian_opt` with the set's options
//! plus the source). `tol_W = (rho cp)_ref sum |r|` is the solver tolerance in W. P: the mesh is skewed. E-tree: per cell the
//! new flows are the operator's. R-eq: the tree's reports are the patch sums of the new flows. B1: the reports balance the
//! source to `tol_W`; B2: `tol_W <= 1e-8 Q`; B3: the same per material. T-B1, T-B3, T-O1: the unchanged formula misses B1
//! (every set that changes boundary faces) and B3 (every set), and the face correction alone does not change a patch report.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::cht::{
    conductive_face_flows, conductive_face_flows_opt, group_net_outflow, patch_heat_flow_out, ChtMaterials,
};
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, ZeroGradient};

/// E-tree limit, relative to the largest cell sum of |flow| / (rho cp)_ref.
const IDENTITY: f64 = 1e-12;
/// R-eq limit, relative to `Q + sum |H_p|`.
const REPORT: f64 = 1e-12;
/// Round-off allowance of B1 and B3, relative to `F` (the sum of |flow| over all faces).
const ROUND_OFF: f64 = 1e-12;
/// B2: the solver tolerance against the source.
const CONVERGED: f64 = 1e-8;
/// P: the smallest maximum non-orthogonality of the set-up, degrees.
const SKEWED: f64 = 20.0;
const PATCHES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];
/// (rho cp)_ref of C (physics.rho * physics.cp).
const RHO_CP: f64 = 1.0;

/// One option set: name, the `[solver]` lines, the library options.
struct Set {
    name: &'static str,
    keys: &'static str,
    opts: Option<GradientOptions>,
}

/// The default and ADR 0190's sets O1 to O6 and C2.
fn sets() -> Vec<Set> {
    let d = GradientOptions::default();
    let set = |name, keys, opts| Set {
        name,
        keys,
        opts: Some(opts),
    };
    vec![
        Set {
            name: "default",
            keys: "",
            opts: None,
        },
        set(
            "O1",
            "gradient_face_correction = true\n",
            GradientOptions {
                face_correction: true,
                ..d
            },
        ),
        set(
            "O2",
            "gradient_boundary = \"corrected\"\n",
            GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        ),
        set(
            "O3",
            "gradient_boundary = \"dropped\"\n",
            GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        ),
        set(
            "O4",
            "gradient_secondary_limiter = \"barth_jespersen\"\n",
            GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        ),
        set(
            "O5",
            "gradient_secondary_limiter = \"venkatakrishnan\"\n",
            GradientOptions {
                secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
                ..d
            },
        ),
        set(
            "O6",
            "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
            GradientOptions {
                secondary_limiter: Some(Limiter::Venkatakrishnan(0.5)),
                ..d
            },
        ),
        set(
            "C2",
            "gradient_face_correction = true\ngradient_boundary = \"corrected\"\ngradient_secondary_limiter = \"venkatakrishnan\"\n",
            GradientOptions {
                face_correction: true,
                secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
                boundary: BoundaryGradient::Corrected,
            },
        ),
    ]
}

/// The four keys written at their defaults (D2).
const DEFAULT_KEYS: &str = "gradient_face_correction = false\ngradient_secondary_limiter = \"none\"\ngradient_limiter_k = 5.0\ngradient_boundary = \"owner\"\n";

/// Set-up C of ADR 0202 with the scheme `convection` and extra `[solver]` lines.
fn text(convection: &str, solver: &str) -> String {
    let reports: String = PATCHES
        .iter()
        .map(|p| format!("[reports.h_{p}]\ntype = \"heat_flow\"\npatch = \"{p}\"\n"))
        .collect();
    format!(
        r#"
title = "f289 conjugate balance"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
[energy]
enabled = true
relax = 0.9
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "{convection}"
frozen_flow = true
max_iterations = 3000
tolerance_u = 1e-13
tolerance_p = 1e-13
{solver}[solids.s]
zone = "box"
box_min = [-10.0, -10.0, -10.0]
box_max = [0.5, 10.0, 10.0]
conductivity = 1.0
density = 1.0
specific_heat = 1.0
[zone_sources.q]
target = "temperature"
zone = "box"
box_min = [-10.0, -10.0, -10.0]
box_max = [0.5, 10.0, 10.0]
value = 1.0
{reports}"#
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("case does not parse: {d:?}"))
}

/// A run of C through the tree: the simulation, its iteration count and the `heat_flow` reports in patch order.
fn tree(text: &str) -> (Simulation, usize, [f64; 4]) {
    let mut s = Simulation::new(&case(text)).unwrap_or_else(|d| panic!("not built: {d:?}"));
    let n = s.run(10_000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let vals = s.report_values();
    let h = PATCHES.map(|p| {
        let name = format!("h_{p}");
        vals.iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("report {name}"))
            .1
            .as_ref()
            .unwrap_or_else(|e| panic!("report {name}: {e}"))
            .value
    });
    (s, n, h)
}

/// What the test rebuilds of a finished run with `rcfd_fv` calls.
struct Rebuilt {
    t: ScalarField,
    label: Vec<usize>,
    k: Vec<f64>,
    gamma: Vec<f64>,
    /// Source in W of each group (fluid 0, solid 1) and in total.
    q: [f64; 2],
}

fn rebuild(geom: &FvGeometry, values: Vec<f64>) -> Rebuilt {
    let mut t = ScalarField::uniform("T", geom, 0.5);
    t.set_bc(geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
    for p in ["xmax", "ymax"] {
        t.set_bc(geom, p, Box::new(ZeroGradient)).unwrap();
    }
    t.values = values;
    let label: Vec<usize> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| usize::from(c[0] <= 0.5))
        .collect();
    let mats = ChtMaterials {
        rho_cp_ref: RHO_CP,
        k_cell: label.iter().map(|&l| if l == 1 { 1.0 } else { 0.1 }).collect(),
        rho_cp_cell: vec![RHO_CP; label.len()],
        contacts: vec![],
    };
    let vol = geom.mesh.cell_volumes();
    let q_solid: f64 = (0..label.len()).filter(|&c| label[c] == 1).map(|c| vol[c]).sum();
    Rebuilt {
        t,
        k: mats.face_conductivity(geom),
        gamma: mats.gamma(geom),
        label,
        q: [0.0, q_solid],
    }
}

impl Rebuilt {
    fn source(&self) -> f64 {
        self.q[1]
    }

    /// The residual `b + S V - A T` of the solved equation at the field's values (the convection part is zero on a zero
    /// flux), per cell.
    fn residual(&self, geom: &FvGeometry, opts: Option<GradientOptions>) -> Vec<f64> {
        let bc = self.t.boundary_coeffs(geom);
        let bvals = self.t.boundary_values(geom);
        let grad = least_squares(geom, &self.t.values, &bvals);
        let m = laplacian_opt(geom, &self.gamma, &bc, Some(&grad), &self.t.values, &bvals, opts);
        let mut at = vec![0.0; self.t.values.len()];
        m.a.matvec(&self.t.values, &mut at);
        let vol = geom.mesh.cell_volumes();
        (0..at.len())
            .map(|c| m.b[c] + (self.label[c] as f64) * (1.0 / RHO_CP) * vol[c] - at[c])
            .collect()
    }
}

/// Net flow out of every cell / (rho cp)_ref, and the largest cell sum of |flow| / (rho cp)_ref.
fn net(geom: &FvGeometry, flows: &[f64]) -> (Vec<f64>, f64) {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let n = geom.n_cells();
    let (mut out, mut abs) = (vec![0.0; n], vec![0.0; n]);
    for (f, q) in flows.iter().enumerate() {
        if f >= geom.n_internal && geom.empty[f] {
            continue;
        }
        out[own[f]] += q / RHO_CP;
        abs[own[f]] += q.abs() / RHO_CP;
        if f < geom.n_internal {
            out[nei[f]] -= q / RHO_CP;
            abs[nei[f]] += q.abs() / RHO_CP;
        }
    }
    (out, abs.iter().fold(0.0, |a, b| a.max(*b)))
}

fn patch_sums(geom: &FvGeometry, flows: &[f64]) -> [f64; 4] {
    PATCHES.map(|p| patch_heat_flow_out(geom, flows, p).unwrap())
}

/// The measured quantities of one set.
struct Measured {
    name: &'static str,
    iterations: usize,
    identity: f64,
    report: f64,
    report_bits_old: bool,
    tol_w: f64,
    tol_w_group: [f64; 2],
    f_abs: f64,
    q: f64,
    imbalance: f64,
    imbalance_old: f64,
    group: [f64; 2],
    group_old: [f64; 2],
    o1_bits: bool,
}

fn measure(set: &Set) -> Measured {
    let (s, iterations, h) = tree(&text("linear_upwind", set.keys));
    let geom = &s.geom;
    let rb = rebuild(geom, s.field("T").unwrap());
    let new = conductive_face_flows_opt(geom, &rb.t, &rb.k, true, set.opts);
    let old = conductive_face_flows(geom, &rb.t, &rb.k, true);
    let r = rb.residual(geom, set.opts);
    // E-tree: per cell, the net flow is A T - b of the operator (A T - b = S V - r)
    let (n, scale) = net(geom, &new);
    let vol = geom.mesh.cell_volumes();
    let identity = (0..n.len())
        .map(|c| (n[c] - ((rb.label[c] as f64) * vol[c] / RHO_CP - r[c])).abs())
        .fold(0.0, f64::max)
        / scale;
    let (h_new, h_old) = (patch_sums(geom, &new), patch_sums(geom, &old));
    let q = rb.source();
    let report_scale = q + h.iter().map(|x| x.abs()).sum::<f64>();
    let report = (0..4).map(|i| (h[i] - h_new[i]).abs()).fold(0.0, f64::max) / report_scale;
    let report_bits_old = (0..4).all(|i| h[i].to_bits() == h_old[i].to_bits());
    let o1_bits = (0..4).all(|i| h_new[i].to_bits() == h_old[i].to_bits());
    let tol_w = RHO_CP * r.iter().map(|x| x.abs()).sum::<f64>();
    let mut tol_w_group = [0.0; 2];
    for (c, x) in r.iter().enumerate() {
        tol_w_group[rb.label[c]] += RHO_CP * x.abs();
    }
    let f_abs: f64 = new
        .iter()
        .enumerate()
        .filter(|(f, _)| *f < geom.n_internal || !geom.empty[*f])
        .map(|(_, x)| x.abs())
        .sum();
    let g_new = group_net_outflow(geom, &new, &rb.label, 2);
    let g_old = group_net_outflow(geom, &old, &rb.label, 2);
    Measured {
        name: set.name,
        iterations,
        identity,
        report,
        report_bits_old,
        tol_w,
        tol_w_group,
        f_abs,
        q,
        imbalance: (h.iter().sum::<f64>() - q).abs(),
        imbalance_old: (h_old.iter().sum::<f64>() - q).abs(),
        group: [0, 1].map(|g| (g_new[g] - rb.q[g]).abs()),
        group_old: [0, 1].map(|g| (g_old[g] - rb.q[g]).abs()),
        o1_bits,
    }
}

/// P, E-tree, R-eq, B1, B2, B3 and the teeth T-B1, T-B3, T-O1 for the default and every option set.
#[test]
fn balance_of_the_reported_flows_for_every_option_set() {
    {
        let s = Simulation::new(&case(&text("linear_upwind", ""))).unwrap();
        let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
        let rb = rebuild(&s.geom, s.field("T").unwrap());
        let solid = rb.label.iter().filter(|&&l| l == 1).count();
        println!(
            "P: max non-orthogonality {:.3} degrees (at least {SKEWED}); {solid} of {} cells solid, Q = {:.6e} W",
            q.max_non_orthogonality,
            rb.label.len(),
            rb.source()
        );
        assert!(
            q.max_non_orthogonality >= SKEWED,
            "P set-up: {}",
            q.max_non_orthogonality
        );
        assert!(solid > 0 && rb.source() > 0.0, "P set-up: no solid cells");
    }
    let mut failures = Vec::new();
    for set in sets() {
        let m = measure(&set);
        let b1 = m.tol_w + ROUND_OFF * m.f_abs;
        let b3 = [0, 1].map(|g| m.tol_w_group[g] + ROUND_OFF * m.f_abs);
        println!(
            "{}: {} iterations; E-tree {:.3e} (limit {IDENTITY:e}); R-eq {:.3e} (limit {REPORT:e}){}",
            m.name,
            m.iterations,
            m.identity,
            m.report,
            if set.opts.is_none() {
                format!(", bits of the unchanged formula: {}", m.report_bits_old)
            } else {
                String::new()
            }
        );
        println!(
            "  B1 |sum H - Q| {:.3e} W (Q {:.6e} W) against tol_W {:.3e} + 1e-12 F {:.3e} = {b1:.3e}; B2 tol_W / Q {:.3e} (limit {CONVERGED:e})",
            m.imbalance,
            m.q,
            m.tol_w,
            ROUND_OFF * m.f_abs,
            m.tol_w / m.q
        );
        println!(
            "  B3 fluid {:.3e} against {:.3e}, solid {:.3e} against {:.3e}; unchanged formula: B1 {:.3e}, B3 fluid {:.3e}, solid {:.3e}; patch reports new = old bit for bit: {}",
            m.group[0], b3[0], m.group[1], b3[1], m.imbalance_old, m.group_old[0], m.group_old[1], m.o1_bits
        );
        let mut fail = |what: &str, ok: bool| {
            if !ok {
                failures.push(format!("{} {what}", m.name));
            }
        };
        fail("E-tree", m.identity <= IDENTITY);
        fail("R-eq", m.report <= REPORT);
        fail("B1", m.imbalance <= b1);
        fail("B2", m.tol_w <= CONVERGED * m.q);
        fail("B3", (0..2).all(|g| m.group[g] <= b3[g]));
        match m.name {
            "default" => fail("R-eq bits", m.report_bits_old),
            name => {
                if name != "O1" {
                    fail("T-B1", m.imbalance_old > b1);
                } else {
                    fail("T-O1", m.o1_bits);
                }
                fail("T-B3", (0..2).any(|g| m.group_old[g] > b3[g]));
            }
        }
    }
    assert!(failures.is_empty(), "missed: {failures:?}");
}

/// D2: the four keys written at their defaults change neither the settings hash nor a bit of a report or of `T`.
#[test]
fn d2_keys_at_their_defaults_change_nothing() {
    let (a, b) = (text("linear_upwind", ""), text("linear_upwind", DEFAULT_KEYS));
    let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
    let (sa, na, ra) = tree(&a);
    let (sb, nb, rb) = tree(&b);
    let (ta, tb) = (sa.field("T").unwrap(), sb.field("T").unwrap());
    let differ = ta.iter().zip(&tb).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let reports = (0..4).filter(|&i| ra[i].to_bits() != rb[i].to_bits()).count();
    println!(
        "D2: settings hash {ha:016x} / {hb:016x}; {na} / {nb} iterations; {differ} of {} cells and {reports} of 4 reports differ in a bit (limit 0)",
        ta.len()
    );
    assert_eq!(ha, hb, "D2 settings hash");
    assert_eq!(na, nb, "D2 iterations");
    assert_eq!(differ, 0, "D2 T");
    assert_eq!(reports, 0, "D2 reports");
}

/// The path of the first diagnostic of a refused case (parse or build), or "accepted".
fn refused_at(text: &str) -> String {
    match Case::parse_toml(text) {
        Err(d) => d.first().map(|x| x.path.clone()).unwrap_or_default(),
        Ok(c) => match Simulation::new(&c) {
            Err(d) => {
                println!("  {}: {}", d[0].path, d[0].message);
                d[0].path.clone()
            }
            Ok(_) => "accepted".into(),
        },
    }
}

/// L: the refusals of ADR 0190 hold on the conjugate set-up C; C and C with each set are accepted.
#[test]
fn l_located_errors_on_the_conjugate_case() {
    let mut checks: Vec<(String, String, &str)> = vec![
        (
            "upwind with O2".into(),
            text("upwind", "gradient_boundary = \"corrected\"\n"),
            "solver.gradient_boundary",
        ),
        (
            "central with O1".into(),
            text("central", "gradient_face_correction = true\n"),
            "solver.gradient_face_correction",
        ),
        (
            "K without its limiter".into(),
            text("linear_upwind", "gradient_limiter_k = 1.0\n"),
            "solver.gradient_limiter_k",
        ),
        (
            "unknown limiter".into(),
            text("linear_upwind", "gradient_secondary_limiter = \"minmod\"\n"),
            "solver.gradient_secondary_limiter",
        ),
    ];
    for set in sets() {
        checks.push((
            format!("control {}", set.name),
            text("linear_upwind", set.keys),
            "accepted",
        ));
    }
    for (what, text, want) in checks {
        let got = refused_at(&text);
        println!("L {what}: {got} (want {want})");
        assert_eq!(got, want, "L {what}");
    }
}
