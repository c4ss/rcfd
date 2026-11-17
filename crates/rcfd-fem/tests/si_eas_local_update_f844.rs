//! F-844 (ADR 0725): the local mode update of the incompatible Hex8 as a default, measured. The warm start of ADR 0660 D1
//! (WS), the backtracking of ADR 0655 D1 (BT) and the trust region of ADR 0660 D2 (TR), each alone as the would-be default
//! beside today's default (P), on the eleven set-ups of ADR 0658 (module `setups`: solves passed, iterations, the
//! reproductions RP, RB, RT and the controls NCW, NCT, NCW0) and on ADR 0723's fine trace from rung 4 of rule L's `16^3`
//! punch branch with no element states beyond the driver's own (module `trace`, three heavy records: the stop, the reading,
//! the iterations, and at every accepted state the elements where the plain iteration from `alpha = 0` fails at a regular
//! root). The fast local tally runs the trace's tally on ADR 0660's stall state of seed `0xF739_0003`. The helpers are copies
//! of those of `si_eas_hg8.rs` and `si_eas_hg20.rs` (existing tests are not edited). No default is changed. Every aggregated
//! value is required finite before it is compared.

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn equals(&mut self, name: &str, value: usize, expected: usize) {
        let ok = value == expected;
        println!(
            "{name}: {value} (expected {expected}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} != {expected}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The element named by an error text (`... element <e>: ...`), the first occurrence.
fn element_named(text: &str) -> Option<usize> {
    let i = text.find("element ")?;
    let digits: String = text[i + 8..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The four variants of ADR 0725 reading c1: today's default and each opt-in alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Var {
    P,
    Ws,
    Bt,
    Tr,
}

const VARS: [Var; 4] = [Var::P, Var::Ws, Var::Bt, Var::Tr];

impl Var {
    fn name(self) -> &'static str {
        match self {
            Var::P => "P (default)",
            Var::Ws => "WS (warm start)",
            Var::Bt => "BT (backtracking)",
            Var::Tr => "TR (trust region)",
        }
    }
    /// The variant's opt-in on a model (and nothing else).
    fn apply(self, m: &mut rcfd_fem::SolidModel) {
        m.set_incompatible_mode_warm_start(self == Var::Ws);
        m.set_incompatible_mode_backtracking(self == Var::Bt);
        m.set_incompatible_mode_trust_region(self == Var::Tr);
    }
}

// ============================================================================ the set-ups (ADR 0658; ADR 0725 D1, D3)

mod setups {
    use super::{element_named, Checks, Var, VARS};
    use crate::trace;
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::builders::{hex_box, BOX_FACES};
    use rcfd_fem::material::HyperelasticEnergy;
    use rcfd_fem::nl_incompatible::{IncompatibleModeUpdate, IncompatibleStabilisation};
    use rcfd_fem::nonlinear::{solve, tangent, NewtonOptions, NonlinearResult};
    use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
    use rcfd_linalg::eigen::{count_below, lowest_eigenpairs, SubspaceOptions};
    use rcfd_linalg::skyline::SkylineOrdering;
    use rcfd_linalg::CsrMatrix;
    use std::collections::BTreeSet;
    use std::time::Instant;

    /// The smallest of finite values; NaN if any is not finite or there is none.
    fn least<I: IntoIterator<Item = f64>>(values: I) -> f64 {
        let mut w = f64::INFINITY;
        for v in values {
            if !v.is_finite() {
                return f64::NAN;
            }
            w = w.min(v);
        }
        if w == f64::INFINITY {
            f64::NAN
        } else {
            w
        }
    }

    /// SplitMix64 from a seed.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Rng {
            Rng(seed)
        }
        fn uniform(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
        }
    }

    fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
        let mut out = SolidMesh::new(nodes).unwrap();
        for b in mesh.blocks() {
            out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
        }
        for name in mesh.node_set_names() {
            out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
        }
        for name in mesh.element_set_names() {
            out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
                .unwrap();
        }
        for name in mesh.surface_set_names() {
            out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
                .unwrap();
        }
        out
    }

    fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
        if f != SolidFormulation::Full {
            mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
        }
        mesh
    }
    fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
        mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
    }

    fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
        let mut s = BTreeSet::new();
        for f in BOX_FACES {
            s.extend(mesh.node_set(f).unwrap().iter().copied());
        }
        s
    }

    // ============================================================================ the distorted meshes (ADR 0649 D2)

    /// The interior nodes (on none of the six box faces) moved by `amp h_c (2U - 1)` per component, `h_c = lengths[c] / div[c]`,
    /// three draws per interior node in node order; returns the mesh and the number of moved nodes.
    fn distort(mesh: &SolidMesh, lengths: Vec3, div: [usize; 3], amp: f64, seed: u64) -> (SolidMesh, usize) {
        let boundary = boundary_nodes(mesh);
        let mut rng = Rng::new(seed);
        let h: [f64; 3] = std::array::from_fn(|c| lengths[c] / div[c] as f64);
        let mut nodes = mesh.nodes().to_vec();
        let mut moved = 0;
        for (i, p) in nodes.iter_mut().enumerate() {
            if !boundary.contains(&i) {
                moved += 1;
                for (c, v) in p.iter_mut().enumerate() {
                    *v += amp * h[c] * (2.0 * rng.uniform() - 1.0);
                }
            }
        }
        (rebuild(mesh, nodes), moved)
    }

    /// The reference corners of the Hex8 (node order of `hex_box`).
    const XI: [[f64; 3]; 8] = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];

    /// `det J` of the trilinear map of an element at a reference point.
    fn det_jacobian(x: &[Vec3], xi: [f64; 3]) -> f64 {
        let mut j = [[0.0; 3]; 3];
        for (a, s) in XI.iter().enumerate() {
            let f: [f64; 3] = std::array::from_fn(|k| 1.0 + s[k] * xi[k]);
            let d = [
                s[0] * f[1] * f[2] / 8.0,
                f[0] * s[1] * f[2] / 8.0,
                f[0] * f[1] * s[2] / 8.0,
            ];
            for r in 0..3 {
                for c in 0..3 {
                    j[r][c] += x[a][r] * d[c];
                }
            }
        }
        j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
            + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0])
    }

    /// M0: the smallest corner `det J` over the element centre's, over the mesh.
    fn min_corner_ratio(mesh: &SolidMesh) -> f64 {
        least((0..mesh.n_elements()).flat_map(|e| {
            let x = element_coords(mesh, e);
            let d0 = det_jacobian(&x, [0.0; 3]);
            XI.iter().map(move |&c| det_jacobian(&x, c) / d0).collect::<Vec<_>>()
        }))
    }

    // ============================================================================ the material and the elements

    const MU: f64 = 1.0e6;
    const NU: f64 = 0.49;
    const INC: SolidFormulation = SolidFormulation::Incompatible;
    const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;

    fn kappa_of(nu: f64) -> f64 {
        2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
    }

    fn material(nu: f64) -> Material {
        Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(nu)).unwrap(), 1000.0).unwrap()
    }

    /// The nodes of a set whose coordinates pass a filter.
    fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
        mesh.node_set(set)
            .unwrap()
            .iter()
            .copied()
            .filter(|&n| keep(&mesh.nodes()[n]))
            .collect()
    }

    /// The sum of the driver's reactions over a node set's component.
    fn reaction_sum(r: &NonlinearResult, nodes: &[usize], comp: usize) -> f64 {
        let re = r.result.reactions().unwrap();
        let dofs = r.result.dofs();
        nodes.iter().map(|&n| re[dofs.equation(n, comp).unwrap()]).sum()
    }

    /// A model with its prescribed equations, its solve and the two measures of the load case.
    struct Run {
        model: SolidModel,
        prescribed: BTreeSet<usize>,
        result: Option<NonlinearResult>,
        a: f64,
        b: f64,
        error: Option<String>,
    }

    fn constrain(m: &mut SolidModel, bcs: &[(&str, usize, f64)]) -> BTreeSet<usize> {
        let mut prescribed = BTreeSet::new();
        for &(set, comp, value) in bcs {
            m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
            for &nd in m.mesh().node_set(set).unwrap() {
                prescribed.insert(m.dof_map().equation(nd, comp).unwrap());
            }
        }
        prescribed
    }

    // ============================================================================ the stability of the loaded state (SL)

    /// The free block of the tangent at the converged state, as a CSR matrix.
    fn free_tangent(run: &Run) -> Option<CsrMatrix> {
        let r = run.result.as_ref()?;
        let k = tangent(&run.model, r.result.displacement_values(), &r.states, Some(4)).ok()?;
        let free: Vec<usize> = (0..k.n).filter(|e| !run.prescribed.contains(e)).collect();
        let mut pos = vec![usize::MAX; k.n];
        for (i, &e) in free.iter().enumerate() {
            pos[e] = i;
        }
        let mut t = Vec::new();
        for (row, &e) in free.iter().enumerate() {
            for p in k.row_ptr[e]..k.row_ptr[e + 1] {
                let c = pos[k.col[p]];
                if c != usize::MAX {
                    t.push((row, c, k.val[p]));
                }
            }
        }
        Some(CsrMatrix::from_triplets(free.len(), t))
    }

    /// SL: `(negative eigenvalues of K_ff, lowest eigenvalue / largest diagonal)`; NaN where a step fails.
    fn stability(run: &Run) -> (f64, f64) {
        let Some(kff) = free_tangent(run) else {
            return (f64::NAN, f64::NAN);
        };
        let n = kff.n;
        let dmax = kff.diag().iter().fold(0.0f64, |w, v| w.max(v.abs()));
        let id = CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect());
        let neg = count_below(&kff, &id, 0.0, &SkylineOrdering::Rcm)
            .map(|c| c as f64)
            .unwrap_or(f64::NAN);
        let low = lowest_eigenpairs(&kff, &id, 1, &SubspaceOptions::default())
            .map(|e| e.values[0] / dmax)
            .unwrap_or(f64::NAN);
        (neg, low)
    }

    fn rel(a: f64, b: f64) -> f64 {
        (a / b - 1.0).abs()
    }

    /// The bitwise difference count of two results' displacements and reactions; `usize::MAX` if one is missing.
    fn bit_differences(a: Option<&NonlinearResult>, b: Option<&NonlinearResult>) -> usize {
        let (Some(a), Some(b)) = (a, b) else {
            return usize::MAX;
        };
        let ua = a.result.displacement_values();
        let ub = b.result.displacement_values();
        let ra = a.result.reactions().unwrap();
        let rb = b.result.reactions().unwrap();
        if ua.len() != ub.len() || ra.len() != rb.len() {
            return usize::MAX;
        }
        ua.iter().zip(ub).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
            + ra.iter().zip(rb).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
    }

    // ============================================================================ the load cases (ADR 0636 readings c1, c2)

    const DELTA: f64 = 0.1;
    const CANT_L: f64 = 1.0;
    const CANT_H: f64 = 0.2;
    const TIP_V: f64 = 0.3;

    /// The cantilever mesh `nx x ny x ny` moved at amplitude `amp` with `seed`.
    fn cant_mesh(nx: usize, ny: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
        let mesh = hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [nx, ny, ny]).unwrap();
        distort(&mesh, [CANT_L, CANT_H, CANT_H], [nx, ny, ny], amp, seed)
    }

    /// The step record of a solve, printed.
    fn steps_text(r: &NonlinearResult) -> String {
        let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
        let searched = r
            .steps
            .iter()
            .flat_map(|s| s.step_lengths.iter())
            .filter(|&&l| l < 1.0)
            .count();
        let shortest = r
            .steps
            .iter()
            .flat_map(|s| s.step_lengths.iter().copied())
            .fold(f64::INFINITY, f64::min);
        format!(
            "increments {}, halved {halved}, step lengths < 1: {searched} (shortest {shortest:.4e})",
            r.steps.len()
        )
    }
    /// [`rebuild`] with the constructor's refusal returned instead of a panic (A1.1).
    fn rebuild_checked(mesh: &SolidMesh, nodes: Vec<Vec3>) -> Result<SolidMesh, String> {
        let mut out = SolidMesh::new(nodes).map_err(|e| format!("{e:?}"))?;
        for b in mesh.blocks() {
            out.add_block(b.kind(), b.connectivity().to_vec())
                .map_err(|e| format!("{e:?}"))?;
        }
        for name in mesh.node_set_names() {
            out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
        }
        for name in mesh.element_set_names() {
            out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
                .unwrap();
        }
        for name in mesh.surface_set_names() {
            out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
                .unwrap();
        }
        Ok(out)
    }

    /// The block mesh of [`block_mesh`] with the constructor's refusal of an inverted mesh returned (A1.1).
    fn block_mesh_checked(n: usize, amp: f64, seed: u64) -> Result<(SolidMesh, usize), String> {
        let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
        let tol = 1e-9;
        let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + tol && p[1] <= 0.5 + tol);
        let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < tol && (p[1] - 1.0).abs() < tol);
        mesh.add_node_set("punch", punch).unwrap();
        mesh.add_node_set("corner", corner).unwrap();
        let boundary = boundary_nodes(&mesh);
        let mut rng = Rng::new(seed);
        let h = 1.0 / n as f64;
        let mut nodes = mesh.nodes().to_vec();
        let mut moved = 0;
        for (i, p) in nodes.iter_mut().enumerate() {
            if !boundary.contains(&i) {
                moved += 1;
                for v in p.iter_mut() {
                    *v += amp * h * (2.0 * rng.uniform() - 1.0);
                }
            }
        }
        Ok((rebuild_checked(&mesh, nodes)?, moved))
    }

    // ======================================================================== ADR 0725 D1: the runs of the variants

    /// The model of rule L on a mesh with the variant's opt-in.
    fn model_of(mesh: SolidMesh, var: Var) -> SolidModel {
        let mut m = SolidModel::new(with_formulation(mesh, INC));
        m.set_incompatible_stabilisation(LIN);
        var.apply(&mut m);
        m.add_material("all", material(NU)).unwrap();
        m
    }

    /// The driver's defaults with 10 increments and 4 threads (reading c1), the line search only for ADR 0658's R (D3).
    fn newton(line_search: bool) -> NewtonOptions {
        NewtonOptions::new(10).threads(4).line_search(line_search)
    }

    /// The iterations of a converged solve (reading c5): increments, halved increments, halvings, global Newton iterations.
    fn iterations(r: &NonlinearResult) -> [usize; 4] {
        [
            r.steps.len(),
            r.steps.iter().filter(|s| s.halvings > 0).count(),
            r.steps.iter().map(|s| s.halvings).sum(),
            r.steps.iter().map(|s| s.residuals.len().saturating_sub(1)).sum(),
        ]
    }

    /// The block of ADR 0636 reading c1 on a mesh, the punch at `delta`, 10 increments; `(R, w_c)` or NaN with the error.
    fn run_block(mesh: &SolidMesh, tag: &str, delta: f64, var: Var, line_search: bool) -> Run {
        let mut m = model_of(mesh.clone(), var);
        let prescribed = constrain(
            &mut m,
            &[
                ("xmin", 0, 0.0),
                ("xmax", 0, 0.0),
                ("ymin", 1, 0.0),
                ("ymax", 1, 0.0),
                ("zmin", 2, 0.0),
                ("punch", 2, -delta),
            ],
        );
        let t0 = Instant::now();
        let (result, a, b, error) = match solve(&m, &[], &[], &newton(line_search)) {
            Ok(r) => {
                let punch = m.mesh().node_set("punch").unwrap().to_vec();
                let corner = m.mesh().node_set("corner").unwrap()[0];
                let rz = -reaction_sum(&r, &punch, 2) / MU;
                let wc = r.result.displacement(corner)[2];
                println!(
                    "{tag} delta {delta} {}: R/mu {rz:.8}, w_c {wc:+.8}, {}, {:.1} s",
                    var.name(),
                    steps_text(&r),
                    t0.elapsed().as_secs_f64()
                );
                (Some(r), rz, wc, None)
            }
            Err(e) => {
                println!(
                    "{tag} delta {delta} {}: solve failed: {e}, {:.1} s",
                    var.name(),
                    t0.elapsed().as_secs_f64()
                );
                (None, f64::NAN, f64::NAN, Some(e.to_string()))
            }
        };
        Run {
            model: m,
            prescribed,
            result,
            a,
            b,
            error,
        }
    }

    /// The cantilever of ADR 0636 reading c2 on a mesh; `(P, d)` or NaN with the error.
    fn run_cantilever(mesh: &SolidMesh, tag: &str, var: Var) -> Run {
        let mut m = model_of(mesh.clone(), var);
        let mut bcs: Vec<(&str, usize, f64)> = (0..3).map(|c| ("xmin", c, 0.0)).collect();
        bcs.push(("xmax", 1, TIP_V));
        let prescribed = constrain(&mut m, &bcs);
        let t0 = Instant::now();
        let (result, a, b, error) = match solve(&m, &[], &[], &newton(false)) {
            Ok(r) => {
                let tip = m.mesh().node_set("xmax").unwrap().to_vec();
                let p = reaction_sum(&r, &tip, 1) / MU;
                let d = -tip.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / tip.len() as f64;
                println!(
                    "{tag} {}: P/mu {p:.8e}, d {d:.8e}, {}, {:.1} s",
                    var.name(),
                    steps_text(&r),
                    t0.elapsed().as_secs_f64()
                );
                (Some(r), p, d, None)
            }
            Err(e) => {
                println!(
                    "{tag} {}: solve failed: {e}, {:.1} s",
                    var.name(),
                    t0.elapsed().as_secs_f64()
                );
                (None, f64::NAN, f64::NAN, Some(e.to_string()))
            }
        };
        Run {
            model: m,
            prescribed,
            result,
            a,
            b,
            error,
        }
    }

    /// Reading c4 on a set-up: converged, the two errors against the recorded F-bar values, SL, and "passed".
    struct Pass {
        passed: bool,
        ea: f64,
        eb: f64,
        neg: f64,
        low: f64,
    }

    fn pass_of(run: &Run, fbar: (f64, f64), tol: f64) -> Pass {
        if run.result.is_none() {
            return Pass {
                passed: false,
                ea: f64::NAN,
                eb: f64::NAN,
                neg: f64::NAN,
                low: f64::NAN,
            };
        }
        let (ea, eb) = (rel(run.a, fbar.0), rel(run.b, fbar.1));
        let (neg, low) = stability(run);
        let passed = ea.is_finite() && ea <= tol && eb.is_finite() && eb <= tol && neg == 0.0 && low > 0.0;
        Pass {
            passed,
            ea,
            eb,
            neg,
            low,
        }
    }

    /// A set-up of reading c2 by its ID.
    struct Setup {
        id: &'static str,
        tag: String,
        mesh: SolidMesh,
        block: bool,
        tol: f64,
        fbar: (f64, f64),
    }

    /// The four variants on a set-up: the runs and their readings, each printed as a table row.
    fn run_setup(s: &Setup) -> Vec<(Var, Run, Pass)> {
        VARS.iter()
            .map(|&v| {
                let run = if s.block {
                    run_block(&s.mesh, &s.tag, DELTA, v, false)
                } else {
                    run_cantilever(&s.mesh, &s.tag, v)
                };
                let p = pass_of(&run, s.fbar, s.tol);
                let its = run
                    .result
                    .as_ref()
                    .map(|r| format!("{:?}", iterations(r)))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "TABLE SETUP {} {} | {} | converged {} | errors {:.4e} {:.4e} (limit {}) | SL negative {} lowest/max diag {:.4e} | \
                     passed {} | increments, halved, halvings, Newton iterations {its} | element named {:?}",
                    s.id,
                    s.tag,
                    v.name(),
                    run.result.is_some(),
                    p.ea,
                    p.eb,
                    s.tol,
                    p.neg,
                    p.low,
                    p.passed,
                    run.error.as_deref().and_then(element_named)
                );
                (v, run, p)
            })
            .collect()
    }

    fn of(rows: &[(Var, Run, Pass)], v: Var) -> &(Var, Run, Pass) {
        rows.iter().find(|r| r.0 == v).unwrap()
    }

    /// The per-variant summary of a test: the passes and the Newton iterations over the set-ups all four variants pass.
    /// The rows of one set-up (its ID and the four variants).
    type Rows<'a> = (&'a str, Vec<(Var, Run, Pass)>);

    fn summary(test: &str, all: &[Rows]) {
        for v in VARS {
            let passed: Vec<&str> = all
                .iter()
                .filter(|(_, r)| of(r, v).2.passed)
                .map(|(id, _)| *id)
                .collect();
            println!(
                "SUMMARY {test} {}: passed {} of {}: {passed:?}",
                v.name(),
                passed.len(),
                all.len()
            );
        }
        let common: Vec<&Rows> = all
            .iter()
            .filter(|(_, r)| VARS.iter().all(|&v| of(r, v).2.passed))
            .collect();
        for v in VARS {
            let n: usize = common
                .iter()
                .map(|(_, r)| of(r, v).1.result.as_ref().map(|x| iterations(x)[3]).unwrap_or(0))
                .sum();
            println!(
                "SUMMARY {test} {}: Newton iterations over the {} set-ups all four pass: {n}",
                v.name(),
                common.len()
            );
        }
    }

    /// RP0, RT0: a variant's "passed" on a set-up against the expected reading.
    fn expect_pass(ch: &mut Checks, row: &str, s: &Setup, rows: &[(Var, Run, Pass)], v: Var, expected: bool) {
        ch.equals(
            &format!("{row} {} {}: {} passed (1 = yes)", s.id, s.tag, v.name()),
            of(rows, v).2.passed as usize,
            expected as usize,
        );
    }

    /// RP1, RB0, RT0: the element named by a failed variant's error.
    fn expect_element(ch: &mut Checks, row: &str, s: &Setup, rows: &[(Var, Run, Pass)], v: Var, e: usize) {
        let named = of(rows, v).1.error.as_deref().and_then(element_named);
        ch.equals(
            &format!("{row} {} {}: {} the element named by the error", s.id, s.tag, v.name()),
            named.unwrap_or(usize::MAX),
            e,
        );
    }

    fn bits(a: &Run, b: &Run) -> f64 {
        match bit_differences(a.result.as_ref(), b.result.as_ref()) {
            usize::MAX => f64::NAN,
            n => n as f64,
        }
    }

    fn mesh_rows(ch: &mut Checks, tag: &str, mesh: &SolidMesh, count: usize, moved: usize) {
        ch.above(
            &format!("M0 {tag}: min corner det J / centre det J"),
            min_corner_ratio(mesh),
            0.0,
        );
        ch.equals(&format!("M1 {tag}: moved nodes"), count, moved);
    }

    // ======================================================================== the set-up tests

    #[test]
    /// ADR 0725 D1 on the four block meshes of ADR 0649 (DB1 to DB4): RP0, RP1, RB0, RB1, RT0; WS measured.
    fn f844_setups_blocks() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let list = [
            ("DB1", 4, 0.1, 0xF719_0004, 0.15, 27, (0.38349042, 0.07137650)),
            ("DB2", 4, 0.25, 0xF719_0004, 0.15, 27, (0.38316202, 0.07044819)),
            ("DB3", 8, 0.1, 0xF719_0008, 0.10, 343, (0.34263132, 0.06370837)),
            ("DB4", 8, 0.25, 0xF719_0008, 0.10, 343, (0.34220247, 0.06389947)),
        ];
        let mut all = Vec::new();
        for (id, n, amp, seed, tol, moved, fbar) in list {
            let tag = format!("block {n}^3 a {amp}");
            let (mesh, count) = block_mesh_checked(n, amp, seed).unwrap();
            mesh_rows(&mut ch, &tag, &mesh, count, moved);
            let s = Setup {
                id,
                tag,
                mesh,
                block: true,
                tol,
                fbar,
            };
            let rows = run_setup(&s);
            let db4 = id == "DB4";
            expect_pass(&mut ch, "RP0", &s, &rows, Var::P, !db4);
            expect_pass(&mut ch, "RT0", &s, &rows, Var::Tr, true);
            if db4 {
                expect_element(&mut ch, "RP1", &s, &rows, Var::P, 476);
                ch.equals(
                    &format!("RB0 {} {}: BT failed (1 = yes)", s.id, s.tag),
                    of(&rows, Var::Bt).1.result.is_none() as usize,
                    1,
                );
                expect_element(&mut ch, "RB0", &s, &rows, Var::Bt, 475);
            } else {
                ch.at_most(
                    &format!("RB1 {} {}: BT against P, values differing in bits", s.id, s.tag),
                    bits(&of(&rows, Var::Bt).1, &of(&rows, Var::P).1),
                    0.0,
                );
            }
            all.push((id, rows));
        }
        summary("blocks", &all);
        println!("f844_setups_blocks: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }

    #[test]
    /// ADR 0725 D1 on the four cantilever meshes of ADR 0649 (DC1 to DC4): RP0, RB1, RT0, NCW, NCT, NCW0; WS measured.
    fn f844_setups_cantilevers() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let list = [
            ("DC1", 10, 2, 0.1, 0xF719_1010, 0.15, 9, (6.17271412e-4, 5.24022571e-2)),
            ("DC2", 10, 2, 0.25, 0xF719_1010, 0.15, 9, (6.19924741e-4, 5.23693020e-2)),
            (
                "DC3",
                20,
                4,
                0.1,
                0xF719_1020,
                0.10,
                171,
                (5.72841645e-4, 5.23430902e-2),
            ),
            (
                "DC4",
                20,
                4,
                0.25,
                0xF719_1020,
                0.10,
                171,
                (5.75783694e-4, 5.23261081e-2),
            ),
        ];
        let mut all = Vec::new();
        for (id, nx, ny, amp, seed, tol, moved, fbar) in list {
            let tag = format!("cantilever {nx}x{ny}x{ny} a {amp}");
            let (mesh, count) = cant_mesh(nx, ny, amp, seed);
            mesh_rows(&mut ch, &tag, &mesh, count, moved);
            let s = Setup {
                id,
                tag,
                mesh,
                block: false,
                tol,
                fbar,
            };
            let rows = run_setup(&s);
            expect_pass(&mut ch, "RP0", &s, &rows, Var::P, true);
            expect_pass(&mut ch, "RT0", &s, &rows, Var::Tr, true);
            ch.at_most(
                &format!("RB1 {} {}: BT against P, values differing in bits", s.id, s.tag),
                bits(&of(&rows, Var::Bt).1, &of(&rows, Var::P).1),
                0.0,
            );
            if id == "DC2" {
                let p = &of(&rows, Var::P).1;
                ch.above(
                    "NCW DC2: WS against P, values differing in bits",
                    bits(&of(&rows, Var::Ws).1, p),
                    0.0,
                );
                ch.above(
                    "NCT DC2: TR against P, values differing in bits",
                    bits(&of(&rows, Var::Tr).1, p),
                    0.0,
                );
                let with_values = p
                    .result
                    .as_ref()
                    .map(|r| r.states.iter().filter(|st| !st.values().is_empty()).count() as f64)
                    .unwrap_or(f64::NAN);
                ch.at_most(
                    "NCW0 DC2: P's elements whose committed state has a value",
                    with_values,
                    0.0,
                );
            }
            all.push((id, rows));
        }
        summary("cantilevers", &all);
        println!("f844_setups_cantilevers: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }

    /// The first three seeds from `first` whose `8^3` mesh at `0.25 h` passes M0 (ADR 0658 reading c3, ADR 0660 A1.1).
    fn seeds_from_checked(first: u64) -> Vec<(u64, SolidMesh, usize)> {
        let mut out = Vec::new();
        let mut seed = first;
        while out.len() < 3 {
            match block_mesh_checked(8, 0.25, seed) {
                Ok((mesh, count)) if min_corner_ratio(&mesh) > 0.0 => out.push((seed, mesh, count)),
                Ok(_) => println!("seed {seed:#X}: M0 <= 0, not a valid mesh; replaced by the next seed"),
                Err(e) => println!("seed {seed:#X}: the mesh is refused as inverted ({e}); replaced by the next seed"),
            }
            seed += 1;
        }
        out
    }

    #[test]
    /// ADR 0725 D1 on the `8^3` block at `0.25 h` with the seeds of ADR 0658 (DS1 to DS3): RP0, RP1, RT0; WS, BT measured.
    fn f844_setups_seeds() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let fbar = [
            (0.34387169, 0.06336271),
            (0.34915749, 0.06430598),
            (0.34360196, 0.06344073),
        ];
        let ids = ["DS1", "DS2", "DS3"];
        let p_elements = [475, 420, 476];
        let mut all = Vec::new();
        for (i, (seed, mesh, count)) in seeds_from_checked(0xF739_0001).into_iter().enumerate() {
            let tag = format!("block 8^3 a 0.25 seed {seed:#X}");
            mesh_rows(&mut ch, &tag, &mesh, count, 343);
            let s = Setup {
                id: ids[i],
                tag,
                mesh,
                block: true,
                tol: 0.10,
                fbar: fbar[i],
            };
            let rows = run_setup(&s);
            expect_pass(&mut ch, "RP0", &s, &rows, Var::P, false);
            expect_element(&mut ch, "RP1", &s, &rows, Var::P, p_elements[i]);
            expect_pass(&mut ch, "RT0", &s, &rows, Var::Tr, i == 0);
            if i > 0 {
                expect_element(&mut ch, "RT0", &s, &rows, Var::Tr, p_elements[i]);
            }
            all.push((ids[i], rows));
        }
        summary("seeds", &all);
        println!("f844_setups_seeds: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }

    // ======================================================================== ADR 0725 D3: the fast local tally

    #[test]
    /// LT1 to LT5: the tally of reading c6 at ADR 0660's trial state `u_T` of seed `0xF739_0003` (all 512 elements), the
    /// previous roots those of the cold `Backtracking` record at `u_b`.
    fn f844_local_tally_at_the_stall_of_seed_3() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let (seed, e, pa, pb, pt) = (0xF739_0003u64, 476usize, 0.0850, 0.0855, 0.0858);
        let tag = format!("seed {seed:#X}");
        let (mesh, _) = block_mesh_checked(8, 0.25, seed).unwrap();
        let ra = run_block(&mesh, &format!("D3 {tag} R"), pa, Var::Bt, true);
        let rb = run_block(&mesh, &format!("D3 {tag} R"), pb, Var::Bt, true);
        let (Some(a), Some(b)) = (ra.result.as_ref(), rb.result.as_ref()) else {
            ch.holds("D3: the two solves of R converge", false);
            ch.finish();
            return;
        };
        let (ua, ub) = (a.result.displacement_values(), b.result.displacement_values());
        let ut: Vec<f64> = ub
            .iter()
            .zip(ua)
            .map(|(q, p)| q + (pt - pb) / (pb - pa) * (q - p))
            .collect();
        let m = &rb.model;
        let mat = material(NU);
        let states = b.states.clone();
        let prev = trace::roots(m, &mat, ub, &states, IncompatibleModeUpdate::Backtracking);
        println!(
            "D3: previous roots at u_b: {} of {} elements converged",
            prev.iter().filter(|r| r.is_some()).count(),
            prev.len()
        );
        let t = trace::tally(
            m,
            &mat,
            &ut,
            &states,
            Some(&prev),
            trace::Opt::Update(IncompatibleModeUpdate::Backtracking),
            true,
        );
        trace::print_tally(&format!("LT5 {tag} u_T"), &t);
        // LT1 to LT3: element 476 read directly.
        let el = trace::element_of(m, e, &ut, &states);
        let bt = trace::rec(&mat, &el, IncompatibleModeUpdate::Backtracking, None);
        let tr = trace::rec(&mat, &el, IncompatibleModeUpdate::TrustRegion, None);
        let pl = trace::rec(&mat, &el, IncompatibleModeUpdate::Plain, None);
        let cont = prev[e].map(|p| trace::rec(&mat, &el, IncompatibleModeUpdate::Plain, Some(&p)));
        println!(
            "D3 element {e} at u_T: plain from 0 {} iterates, {}; BT from 0 {} iterates, {}; TR from 0 {} iterates, {}",
            pl.0,
            trace::text(&pl.1),
            bt.0,
            trace::text(&bt.1),
            tr.0,
            trace::text(&tr.1)
        );
        ch.equals(
            &format!("LT1 element {e}: BT from alpha = 0 at u_T fails (1 = yes)"),
            bt.1.is_err() as usize,
            1,
        );
        ch.equals(
            &format!("LT2 element {e}: TR from alpha = 0 at u_T fails (1 = yes)"),
            tr.1.is_err() as usize,
            1,
        );
        match &cont {
            Some((n, out)) => {
                println!(
                    "D3 element {e}: the continuation from the root at u_b: {n} iterates, {}",
                    trace::text(out)
                );
                ch.equals(
                    &format!("LT3 element {e}: the continuation from the root at u_b converges (1 = yes)"),
                    out.is_ok() as usize,
                    1,
                );
            }
            None => ch.holds(&format!("LT3 element {e}: a root at u_b to continue from"), false),
        }
        match t.entries.iter().find(|x| x.e == e) {
            Some(x) => ch.holds(
                &format!("LT4 element {e}: the tally's entry agrees with LT1 to LT3"),
                x.opt.is_err() == bt.1.is_err()
                    && x.beside.map(|b| b.1 .0) == Some(tr.1.is_ok())
                    && x.cont.as_ref().map(|c| c.is_ok()) == cont.as_ref().map(|c| c.1.is_ok()),
            ),
            None => println!("LT4 element {e}: not applicable (the plain iteration from 0 converges there)"),
        }
        println!(
            "f844_local_tally_at_the_stall_of_seed_3: {:.1} s",
            t_all.elapsed().as_secs_f64()
        );
        ch.finish();
    }
}

// ============================================================================ the trace (ADR 0723 D1, D2; ADR 0725 D2, D3)

mod trace {
    use super::{Checks, Var};
    use rcfd_core::vec3::Vec3;
    use rcfd_fem::builders::hex_box;
    use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
    use rcfd_fem::element::ElementState;
    use rcfd_fem::material::{HyperelasticEnergy, PointContext};
    use rcfd_fem::nl_incompatible::{
        condensed_force_and_tangent_warm, enhanced_force_and_tangent_with, mode_iteration_record_from,
        IncompatibleModeUpdate, IncompatibleStabilisation, ModeIterate,
    };
    use rcfd_fem::nonlinear::{
        initial_states, internal_force, solve_from, solve_increment, solve_with, tangent, Contribution,
        DriverExtensions, LoadStep, NewtonOptions, StepControl,
    };
    use rcfd_fem::solve::DirectFactor;
    use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
    use rcfd_linalg::eigen::{
        lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, ResidualTest, Shift, SubspaceOptions,
    };
    use rcfd_linalg::skyline::PivotRule;
    use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalLdlt};
    use rcfd_linalg::CsrMatrix;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Instant;

    fn norm(v: impl Iterator<Item = f64>) -> f64 {
        v.map(|x| x * x).sum::<f64>().sqrt()
    }

    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
        v.fold(0.0f64, |w, x| if x.is_finite() { w.max(x.abs()) } else { f64::NAN })
    }

    // ============================================================================ the materials and the block (ADR 0694)

    const MU: f64 = 1.0e6;
    const NU: f64 = 0.49;
    const INC: SolidFormulation = SolidFormulation::Incompatible;
    const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;

    fn kappa_of(nu: f64) -> f64 {
        2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
    }

    /// The neo-Hookean material of ADR 0636 (density 1000).
    fn neo_hookean() -> Material {
        Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(NU)).unwrap(), 1000.0).unwrap()
    }

    #[derive(Clone, Copy)]
    struct Kind {
        name: &'static str,
        formulation: SolidFormulation,
        rule: IncompatibleStabilisation,
    }

    const K_L: Kind = Kind {
        name: "L",
        formulation: INC,
        rule: LIN,
    };
    const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;

    fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
        if f != SolidFormulation::Full {
            mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
        }
        mesh
    }

    fn model_of(mesh: SolidMesh, kind: Kind) -> SolidModel {
        let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
        m.set_incompatible_stabilisation(kind.rule);
        m.add_material("all", neo_hookean()).unwrap();
        m
    }

    const DELTA: f64 = 0.1;
    const TOL: f64 = 1e-9;

    fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
        mesh.node_set(set)
            .unwrap()
            .iter()
            .copied()
            .filter(|&n| keep(&mesh.nodes()[n]))
            .collect()
    }

    /// The punch and corner sets (by coordinates).
    fn add_sets(mesh: &mut SolidMesh) {
        let punch = nodes_where(mesh, "zmax", |p| p[0] <= 0.5 + TOL && p[1] <= 0.5 + TOL);
        let corner = nodes_where(mesh, "zmax", |p| (p[0] - 1.0).abs() < TOL && (p[1] - 1.0).abs() < TOL);
        mesh.add_node_set("punch", punch).unwrap();
        mesh.add_node_set("corner", corner).unwrap();
    }

    fn block_mesh(n: usize) -> SolidMesh {
        let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
        add_sets(&mut mesh);
        mesh
    }

    /// The model of the block with the punch at `-0.1` (the die constraints of ADR 0678 reading c1).
    fn block_model(mesh: &SolidMesh, kind: Kind) -> SolidModel {
        let mut m = model_of(mesh.clone(), kind);
        for (set, comp, value) in [
            ("xmin", 0, 0.0),
            ("xmax", 0, 0.0),
            ("ymin", 1, 0.0),
            ("ymax", 1, 0.0),
            ("zmin", 2, 0.0),
            ("punch", 2, -DELTA),
        ] {
            m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
        }
        m
    }

    fn options(steps: usize) -> NewtonOptions {
        NewtonOptions::new(steps)
            .threads(4)
            .record_active_set(true)
            .factor(SUPERNODAL)
    }

    /// `R = -sum_punch f_z / mu` and `w_c` of a state of the block.
    fn block_measures(m: &SolidModel, u: &[f64], fint: &[f64]) -> (f64, f64) {
        let dofs = m.dof_map();
        let punch = m.mesh().node_set("punch").unwrap();
        let rz: f64 = punch.iter().map(|&n| fint[dofs.equation(n, 2).unwrap()]).sum();
        let corner = m.mesh().node_set("corner").unwrap()[0];
        (-rz / MU, u[dofs.equation(corner, 2).unwrap()])
    }

    /// The error text of the EC1 solve of ADR 0678 (`f747-scratch/run3-heavy.log`, line 23).
    const ADR0678_TEXT: &str =
        "solver newton: load step 10 of 10 (load factor 9.625e-1 to 9.656250000000001e-1) after 5 \
                            halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal \
                            2.8253460477227545e6)";

    // ============================================================================ the observer (ADR 0688 D1)

    struct AttemptLog {
        committed: f64,
        attempted: f64,
        converged: Option<bool>,
        iterates: Vec<Vec<f64>>,
    }

    #[derive(Default)]
    struct Log {
        attempts: Vec<AttemptLog>,
    }

    struct Watch<'a> {
        log: &'a RefCell<Log>,
    }

    impl StepControl for Watch<'_> {
        fn name(&self) -> &str {
            "watch"
        }
        fn load_factor(
            &mut self,
            committed: f64,
            proposed: f64,
            _step: usize,
            _halvings: usize,
        ) -> rcfd_fem::Result<f64> {
            self.log.borrow_mut().attempts.push(AttemptLog {
                committed,
                attempted: proposed,
                converged: None,
                iterates: Vec::new(),
            });
            Ok(proposed)
        }
        fn converged(&mut self, _increment: &LoadStep) {
            if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
                a.converged = Some(true);
            }
        }
        fn failed(&mut self, _committed: f64, _attempted: f64) {
            if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
                a.converged = Some(false);
            }
        }
    }

    struct Observer<'a> {
        log: &'a RefCell<Log>,
    }

    impl Contribution for Observer<'_> {
        fn name(&self) -> &str {
            "observer"
        }
        fn add(
            &self,
            u: &[f64],
            _load_factor: f64,
            _force: &mut [f64],
            _tangent: &mut Vec<(usize, usize, f64)>,
        ) -> rcfd_fem::Result<()> {
            if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
                if a.converged.is_none() {
                    a.iterates.push(u.to_vec());
                }
            }
            Ok(())
        }
        fn add_trial_force(&self, _u: &[f64], _load_factor: f64, _force: &mut [f64]) -> rcfd_fem::Result<()> {
            Ok(())
        }
    }

    /// The solve of the block with the observer: the error text (empty if it passes) and the log.
    fn solve_observed(mesh: &SolidMesh, tag: &str, kind: Kind, o: &NewtonOptions) -> (String, Log) {
        let m = block_model(mesh, kind);
        let log = RefCell::new(Log::default());
        let obs = Observer { log: &log };
        let mut watch = Watch { log: &log };
        let t0 = Instant::now();
        let out = solve_with(
            &m,
            &[],
            &[],
            o,
            DriverExtensions {
                contributions: vec![&obs],
                step_control: Some(&mut watch),
            },
        );
        let secs = t0.elapsed().as_secs_f64();
        let text = match out {
            Ok(_) => {
                println!("{tag} observed ({}): passes, {secs:.1} s", kind.name);
                String::new()
            }
            Err(e) => {
                println!("{tag} observed ({}): solve failed: {e}, {secs:.1} s", kind.name);
                e.to_string()
            }
        };
        let log = log.into_inner();
        let att: Vec<String> = log
            .attempts
            .iter()
            .map(|a| {
                format!(
                    "{:.6}->{:.6}{}",
                    a.committed,
                    a.attempted,
                    if a.converged == Some(true) { "" } else { " x" }
                )
            })
            .collect();
        println!("{tag} observed attempts: [{}]", att.join(", "));
        (text, log)
    }

    // ============================================================================ the tangent, its inertia and its lowest pairs

    /// SY: the symmetric part `(K + K^T) / 2` (ADR 0694 `symmetrise`).
    fn symmetrise(a: &CsrMatrix) -> CsrMatrix {
        let mut t = Vec::with_capacity(2 * a.val.len());
        for r in 0..a.n {
            for p in a.row_ptr[r]..a.row_ptr[r + 1] {
                let c = a.col[p];
                t.push((r, c, 0.5 * a.val[p]));
                t.push((c, r, 0.5 * a.val[p]));
            }
        }
        CsrMatrix::from_triplets(a.n, t)
    }

    /// The supernodal `L D L^T` with the `NonZero` rule (ADR 0694 `inertia`).
    fn factor_nonzero(a: &CsrMatrix) -> Option<SupernodalLdlt> {
        SupernodalLdlt::factor_with(
            a,
            &SparseOptions {
                ordering: SparseOrdering::NestedDissection,
                pivots: PivotRule::NonZero,
                symmetry_tolerance: 1e-12,
            },
        )
        .ok()
    }

    fn inertia(a: &CsrMatrix) -> Option<usize> {
        factor_nonzero(a).map(|f| f.inertia().0)
    }

    fn identity(n: usize) -> CsrMatrix {
        CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect())
    }

    fn points_of(m: &SolidModel, free: &[usize]) -> Vec<[f64; 3]> {
        let dofs = m.dof_map();
        free.iter()
            .map(|&e| {
                let (n, _) = dofs.node_component(e).unwrap();
                let p = m.mesh().nodes()[n];
                [p[0], p[1], p[2]]
            })
            .collect()
    }

    /// The four lowest pairs of `(K, M)` (`Shift::Auto`, the supernodal factor, tolerance `1e-8`, 500 iterations: ADR 0694).
    fn lowest(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]], test: ResidualTest) -> Option<GeneralizedEigen> {
        let opts = SubspaceOptions {
            shift: Shift::Auto,
            tolerance: 1e-8,
            residual_test: test,
            max_iterations: 500,
            factorisation: Factorisation::Supernodal,
            points: Some(points.to_vec()),
            ..SubspaceOptions::default()
        };
        let t0 = Instant::now();
        match lowest_eigenpairs(k, mm, 4, &opts) {
            Ok(e) => {
                println!(
                    "lowest pairs: values {:?}, residuals {:?}, iterations {}, shift {:e}, {:.1} s",
                    e.values,
                    e.residuals,
                    e.iterations,
                    e.shift,
                    t0.elapsed().as_secs_f64()
                );
                Some(e)
            }
            Err(err) => {
                println!("lowest pairs failed: {err:?}");
                None
            }
        }
    }

    fn combine(k: &CsrMatrix, m: &CsrMatrix, s: f64) -> CsrMatrix {
        let mut t = Vec::with_capacity(k.val.len() + m.val.len());
        for r in 0..k.n {
            for p in k.row_ptr[r]..k.row_ptr[r + 1] {
                t.push((r, k.col[p], k.val[p]));
            }
        }
        for r in 0..m.n {
            for p in m.row_ptr[r]..m.row_ptr[r + 1] {
                t.push((r, m.col[p], s * m.val[p]));
            }
        }
        CsrMatrix::from_triplets(k.n, t)
    }

    /// The four lowest pairs of `(K, M)` whatever the inertia (ADR 0694 `lowest_any`, the shift `1.05 s_hi`).
    fn lowest_any(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]], test: ResidualTest) -> Option<GeneralizedEigen> {
        let nu = inertia(k);
        if nu == Some(0) {
            return lowest(k, mm, points, test);
        }
        let rho = k.diag().iter().map(|v| v.abs()).sum::<f64>() / mm.diag().iter().sum::<f64>();
        let zero_at = |s: f64| inertia(&combine(k, mm, s)) == Some(0);
        let mut found = None;
        for j in 0..=60 {
            let s = 1e-6 * rho * 2f64.powi(j);
            if zero_at(s) {
                let lo = if j == 0 { 0.0 } else { 1e-6 * rho * 2f64.powi(j - 1) };
                found = Some((lo, s));
                break;
            }
        }
        let Some((mut lo, mut hi)) = found else {
            println!("lowest_any: no shift with zero inertia");
            return None;
        };
        for _ in 0..10 {
            let mid = 0.5 * (lo + hi);
            if zero_at(mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let opts = SubspaceOptions {
            shift: Shift::Value(1.05 * hi),
            tolerance: 1e-8,
            residual_test: test,
            max_iterations: 500,
            factorisation: Factorisation::Supernodal,
            points: Some(points.to_vec()),
            ..SubspaceOptions::default()
        };
        let t0 = Instant::now();
        match lowest_eigenpairs(k, mm, 4, &opts) {
            Ok(e) => {
                println!(
                "lowest pairs (nu {nu:?}, bracket ({lo:e}, {hi:e}]): values {:?}, residuals {:?}, iterations {}, shift {:e}, \
                 {:.1} s",
                e.values,
                e.residuals,
                e.iterations,
                e.shift,
                t0.elapsed().as_secs_f64()
            );
                Some(e)
            }
            Err(err) => {
                println!("lowest pairs (nu {nu:?}) failed: {err:?}");
                None
            }
        }
    }

    // ============================================================================ D1, D2: the case, the bracket, the readings

    /// A converged state of a path at the load parameter `p`.
    #[derive(Clone)]
    struct St {
        p: f64,
        u: Vec<f64>,
        states: Vec<ElementState>,
    }

    /// The symmetrised reduced tangent `K`, the load vector `f = -dr_F/dp` and the free equations at a state.
    #[derive(Clone)]
    struct Sys {
        k: CsrMatrix,
        f: Vec<f64>,
        free: Vec<usize>,
    }

    /// The lowest pair of `(K, I)`: the value, the vector and the residual.
    struct Pair {
        mu: f64,
        phi: Vec<f64>,
        residual: f64,
    }

    trait Case {
        /// The solve from a converged state to the load `to` (`to > from.p`).
        fn solve(&self, from: &St, to: f64) -> Result<St, String>;
        fn system(&self, s: &St) -> Sys;
        fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair>;
    }

    /// The inertia and the path tangent `v_t = K^{-1} f` at a state (one factor).
    #[derive(Clone)]
    struct Info {
        nu: Option<usize>,
        vt: Option<Vec<f64>>,
        sys: Sys,
    }

    fn analyse(case: &dyn Case, s: &St) -> Info {
        let sys = case.system(s);
        let (nu, vt) = match factor_nonzero(&sys.k) {
            Some(fac) => (Some(fac.inertia().0), Some(fac.solve(&sys.f))),
            None => (None, None),
        };
        Info { nu, vt, sys }
    }

    // ============================================================================ the finite-element case

    /// The FEM case: `solve_increment` with the indefinite tangent from the converged state; the prescribed list `pres` (the
    /// model's own constraints and these, scaled by the load factor).
    struct FemCase<'a> {
        m: &'a SolidModel,
        pres: Vec<Prescribed>,
        opts: NewtonOptions,
        /// The residual measure of the lowest pairs (ADR 0697 amendment 1: `Backward` in the amended tests).
        test: ResidualTest,
    }

    impl Case for FemCase<'_> {
        fn solve(&self, from: &St, to: f64) -> Result<St, String> {
            let zero = vec![0.0; self.m.dof_map().n_equations()];
            solve_increment(
                self.m,
                &self.pres,
                &zero,
                &from.u,
                &from.states,
                from.p,
                to,
                &self.opts,
                &[],
            )
            .map(|eq| St {
                p: to,
                u: eq.u,
                states: eq.states,
            })
            .map_err(|e| e.to_string())
        }
        fn system(&self, s: &St) -> Sys {
            let kt = tangent(self.m, &s.u, &s.states, Some(4)).unwrap();
            let fixed = prescribed_equations(self.m, &self.pres).unwrap();
            let sys = reduce(&kt, &vec![0.0; kt.n], &fixed).unwrap();
            let free = sys.free().to_vec();
            Sys {
                k: symmetrise(&sys.matrix),
                f: sys.rhs,
                free,
            }
        }
        fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair> {
            let points = points_of(self.m, &s.free);
            let id = identity(s.free.len());
            let e = if nu == 0 {
                lowest(&s.k, &id, &points, self.test)
            } else {
                lowest_any(&s.k, &id, &points, self.test)
            }?;
            Some(Pair {
                mu: e.values[0],
                phi: e.vectors[0].clone(),
                residual: e.residuals.iter().fold(0.0f64, |w, &r| w.max(r)),
            })
        }
    }

    /// The failing attempt of the observed `16^3` solve: its index in the log.
    fn failing_attempt(log: &Log) -> Option<usize> {
        log.attempts
            .iter()
            .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
    }

    // ============================================================================ D6: the branch case on the block

    /// The block model without its punch constraint (the punch is given as a prescribed list in the fixed-load solves, D6).
    fn block_model_free_punch(mesh: &SolidMesh, kind: Kind) -> SolidModel {
        let mut m = model_of(mesh.clone(), kind);
        for (set, comp) in [("xmin", 0), ("xmax", 0), ("ymin", 1), ("ymax", 1), ("zmin", 2)] {
            m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
        }
        m
    }

    /// A path with a fixed-load Newton under an external force on the free equations.
    trait Branch: Case {
        /// Newton at the fixed load `p` from `start` (its prescribed values replaced by those at `p`) with the external force
        /// `eta g` on the free equations (`g` indexed by the free equations).
        fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String>;
        /// The free equations.
        fn free(&self) -> Vec<usize>;
    }

    /// D6: the branch case of the block: `solve_increment` and the tangent on the block model, `solve_from` on the model
    /// without the punch constraint with the punch prescribed at `p v`.
    struct FemBranch<'a> {
        base: FemCase<'a>,
        m2: &'a SolidModel,
        fixed_eqs: Vec<(usize, f64)>,
        punch: Vec<(usize, usize, f64)>,
        free: Vec<usize>,
    }

    impl<'a> FemBranch<'a> {
        fn new(m: &'a SolidModel, m2: &'a SolidModel) -> FemBranch<'a> {
            let fixed_eqs = prescribed_equations(m, &[]).unwrap();
            let values: HashMap<usize, f64> = fixed_eqs.iter().copied().collect();
            let dofs = m.dof_map();
            let punch = m
                .mesh()
                .node_set("punch")
                .unwrap()
                .iter()
                .map(|&nd| {
                    let eq = dofs.equation(nd, 2).unwrap();
                    (nd, 2, values[&eq])
                })
                .collect();
            let mut is_fixed = vec![false; dofs.n_equations()];
            for &(eq, _) in &fixed_eqs {
                is_fixed[eq] = true;
            }
            let free = (0..dofs.n_equations()).filter(|&q| !is_fixed[q]).collect();
            FemBranch {
                base: FemCase {
                    m,
                    pres: Vec::new(),
                    opts: options(10).indefinite_tangent(true),
                    test: ResidualTest::Backward,
                },
                m2,
                fixed_eqs,
                punch,
                free,
            }
        }
    }

    impl Case for FemBranch<'_> {
        fn solve(&self, from: &St, to: f64) -> Result<St, String> {
            self.base.solve(from, to)
        }
        fn system(&self, s: &St) -> Sys {
            self.base.system(s)
        }
        fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair> {
            self.base.lowest_pair(s, nu)
        }
    }

    impl Branch for FemBranch<'_> {
        fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String> {
            let mut u0 = start.u.clone();
            for &(eq, v) in &self.fixed_eqs {
                u0[eq] = p * v;
            }
            let pres: Vec<Prescribed> = self
                .punch
                .iter()
                .map(|&(nd, c, v)| Prescribed::new(nd, c, p * v))
                .collect();
            let mut load = vec![0.0; u0.len()];
            for (k, &q) in self.free.iter().enumerate() {
                load[q] = eta * g[k];
            }
            solve_from(self.m2, &pres, &load, &u0, &start.states, &self.base.opts, &[])
                .map(|eq| St {
                    p,
                    u: eq.u,
                    states: eq.states,
                })
                .map_err(|e| e.to_string())
        }
        fn free(&self) -> Vec<usize> {
            self.free.clone()
        }
    }

    // ============================================================================ D2: the mode and the forced response

    fn amp(phi: &[f64], free: &[usize], u: &[f64]) -> f64 {
        phi.iter().zip(free).map(|(p, &q)| p * u[q]).sum()
    }

    /// The lowest pair at a state with its vector of unit 2-norm, the sign fixed by its largest component (positive).
    fn mode_at(case: &dyn Case, tag: &str, s: &St) -> (Info, Option<Pair>) {
        let info = analyse(case, s);
        let pair = info.nu.and_then(|nu| case.lowest_pair(&info.sys, nu)).map(|mut p| {
            let n = norm(p.phi.iter().copied());
            let big = p.phi.iter().fold(0.0f64, |w, &x| if x.abs() > w.abs() { x } else { w });
            let s = if big < 0.0 { -1.0 / n } else { 1.0 / n };
            p.phi.iter_mut().for_each(|x| *x *= s);
            p
        });
        println!(
            "{tag} mode at {:.12}: nu {:?}, mu_1 {:?}, residual {:?}",
            s.p,
            info.nu,
            pair.as_ref().map(|p| p.mu),
            pair.as_ref().map(|p| p.residual)
        );
        (info, pair)
    }

    // ============================================================================ D3: the bordered Newton

    /// The branch case with the residual, its convergence measure and the load setting of the bordered Newton (ADR 0711 D3).
    trait Bordered: Branch {
        /// The free residual at a state (indexed by the free equations) and its convergence measure.
        fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String>;
        /// The tolerance of the measure and the energy tolerance (`None`: no energy test).
        fn tolerances(&self) -> (f64, Option<f64>);
        /// The state `s` at the load `p` (the prescribed values `p v`).
        fn at_load(&self, s: &St, p: f64) -> St;
    }

    impl Bordered for FemBranch<'_> {
        fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
            let fint = internal_force(self.base.m, &s.u, &s.states).map_err(|e| e.to_string())?;
            let r: Vec<f64> = self.free.iter().map(|&q| fint[q]).collect();
            let reference = norm(fint.iter().copied());
            let rn = norm(r.iter().copied());
            Ok((r, if reference > 0.0 { rn / reference } else { rn }))
        }
        fn tolerances(&self) -> (f64, Option<f64>) {
            (1e-10, Some(1e-10))
        }
        fn at_load(&self, s: &St, p: f64) -> St {
            let mut u = s.u.clone();
            for &(eq, v) in &self.fixed_eqs {
                u[eq] = p * v;
            }
            St {
                p,
                u,
                states: s.states.clone(),
            }
        }
    }

    /// The linear constraint `g . u_F + h lambda = c` of the bordered Newton.
    struct Control {
        g: Vec<f64>,
        h: f64,
        c: f64,
    }

    /// D3: the bordered Newton from `start` under the constraint `ctl`: `K y_1 = -R`, `K y_2 = f`, `dl = (c - g . u_F - h lambda -
    /// g . y_1) / (g . y_2 + h)`, `du = y_1 + dl y_2`; the converged state and the number of iterations.
    fn bordered<B: Bordered>(case: &B, start: &St, ctl: &Control) -> Result<(St, usize), String> {
        let free = case.free();
        let (tol, etol) = case.tolerances();
        let mut s = start.clone();
        let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
        for k in 0..=20 {
            let (r, m) = case.residual(&s)?;
            if !m.is_finite() {
                return Err(format!("non-finite residual at iteration {k}"));
            }
            let gu: f64 = ctl.g.iter().zip(&free).map(|(g, &q)| g * s.u[q]).sum();
            let cres = gu + ctl.h * s.p - ctl.c;
            let c_ok = cres.abs() <= 1e-9 * ctl.c.abs().max(1e-300);
            let e_ok = etol.map_or(true, |t| e_last <= t * e_first);
            if k >= 1 && m <= tol && e_ok && c_ok {
                return Ok((s, k));
            }
            if k == 20 {
                break;
            }
            let sys = case.system(&s);
            let fac = factor_nonzero(&sys.k).ok_or_else(|| format!("the factor refused at iteration {k}"))?;
            let mr: Vec<f64> = r.iter().map(|x| -x).collect();
            let y1 = fac.solve(&mr);
            let y2 = fac.solve(&sys.f);
            let den = dot(&ctl.g, &y2) + ctl.h;
            let dl = (-cres - dot(&ctl.g, &y1)) / den;
            if !dl.is_finite() {
                return Err(format!("non-finite load correction at iteration {k}"));
            }
            let du: Vec<f64> = y1.iter().zip(&y2).map(|(a, b)| a + dl * b).collect();
            let e = du
                .iter()
                .zip(sys.f.iter().zip(&r))
                .map(|(d, (f, ri))| d * (dl * f - ri))
                .sum::<f64>()
                .abs();
            if k == 0 {
                e_first = e;
            }
            e_last = e;
            for (&q, d) in free.iter().zip(&du) {
                s.u[q] += d;
            }
            s = case.at_load(&s, s.p + dl);
        }
        Err("not converged in 20 iterations".to_string())
    }

    // ============================================================================ D3: the amplitude-controlled trace

    /// An accepted state of the D3 trace.
    #[derive(Clone)]
    struct APoint {
        s: St,
        info: Info,
        a: f64,
        /// `dlambda / da = 1 / (phi . v_t)`.
        sigma: f64,
        /// The rung (0: the start; `None`: a halved state).
        rung: Option<usize>,
    }

    struct Fold {
        /// The index of the second state of the bracket.
        k: usize,
        lo: f64,
        hi: f64,
        a_est: f64,
        l_est: f64,
    }

    struct ATrace {
        points: Vec<APoint>,
        /// The stop: "fold + 2", "rungs", "range", "lost".
        stop: String,
        reading: String,
        fold: Option<Fold>,
    }

    const AMP_HALVINGS: usize = 4;
    const AMP_CAP: usize = 40;

    fn apoint<B: Bordered>(case: &B, s: St, info: Info, phi: &[f64], rung: Option<usize>) -> APoint {
        let free = case.free();
        let a = amp(phi, &free, &s.u);
        let sigma = info.vt.as_ref().map(|v| 1.0 / dot(phi, v)).unwrap_or(f64::NAN);
        APoint {
            s,
            info,
            a,
            sigma,
            rung,
        }
    }

    /// D3: the fold between two consecutive states.
    fn fold_between(p0: &APoint, p1: &APoint) -> Fold {
        let (a0, l0, s0) = (p0.a, p0.s.p, p0.sigma);
        let (a1, l1, s1) = (p1.a, p1.s.p, p1.sigma);
        let ax = (l1 - l0 + s0 * a0 - s1 * a1) / (s0 - s1);
        let lo = l0 + s0 * (ax - a0);
        let a_est = a0 - s0 * (a1 - a0) / (s1 - s0);
        let l_est = l0 + s0 * (a_est - a0) / 2.0;
        Fold {
            k: 0,
            lo,
            hi: l0.min(l1),
            a_est,
            l_est,
        }
    }

    /// D3: the trace of the branch from `from` under amplitude control over the rungs `a_b 2^(k/2)`, `k = 1..=rungs`, the load
    /// kept in `range`.
    fn amp_trace<B: Bordered>(
        case: &B,
        tag: &str,
        from: (St, Info),
        phi: &[f64],
        rungs: usize,
        range: (f64, f64),
    ) -> ATrace {
        let free = case.free();
        let first = apoint(case, from.0, from.1, phi, Some(0));
        let a_b = first.a;
        println!(
            "{tag} amplitude trace: start {:.12}, a {a_b:+.9e}, nu {:?}, dlambda/da {:+.6e}",
            first.s.p, first.info.nu, first.sigma
        );
        let mut out = ATrace {
            points: vec![first],
            stop: "rungs".to_string(),
            reading: String::new(),
            fold: None,
        };
        let mut solves = 0;
        let mut after = 0;
        let mut undecided = false;
        'rungs: for k in 1..=rungs {
            let target = a_b * 2f64.powf(k as f64 / 2.0);
            let mut tgt = target;
            let mut halvings = 0;
            loop {
                let cur = out.points.last().unwrap().clone();
                if solves >= AMP_CAP {
                    println!(
                        "{tag} amplitude trace: the cap of {AMP_CAP} solves reached at a {:+.6e}",
                        cur.a
                    );
                    out.stop = "lost".to_string();
                    break 'rungs;
                }
                solves += 1;
                let t0 = Instant::now();
                let accepted = match cur.info.vt.as_ref() {
                    None => Err("the previous factor refused".to_string()),
                    Some(vt) => {
                        let dp = (tgt - cur.a) / dot(phi, vt);
                        let mut up = cur.s.clone();
                        for (&q, v) in free.iter().zip(vt) {
                            up.u[q] += dp * v;
                        }
                        let pred = case.at_load(&up, cur.s.p + dp);
                        let ctl = Control {
                            g: phi.to_vec(),
                            h: 0.0,
                            c: tgt,
                        };
                        match bordered(case, &pred, &ctl) {
                            Ok((s, its)) => {
                                let corr = max_abs(free.iter().map(|&q| s.u[q] - pred.u[q]));
                                let step = max_abs(free.iter().map(|&q| pred.u[q] - cur.s.u[q]));
                                if corr.is_finite() && step.is_finite() && corr <= step {
                                    Ok((s, its, corr, step))
                                } else {
                                    Err(format!("discontinuous ({corr:.4e} > {step:.4e}), {its} iterations"))
                                }
                            }
                            Err(e) => Err(format!("fails: {e}")),
                        }
                    }
                };
                match accepted {
                    Ok((s, its, corr, step)) => {
                        let info = analyse(case, &s);
                        let rung = if tgt == target { Some(k) } else { None };
                        let pt = apoint(case, s, info, phi, rung);
                        println!(
                        "{tag} amplitude solve {solves} (rung {k}{}): a {:+.9e} -> {:+.9e}: accepted, lambda {:.12}, nu {:?}, \
                         dlambda/da {:+.6e}, {its} iterations, continuity {corr:.3e} <= {step:.3e}, {:.1} s",
                        if rung.is_none() { ", halved" } else { "" },
                        cur.a,
                        pt.a,
                        pt.s.p,
                        pt.info.nu,
                        pt.sigma,
                        t0.elapsed().as_secs_f64()
                    );
                        out.points.push(pt);
                        let n = out.points.len();
                        let (p0, p1) = (&out.points[n - 2], &out.points[n - 1]);
                        if out.fold.is_none() && !undecided && p0.sigma < 0.0 && p1.sigma >= 0.0 {
                            let one = match (p0.info.nu, p1.info.nu) {
                                (Some(x), Some(y)) => x.abs_diff(y) == 1,
                                _ => false,
                            };
                            if one {
                                let mut f = fold_between(p0, p1);
                                f.k = n - 1;
                                out.fold = Some(f);
                            } else {
                                undecided = true;
                            }
                        }
                        let lam = out.points[n - 1].s.p;
                        if !(range.0..=range.1).contains(&lam) {
                            out.stop = "range".to_string();
                            break 'rungs;
                        }
                        if tgt == target {
                            break;
                        }
                        tgt = target;
                    }
                    Err(e) => {
                        println!(
                        "{tag} amplitude solve {solves} (rung {k}): a {:+.9e} -> {tgt:+.9e}: rejected, {e}, {:.1} s",
                        cur.a,
                        t0.elapsed().as_secs_f64()
                    );
                        if halvings == AMP_HALVINGS {
                            println!("{tag} amplitude trace: lost after a {:+.9e}", cur.a);
                            out.stop = "lost".to_string();
                            break 'rungs;
                        }
                        halvings += 1;
                        tgt = cur.a + 0.5 * (tgt - cur.a);
                    }
                }
            }
            if out.fold.is_some() && out.fold.as_ref().unwrap().k < out.points.len() - 1 {
                after += 1;
            } else if out.fold.is_some() {
                after = 0;
            }
            if out.fold.is_some() && after == 2 {
                out.stop = "fold + 2".to_string();
                break;
            }
        }
        let last = out.points.last().unwrap();
        out.reading = if undecided {
            "undecided".to_string()
        } else if let Some(f) = &out.fold {
            let (p0, p1) = (&out.points[f.k - 1], &out.points[f.k]);
            format!(
            "fold: a in [{:.9e}, {:.9e}], lambda_f in [{:.12}, {:.12}], estimate a {:.9e} lambda {:.12}, nu {:?} -> {:?}",
            p0.a, p1.a, f.lo, f.hi, f.a_est, f.l_est, p0.info.nu, p1.info.nu
        )
        } else if out.stop == "lost" {
            format!(
                "lost at a = {:.9e}, lambda = {:.12} (dlambda/da {:+.6e})",
                last.a, last.s.p, last.sigma
            )
        } else {
            format!("no fold up to a = {:.9e} (lambda {:.12})", last.a, last.s.p)
        };
        println!(
            "{tag} READING AF: {}; stop {}, {} states, {solves} solves",
            out.reading,
            out.stop,
            out.points.len()
        );
        out
    }

    // ============================================================================ ADR 0718 D2, D3: the element's local problem

    const MODES: usize = 9;
    const NODES: usize = 24;
    const ALL: usize = NODES + MODES;

    type M9 = [[f64; MODES]; MODES];

    /// An element at a state: its coordinates, its nodal displacement (node-major) and its state.
    pub(crate) struct Elem {
        coords: Vec<Vec3>,
        ue: Vec<f64>,
        state: ElementState,
    }

    pub(crate) fn element_of(m: &SolidModel, e: usize, u: &[f64], states: &[ElementState]) -> Elem {
        let mesh = m.mesh();
        let dofs = &m.dof_map();
        let nodes = mesh.element_nodes(e);
        Elem {
            coords: nodes.iter().map(|&n| mesh.nodes()[n]).collect(),
            ue: nodes
                .iter()
                .flat_map(|&n| (0..3).map(move |c| u[dofs.equation(n, c).unwrap()]))
                .collect(),
            state: material_part(&states[e]),
        }
    }

    /// ADR 0723 D1: the material part of an element state (the neo-Hookean material has no state: a warm state of nine values,
    /// the committed modes, has an empty material part).
    fn material_part(st: &ElementState) -> ElementState {
        if st.values().len() == MODES {
            ElementState::new(Vec::new())
        } else {
            st.clone()
        }
    }

    /// ADR 0723 D1: the committed modes of a warm element state, if present.
    fn committed_modes(st: &ElementState) -> Option<[f64; MODES]> {
        let v = st.values();
        (v.len() == MODES).then(|| std::array::from_fn(|k| v[k]))
    }

    /// The force (33 values) and the mode block `K_aa` of the extended element at the modes `alpha` (D2).
    fn mode_block(mat: &Material, el: &Elem, alpha: &[f64; MODES]) -> Result<(Vec<f64>, M9), String> {
        let (f, k) =
            enhanced_force_and_tangent_with(&el.coords, mat, &el.ue, alpha, &el.state, &PointContext::default(), LIN)
                .map_err(|e| e.to_string())?;
        let v = k.values();
        Ok((
            f,
            std::array::from_fn(|i| std::array::from_fn(|j| v[(NODES + i) * ALL + NODES + j])),
        ))
    }

    /// The asymmetry `max |K - K^T| / max |K|` and the eigenvalues (ascending) of the symmetric part.
    fn spectrum(k: &M9) -> (f64, Vec<f64>) {
        let amax = k.iter().flatten().fold(0.0f64, |w, x| w.max(x.abs()));
        let mut asym = 0.0f64;
        for i in 0..MODES {
            for j in 0..MODES {
                asym = asym.max((k[i][j] - k[j][i]).abs());
            }
        }
        let dense: Vec<f64> = k.iter().flatten().copied().collect();
        let eig = symmetric_eigen(&dense, MODES)
            .map(|e| e.values)
            .unwrap_or_else(|_| vec![f64::NAN; MODES]);
        (asym / amax, eig)
    }

    /// The element's record of the plain local iteration from `start` (`None`: `alpha = 0`): the iterates and the outcome.
    type Record = (Vec<ModeIterate>, Result<[f64; MODES], String>);

    fn record_from(mat: &Material, el: &Elem, start: Option<&[f64; MODES]>) -> Record {
        match mode_iteration_record_from(
            &el.coords,
            mat,
            &el.ue,
            &el.state,
            &PointContext::default(),
            LIN,
            IncompatibleModeUpdate::Plain,
            start,
        ) {
            Ok((rec, out)) => (rec, out.map_err(|e| e.to_string())),
            Err(e) => (Vec::new(), Err(e.to_string())),
        }
    }
    // ============================================================================ ADR 0718 D4: the fine trace and the reading

    /// The bordered Newton of [`bordered`] (the same arithmetic) that keeps, on a failure, the iterate whose evaluation failed;
    /// with `verbose` each iterate is printed.
    fn bordered_rec<B: Bordered>(
        case: &B,
        start: &St,
        ctl: &Control,
        verbose: bool,
    ) -> Result<(St, usize), (String, Option<St>)> {
        let free = case.free();
        let (tol, etol) = case.tolerances();
        let mut s = start.clone();
        let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
        for k in 0..=20 {
            let (r, m) = match case.residual(&s) {
                Ok(x) => x,
                Err(e) => {
                    if verbose {
                        println!("    global iterate {k}: lambda {:.15}: the residual fails", s.p);
                    }
                    return Err((e, Some(s)));
                }
            };
            if !m.is_finite() {
                return Err((format!("non-finite residual at iteration {k}"), Some(s)));
            }
            let gu: f64 = ctl.g.iter().zip(&free).map(|(g, &q)| g * s.u[q]).sum();
            let cres = gu + ctl.h * s.p - ctl.c;
            if verbose {
                println!(
                    "    global iterate {k}: lambda {:.15}, residual {m:.4e}, constraint {cres:+.4e}",
                    s.p
                );
            }
            let c_ok = cres.abs() <= 1e-9 * ctl.c.abs().max(1e-300);
            let e_ok = etol.map_or(true, |t| e_last <= t * e_first);
            if k >= 1 && m <= tol && e_ok && c_ok {
                return Ok((s, k));
            }
            if k == 20 {
                break;
            }
            let sys = case.system(&s);
            let Some(fac) = factor_nonzero(&sys.k) else {
                return Err((format!("the factor refused at iteration {k}"), None));
            };
            let mr: Vec<f64> = r.iter().map(|x| -x).collect();
            let y1 = fac.solve(&mr);
            let y2 = fac.solve(&sys.f);
            let den = dot(&ctl.g, &y2) + ctl.h;
            let dl = (-cres - dot(&ctl.g, &y1)) / den;
            if !dl.is_finite() {
                return Err((format!("non-finite load correction at iteration {k}"), None));
            }
            let du: Vec<f64> = y1.iter().zip(&y2).map(|(a, b)| a + dl * b).collect();
            let e = du
                .iter()
                .zip(sys.f.iter().zip(&r))
                .map(|(d, (f, ri))| d * (dl * f - ri))
                .sum::<f64>()
                .abs();
            if k == 0 {
                e_first = e;
            }
            e_last = e;
            for (&q, d) in free.iter().zip(&du) {
                s.u[q] += d;
            }
            s = case.at_load(&s, s.p + dl);
        }
        Err(("not converged in 20 iterations".to_string(), None))
    }

    /// One attempt of ADR 0711 D3 from `cur` to the amplitude `tgt`: the tangent predictor, the bordered Newton, the continuity
    /// test; the state, the iterations, the correction and the step, or the text and the failing iterate.
    type Attempt = Result<(St, usize, f64, f64), (String, Option<St>)>;

    fn attempt<B: Bordered>(case: &B, cur: &APoint, phi: &[f64], tgt: f64, verbose: bool) -> Attempt {
        let free = case.free();
        let Some(vt) = cur.info.vt.as_ref() else {
            return Err(("the previous factor refused".to_string(), None));
        };
        let dp = (tgt - cur.a) / dot(phi, vt);
        let mut up = cur.s.clone();
        for (&q, v) in free.iter().zip(vt) {
            up.u[q] += dp * v;
        }
        let pred = case.at_load(&up, cur.s.p + dp);
        let ctl = Control {
            g: phi.to_vec(),
            h: 0.0,
            c: tgt,
        };
        let (s, its) = bordered_rec(case, &pred, &ctl, verbose)?;
        let corr = max_abs(free.iter().map(|&q| s.u[q] - pred.u[q]));
        let step = max_abs(free.iter().map(|&q| pred.u[q] - cur.s.u[q]));
        if corr.is_finite() && step.is_finite() && corr <= step {
            Ok((s, its, corr, step))
        } else {
            Err((
                format!("discontinuous ({corr:.4e} > {step:.4e}), {its} iterations"),
                None,
            ))
        }
    }

    /// An accepted state of the fine trace: the state, its target, `l_min` of the watched local tangent, the continuity numbers,
    /// and the diagnostics of the case.
    struct FPoint {
        pt: APoint,
        /// ADR 0725 reading c5: the bordered iterations of the accepted state.
        its: usize,
        target: f64,
        lmin: f64,
        corr: f64,
        step: f64,
    }

    /// A failed attempt: its target, its text, whether it is local, the failing iterate, the index of the state it started from.
    struct FFail {
        target: f64,
        text: String,
        local: bool,
        from: usize,
    }

    struct FTrace {
        points: Vec<FPoint>,
        fails: Vec<FFail>,
        /// "fold + 2", "steps", "lost".
        stop: String,
        /// The index of the second state of the first sign change of `sigma`.
        sign: Option<usize>,
        solves: usize,
    }

    struct FineOpts {
        da: f64,
        steps: usize,
        halvings: usize,
        cap: usize,
        /// ADR 0725 D2: the time cap of a trace in seconds (checked at the start of an attempt).
        time_cap: f64,
    }

    /// The watched local tangent's smallest eigenvalue at a state.
    type LminOf<'a> = &'a dyn Fn(&St) -> f64;
    /// Whether a failure (its iterate, its text) is local.
    type LocalOf<'a> = &'a dyn Fn(Option<&St>, &str) -> bool;
    /// ADR 0723 D2: the commit of an accepted state (the identity for the plain trace).
    type CommitOf<'a> = &'a dyn Fn(St) -> St;

    /// ADR 0718 D4 (ADR 0723 D2: with the commit): the fine trace from `from` over the targets `a_0 + j da`, `j = 1..=steps`; a failure halved toward its target,
    /// the halvings reset on each acceptance, at most `halvings` in a row, at most `cap` solves.
    #[allow(clippy::too_many_arguments)]
    fn fine_trace<B: Bordered>(
        case: &B,
        tag: &str,
        from: APoint,
        phi: &[f64],
        o: &FineOpts,
        lmin: LminOf,
        local: LocalOf,
        commit: CommitOf,
        at_state: &mut dyn FnMut(&APoint),
    ) -> FTrace {
        let a0 = from.a;
        at_state(&from);
        let lm = lmin(&from.s);
        println!(
        "{tag} fine trace: start lambda {:.12}, a {a0:+.9e}, nu {:?}, dlambda/da {:+.6e}, l_min {lm:+.6e}, Da {:.6e}",
        from.s.p, from.info.nu, from.sigma, o.da
    );
        let mut out = FTrace {
            points: vec![FPoint {
                its: 0,
                target: a0,
                pt: from,
                lmin: lm,
                corr: 0.0,
                step: 0.0,
            }],
            fails: Vec::new(),
            stop: String::new(),
            sign: None,
            solves: 0,
        };
        let t_all = Instant::now();
        'steps: for j in 1..=o.steps {
            let target = a0 + j as f64 * o.da;
            let mut tgt = target;
            let mut h = 0;
            loop {
                if t_all.elapsed().as_secs_f64() > o.time_cap {
                    println!("{tag} fine trace: the time cap of {} s reached", o.time_cap);
                    out.stop = "time cap".to_string();
                    break 'steps;
                }
                if out.solves >= o.cap {
                    println!("{tag} fine trace: the cap of {} solves reached", o.cap);
                    out.stop = "lost".to_string();
                    break 'steps;
                }
                out.solves += 1;
                let t0 = Instant::now();
                let cur = &out.points.last().unwrap().pt;
                match attempt(case, cur, phi, tgt, false) {
                    Ok((s, its, corr, step)) => {
                        // ADR 0723 D2: the accepted state committed before it is analysed.
                        let s = commit(s);
                        let info = analyse(case, &s);
                        let rung = if tgt == target { Some(j) } else { None };
                        let pt = apoint(case, s, info, phi, rung);
                        let lm = lmin(&pt.s);
                        at_state(&pt);
                        println!(
                        "{tag} fine solve {} (step {j}{}): a -> {:+.9e}: accepted, lambda {:.12}, nu {:?}, dlambda/da \
                         {:+.6e}, l_min {lm:+.6e}, {its} iterations, continuity {corr:.3e} <= {step:.3e}, {:.1} s (total {:.1} s)",
                        out.solves,
                        if rung.is_none() { ", halved" } else { "" },
                        pt.a,
                        pt.s.p,
                        pt.info.nu,
                        pt.sigma,
                        t0.elapsed().as_secs_f64(),
                        t_all.elapsed().as_secs_f64()
                    );
                        out.points.push(FPoint {
                            pt,
                            its,
                            target: tgt,
                            lmin: lm,
                            corr,
                            step,
                        });
                        h = 0;
                        let n = out.points.len();
                        if out.sign.is_none() && out.points[n - 2].pt.sigma < 0.0 && out.points[n - 1].pt.sigma >= 0.0 {
                            out.sign = Some(n - 1);
                        }
                        if let Some(k) = out.sign {
                            if n > k + 2 {
                                out.stop = "fold + 2".to_string();
                                break 'steps;
                            }
                        }
                        if tgt == target {
                            break;
                        }
                        tgt = target;
                    }
                    Err((text, iterate)) => {
                        let is_local = local(iterate.as_ref(), &text);
                        let ca = cur.a;
                        println!(
                        "{tag} fine solve {} (step {j}): a {ca:+.9e} -> {tgt:+.9e}: rejected (local {is_local}), {text}, \
                         {:.1} s",
                        out.solves,
                        t0.elapsed().as_secs_f64()
                    );
                        out.fails.push(FFail {
                            target: tgt,
                            text,
                            local: is_local,
                            from: out.points.len() - 1,
                        });
                        if h == o.halvings {
                            println!("{tag} fine trace: lost after a {ca:+.9e}");
                            out.stop = "lost".to_string();
                            break 'steps;
                        }
                        h += 1;
                        tgt = ca + 0.5 * (tgt - ca);
                    }
                }
            }
        }
        if out.stop.is_empty() {
            out.stop = "steps".to_string();
        }
        out
    }

    const RHO_REG: f64 = 0.1;

    /// ADR 0718 D4: `rho_j = l_min,j / l_min,0` (NaN if `l_min,0 <= 0`).
    fn rho_of(t: &FTrace, j: usize) -> f64 {
        let l0 = t.points[0].lmin;
        if l0 > 0.0 {
            t.points[j].lmin / l0
        } else {
            f64::NAN
        }
    }

    /// ADR 0718 D4: the reading of the fine trace.
    fn reading(t: &FTrace) -> String {
        let p = &t.points;
        let n = p.len();
        let lsign = |j: usize| p[j - 1].lmin * p[j].lmin < 0.0;
        if let Some(j) = t.sign {
            let one = match (p[j - 1].pt.info.nu, p[j].pt.info.nu) {
                (Some(x), Some(y)) => x.abs_diff(y) == 1,
                _ => false,
            };
            let regular = [j - 1, j].iter().all(|&k| p[k].lmin > 0.0 && rho_of(t, k) >= RHO_REG);
            let earlier = (1..=j).any(lsign);
            return if one && regular && !earlier {
                "fold".to_string()
            } else {
                "undecided".to_string()
            };
        }
        if (1..n).any(lsign) {
            return "local breakdown: the mode tangent".to_string();
        }
        // ADR 0723 D4: the cap reached without a sign change of sigma or of l_min.
        if t.stop == "steps" {
            return "no fold up to the cap".to_string();
        }
        if t.stop == "lost" {
            let rho = rho_of(t, n - 1);
            if rho < RHO_REG {
                return "local breakdown: the mode tangent".to_string();
            }
            let last: Vec<&FFail> = t.fails.iter().filter(|f| f.from == n - 1).collect();
            if rho >= RHO_REG && !last.is_empty() && last.iter().all(|f| f.local) {
                return "local breakdown: the local iteration".to_string();
            }
        }
        "undecided".to_string()
    }

    /// The smallest failed target after the last accepted state.
    fn t_fail_min(t: &FTrace) -> f64 {
        let n = t.points.len();
        t.fails
            .iter()
            .filter(|f| f.from == n - 1)
            .map(|f| f.target)
            .fold(f64::INFINITY, f64::min)
    }

    /// The reported companions of the reading: the extrapolated amplitudes where `sigma` and `l_min` vanish, `|sigma_last| / max
    /// |sigma|`, the end interval.
    fn companions(t: &FTrace) -> String {
        let p = &t.points;
        let n = p.len();
        let ex = |f: &dyn Fn(&FPoint) -> f64| {
            if n < 2 {
                return f64::NAN;
            }
            let (x0, x1) = (&p[n - 2], &p[n - 1]);
            x1.pt.a - f(x1) * (x1.pt.a - x0.pt.a) / (f(x1) - f(x0))
        };
        let smax = p.iter().fold(0.0f64, |w, x| w.max(x.pt.sigma.abs()));
        format!(
        "a where sigma -> 0 (linear, last two states) {:.6e}; a where l_min -> 0 {:.6e}; |sigma_last| / max |sigma| {:.4e}; \
         the end in [{:.9e}, {:.9e}]; rho_last {:.4e}; {} solves, stop {}",
        ex(&|x| x.pt.sigma),
        ex(&|x| x.lmin),
        p[n - 1].pt.sigma.abs() / smax,
        p[n - 1].pt.a,
        t_fail_min(t),
        rho_of(t, n - 1),
        t.solves,
        t.stop
    )
    }
    // ============================================================================ the heavy record

    const LF: f64 = 0.9647;
    const LB: f64 = 0.9645;
    /// ADR 0700's `a*_pred` (the `Debug` form of its log line 81).
    const EPS_SWITCH: f64 = 2.7468713663317314e-5;
    /// ADR 0711's rung 5 (`f789-scratch/run2-heavy.log` lines 93 and 110).
    const LAMBDA_5: f64 = 0.958472223921;
    const A_5: f64 = 2.164988001e-4;
    const SIGMA_5: f64 = -5.382683e-1;
    /// The element named by an element error (`element <e>: ...`).
    fn element_in(text: &str) -> Option<usize> {
        text.strip_prefix("element ")?.split(':').next()?.parse().ok()
    }

    // ============================================================================ ADR 0723 D3: the same local root

    /// `|a - b|_inf / |a|_inf`.
    fn rel_dist(a: &[f64], b: &[f64]) -> f64 {
        max_abs(a.iter().zip(b).map(|(x, y)| x - y)) / max_abs(a.iter().copied())
    }

    /// ADR 0723 D3: the root classifier on `d` (the relative distance of the modes) and `dl` (the load difference).
    fn root_reading(d: f64, dl: f64) -> &'static str {
        if !(d.is_finite() && dl.is_finite()) {
            "undecided"
        } else if d <= 1e-6 && dl <= 1e-8 {
            "same root"
        } else if d >= 1e-3 || dl >= 1e-6 {
            "another root"
        } else {
            "undecided"
        }
    }

    // ======================================================================== ADR 0725 reading c6: the tally

    /// The element watched (ADR 0718 X1).
    const E_W: usize = 3720;
    /// ADR 0718's fine trace at `t_1` (`f796-scratch/run3-heavy.log` line 196) and its halved state (line 200).
    const LAMBDA_T1: f64 = 0.959749331159;
    const A_HALVED_PLAIN: f64 = 1.649773378e-4;
    /// ADR 0723's warm trace (`f841-scratch/run2-heavy.log`, the eleven accepted states after rung 4, in order).
    const LAMBDA_WARM: [f64; 11] = [
        0.959749331159,
        0.959451768229,
        0.959182592554,
        0.958947573323,
        0.958752703253,
        0.958604196347,
        0.958508486055,
        0.958472223921,
        0.958478523962,
        0.958502278853,
        0.958605737154,
    ];
    /// ADR 0723's table: the accepted states at which element 3720's plain iteration from 0 fails.
    const A_PLAIN_FAILS: [f64; 4] = [1.689405272e-4, 1.847932848e-4, 2.204619895e-4, 2.323515577e-4];
    /// ADR 0723's fold bracket (reading c4).
    const FOLD_LO: f64 = 2.165e-4;
    const FOLD_HI: f64 = 2.205e-4;

    /// The option whose root is tallied: the warm start (the committed modes) or a local update from `alpha = 0`.
    #[derive(Clone, Copy)]
    pub(crate) enum Opt {
        Warm,
        Update(IncompatibleModeUpdate),
    }

    /// The text of a local outcome.
    pub(crate) fn text(o: &Result<[f64; MODES], String>) -> String {
        match o {
            Ok(_) => "converged".to_string(),
            Err(t) => t.clone(),
        }
    }

    /// The record of `update` from `start` (`None`: `alpha = 0`): the iterates and the outcome.
    pub(crate) fn rec(
        mat: &Material,
        el: &Elem,
        update: IncompatibleModeUpdate,
        start: Option<&[f64; MODES]>,
    ) -> (usize, Result<[f64; MODES], String>) {
        match mode_iteration_record_from(
            &el.coords,
            mat,
            &el.ue,
            &el.state,
            &PointContext::default(),
            LIN,
            update,
            start,
        ) {
            Ok((r, out)) => (r.len(), out.map_err(|e| e.to_string())),
            Err(e) => (0, Err(e.to_string())),
        }
    }

    /// `f(e)` over the elements on four threads, in element order.
    fn per_element<T: Send>(ne: usize, f: &(dyn Fn(usize) -> T + Sync)) -> Vec<T> {
        let per = ne.div_ceil(4);
        std::thread::scope(|sc| {
            let hs: Vec<_> = (0..4)
                .map(|c| sc.spawn(move || (c * per..((c + 1) * per).min(ne)).map(f).collect::<Vec<_>>()))
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        })
    }

    /// Every element's root of `update` from `alpha = 0` at a state (`None` where it fails).
    pub(crate) fn roots(
        m: &SolidModel,
        mat: &Material,
        u: &[f64],
        states: &[ElementState],
        update: IncompatibleModeUpdate,
    ) -> Vec<Option<[f64; MODES]>> {
        per_element(m.mesh().n_elements(), &|e| {
            rec(mat, &element_of(m, e, u, states), update, None).1.ok()
        })
    }

    /// An element where the plain iteration from `alpha = 0` fails (reading c6).
    pub(crate) struct Entry {
        pub(crate) e: usize,
        plain: String,
        /// The option's root (the committed modes for WS) and its iterates.
        pub(crate) opt: Result<[f64; MODES], String>,
        opt_its: usize,
        /// `l_min` of the symmetric part of `K_aa` at the option's root (NaN without a root).
        lmin: f64,
        /// The continuation: the plain iteration from the element's root at the previous state.
        pub(crate) cont: Option<Result<[f64; MODES], String>>,
        /// The option's root is the continuation root (`root(d, 0)` "same root").
        same: Option<bool>,
        /// With `beside`: BT and TR from `alpha = 0`, each (converged, the same root as the option's).
        pub(crate) beside: Option<((bool, bool), (bool, bool))>,
    }

    pub(crate) struct Tally {
        pub(crate) entries: Vec<Entry>,
        /// The option's root of every element (the previous roots of the next state).
        roots: Vec<Option<[f64; MODES]>>,
    }

    impl Tally {
        /// The entries at a regular root.
        fn regular(&self) -> usize {
            self.entries.iter().filter(|x| x.lmin > 0.0).count()
        }
    }

    fn same_root(a: &[f64; MODES], b: &[f64; MODES]) -> bool {
        root_reading(rel_dist(a, b), 0.0) == "same root"
    }

    /// Reading c6 at a state: for every element the plain iteration from `alpha = 0`; where it fails, the option's root,
    /// `l_min` there, the continuation from `prev` and (with `beside`) BT and TR from `alpha = 0`.
    pub(crate) fn tally(
        m: &SolidModel,
        mat: &Material,
        u: &[f64],
        states: &[ElementState],
        prev: Option<&[Option<[f64; MODES]>]>,
        opt: Opt,
        beside: bool,
    ) -> Tally {
        let one = |e: usize| -> (Option<[f64; MODES]>, Option<Entry>) {
            let el = element_of(m, e, u, states);
            let (opt_its, root) = match opt {
                Opt::Warm => (
                    0,
                    committed_modes(&states[e]).ok_or_else(|| "no committed modes".to_string()),
                ),
                Opt::Update(up) => rec(mat, &el, up, None),
            };
            let plain = rec(mat, &el, IncompatibleModeUpdate::Plain, None);
            let Err(plain_text) = plain.1 else {
                return (root.ok(), None);
            };
            let lmin = match &root {
                Ok(a) => mode_block(mat, &el, a)
                    .map(|(_, k)| spectrum(&k).1[0])
                    .unwrap_or(f64::NAN),
                Err(_) => f64::NAN,
            };
            let cont = prev
                .and_then(|p| p[e])
                .map(|p| rec(mat, &el, IncompatibleModeUpdate::Plain, Some(&p)).1);
            let same = match (&root, &cont) {
                (Ok(a), Some(Ok(c))) => Some(same_root(a, c)),
                _ => None,
            };
            let beside = beside.then(|| {
                let reading = |up: IncompatibleModeUpdate| match (rec(mat, &el, up, None).1, &root) {
                    (Ok(x), Ok(a)) => (true, same_root(a, &x)),
                    (Ok(_), Err(_)) => (true, false),
                    (Err(_), _) => (false, false),
                };
                (
                    reading(IncompatibleModeUpdate::Backtracking),
                    reading(IncompatibleModeUpdate::TrustRegion),
                )
            });
            let entry = Entry {
                e,
                plain: plain_text,
                opt: root.clone(),
                opt_its,
                lmin,
                cont,
                same,
                beside,
            };
            (root.ok(), Some(entry))
        };
        let out = per_element(m.mesh().n_elements(), &one);
        let mut t = Tally {
            entries: Vec::new(),
            roots: Vec::with_capacity(out.len()),
        };
        for (r, x) in out {
            t.roots.push(r);
            if let Some(x) = x {
                t.entries.push(x);
            }
        }
        t
    }

    pub(crate) fn print_tally(tag: &str, t: &Tally) {
        println!(
            "{tag} TALLY: the plain iteration from 0 fails at {} elements, {} of them at a regular root of the option",
            t.entries.len(),
            t.regular()
        );
        for x in &t.entries {
            println!(
                "{tag} TALLY element {}: plain from 0: {} | option: {} ({} iterates), l_min {:+.6e} | continuation: {} | same root {:?} \
                 | BT, TR from 0 (converged, same root): {:?}",
                x.e,
                x.plain,
                text(&x.opt),
                x.opt_its,
                x.lmin,
                x.cont.as_ref().map(text).unwrap_or_else(|| "none".to_string()),
                x.same,
                x.beside
            );
        }
    }

    // ======================================================================== ADR 0725 reading c3: the driver's own states

    /// The commit of WS: every element's state replaced by the driver's warm trial state at `s`
    /// (`condensed_force_and_tangent_warm`, the per-element evaluation of ADR 0660 D1); the fallbacks (the warm iteration
    /// from the previous committed modes fails) and the elements whose evaluation failed (their state kept).
    fn commit_driver(m: &SolidModel, mat: &Material, s: &St) -> (St, usize, usize) {
        let mesh = m.mesh();
        let one = |e: usize| -> (Option<Vec<f64>>, bool) {
            let el = element_of(m, e, &s.u, &s.states);
            let fell = committed_modes(&s.states[e])
                .map(|a| rec(mat, &el, IncompatibleModeUpdate::Plain, Some(&a)).1.is_err())
                .unwrap_or(false);
            let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
            let w = condensed_force_and_tangent_warm(
                &coords,
                mat,
                &el.ue,
                &s.states[e],
                &PointContext::default(),
                LIN,
                IncompatibleModeUpdate::Plain,
            );
            (w.ok().map(|w| w.3), fell)
        };
        let out = per_element(mesh.n_elements(), &one);
        let mut c = s.clone();
        let (mut fallbacks, mut failed) = (0, 0);
        for (e, (v, fell)) in out.into_iter().enumerate() {
            fallbacks += fell as usize;
            match v {
                Some(v) => c.states[e] = ElementState::new(v),
                None => failed += 1,
            }
        }
        (c, fallbacks, failed)
    }

    // ======================================================================== ADR 0725 D2: the rebuild and the traces

    /// ADR 0723 D1: ADR 0711's rungs rebuilt on the plain model (OB, R1, S5 checked); the mode `phi` and the state `S4`.
    fn rebuild_to_rung_4(ch: &mut Checks, mesh16: &SolidMesh) -> Option<(Vec<f64>, St, f64)> {
        let t_all = Instant::now();
        let m = block_model(mesh16, K_L);
        let m2 = block_model_free_punch(mesh16, K_L);
        let states = initial_states(&m).unwrap();
        let fb = FemBranch::new(&m, &m2);
        let free = fb.free.clone();
        let (text, log) = solve_observed(mesh16, "block 16^3", K_L, &options(10));
        ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
        let Some(fa) = failing_attempt(&log) else {
            ch.holds("OB: the failing attempt from 0.9625 observed", false);
            return None;
        };
        let mut cur = St {
            p: log.attempts[fa].committed,
            u: log.attempts[fa].iterates[0].clone(),
            states,
        };
        let mut sym: Vec<St> = Vec::new();
        for p in [0.963, 0.9635, 0.964, LB, LF] {
            match fb.solve(&cur, p) {
                Ok(s) => {
                    cur = s.clone();
                    sym.push(s);
                }
                Err(e) => {
                    ch.holds(&format!("D2: the symmetric chain at {p} ({e})"), false);
                    return None;
                }
            }
        }
        println!("D2: the symmetric chain, {:.1} s", t_all.elapsed().as_secs_f64());
        let (_, pf) = mode_at(&fb, "D2", &sym[4]);
        let Some(pf) = pf else {
            ch.holds("D2: the lowest pair at 0.9647", false);
            return None;
        };
        let phi = pf.phi.clone();
        let zero = vec![0.0; free.len()];
        let mut start = sym[4].clone();
        for (k, &q) in free.iter().enumerate() {
            start.u[q] += EPS_SWITCH * phi[k];
        }
        let Ok(ssw) = fb.fixed(&start, LF, 0.0, &phi) else {
            ch.holds("D2: the switch at 0.9647 converges", false);
            return None;
        };
        let Ok(sb) = fb.fixed(&ssw, LB, 0.0, &zero) else {
            ch.holds("D2: the step to 0.9645 converges", false);
            return None;
        };
        let ib = analyse(&fb, &sb);
        let a_rel = amp(&phi, &free, &sb.u) - amp(&phi, &free, &sym[3].u);
        ch.at_most(
            "R1: a at 0.9645 against 3.827194476e-5",
            (a_rel / 3.827194476e-5 - 1.0).abs(),
            1e-8,
        );
        ch.equals("R1: nu at 0.9645", ib.nu.unwrap_or(usize::MAX), 1);
        let atr = amp_trace(&fb, "AT", (sb, ib), &phi, 5, (0.5, 1.2));
        let rungs_ok = atr.points.len() == 6 && (1..=5).all(|k| atr.points[k].rung == Some(k));
        ch.holds("S5: rungs 1 to 5 accepted", rungs_ok);
        if !rungs_ok {
            return None;
        }
        let s5 = &atr.points[5];
        ch.at_most("S5: |lambda_5 - 0.958472223921|", (s5.s.p - LAMBDA_5).abs(), 1e-12);
        ch.at_most("S5: a_5 against 2.164988001e-4", (s5.a / A_5 - 1.0).abs(), 1e-9);
        ch.at_most(
            "S5: sigma_5 against -5.382683e-1",
            (s5.sigma / SIGMA_5 - 1.0).abs(),
            1e-6,
        );
        println!("D2: the rebuild to rung 4, {:.1} s", t_all.elapsed().as_secs_f64());
        // ADR 0723 D2: Da = (a_5 - a_4) / 8 from the rebuilt rungs.
        Some((phi, atr.points[4].s.clone(), (atr.points[5].a - atr.points[4].a) / 8.0))
    }

    /// The summary of a trace (TRC, TAL) and the FW1 rows.
    struct TraceOut {
        trace: FTrace,
        /// Per accepted state: `a` and the tally (none on the plain control).
        tallies: Vec<(f64, Option<Tally>)>,
    }

    /// A trace of reading c3 from `S4` on the model of `var` (WS with the commit of the driver's trial states; BT and TR
    /// without a commit; P the plain control without the tally).
    fn trace_of(
        ch: &mut Checks,
        tag: &str,
        mesh16: &SolidMesh,
        phi: &[f64],
        s4: &St,
        da: f64,
        var: Var,
    ) -> Option<TraceOut> {
        let t0 = Instant::now();
        let mut m = block_model(mesh16, K_L);
        var.apply(&mut m);
        let mut m2 = block_model_free_punch(mesh16, K_L);
        var.apply(&mut m2);
        let mut mp = block_model(mesh16, K_L);
        Var::P.apply(&mut mp);
        let mp2 = block_model_free_punch(mesh16, K_L);
        let fb = FemBranch::new(&m, &m2);
        let fp = FemBranch::new(&mp, &mp2);
        let mat = neo_hookean();
        let opt = match var {
            Var::Ws => Opt::Warm,
            Var::Bt => Opt::Update(IncompatibleModeUpdate::Backtracking),
            Var::Tr => Opt::Update(IncompatibleModeUpdate::TrustRegion),
            Var::P => Opt::Update(IncompatibleModeUpdate::Plain),
        };
        let start = if var == Var::Ws {
            let (c, fb_n, failed) = commit_driver(&m, &mat, s4);
            println!("{tag}: S4 committed with the driver's trial states: {fb_n} fallbacks, {failed} failed");
            ch.equals(&format!("W0 {tag}: elements whose commit at S4 failed"), failed, 0);
            c
        } else {
            s4.clone()
        };
        // W0, B0, T0: the free residual at the start against the plain model's at S4.
        let rp = fp.residual(s4);
        let ro = fb.residual(&start);
        let diff = match (&rp, &ro) {
            (Ok((a, _)), Ok((b, _))) if a.len() == b.len() => {
                Some(a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count())
            }
            _ => None,
        };
        println!(
            "{tag}: the free residual at the start evaluates: {}; entries differing from the plain model's at S4: {diff:?}",
            ro.is_ok()
        );
        match var {
            Var::Ws => ch.equals(
                &format!("W0 {tag}: the free residual at S4_W, entries differing from S4's"),
                diff.unwrap_or(usize::MAX),
                0,
            ),
            Var::Bt => ch.equals(
                &format!("B0 {tag}: the free residual at S4, entries differing from the plain model's"),
                diff.unwrap_or(usize::MAX),
                0,
            ),
            Var::Tr => match diff {
                Some(d) => ch.above(
                    &format!("T0 {tag}: entries differing from the plain model's"),
                    d as f64,
                    0.0,
                ),
                None => println!("T0 {tag}: the residual at S4 does not evaluate on the TR model: fails at rung 4"),
            },
            Var::P => {}
        }
        if let Err(e) = &ro {
            println!("TRC {tag}: the trace cannot start: {e}; stop: fails at rung 4");
            return None;
        }
        let i0 = analyse(&fb, &start);
        let p0 = apoint(&fb, start, i0, phi, Some(0));
        if var == Var::Ws {
            let ip = analyse(&fp, s4);
            let pp = apoint(&fp, s4.clone(), ip, phi, Some(0));
            ch.holds(
                &format!(
                    "W0 {tag}: sigma at S4_W {:+.15e} bit-identical to S4's {:+.15e}",
                    p0.sigma, pp.sigma
                ),
                p0.sigma.to_bits() == pp.sigma.to_bits(),
            );
            ch.holds(
                &format!("W0 {tag}: nu at S4_W {:?} equal to S4's {:?}", p0.info.nu, pp.info.nu),
                p0.info.nu == pp.info.nu && p0.info.nu.is_some(),
            );
        }
        let lmin = |s: &St| -> f64 {
            let el = element_of(&m, E_W, &s.u, &s.states);
            let alpha = match opt {
                Opt::Warm => committed_modes(&s.states[E_W]),
                Opt::Update(up) => rec(&mat, &el, up, None).1.ok(),
            };
            alpha
                .and_then(|a| mode_block(&mat, &el, &a).ok())
                .map(|(_, k)| spectrum(&k).1[0])
                .unwrap_or(f64::NAN)
        };
        let local = |it: Option<&St>, text: &str| match (it, element_in(text)) {
            (Some(s), Some(e)) if e < m.mesh().n_elements() => {
                let r = record_from(&mat, &element_of(&m, e, &s.u, &s.states), None);
                !r.0.is_empty() && (text.contains("'internal modes'") || text.contains("'deformation gradient'"))
            }
            _ => false,
        };
        let commit = |s: St| -> St {
            if var != Var::Ws {
                return s;
            }
            let (c, fb_n, failed) = commit_driver(&m, &mat, &s);
            println!("{tag} commit: {fb_n} fallbacks, {failed} failed");
            c
        };
        let prev: RefCell<Option<Vec<Option<[f64; MODES]>>>> = RefCell::new(None);
        let tallies: RefCell<Vec<(f64, Option<Tally>)>> = RefCell::new(Vec::new());
        let mut at = |p: &APoint| {
            if var == Var::P {
                tallies.borrow_mut().push((p.a, None));
                return;
            }
            let t1 = Instant::now();
            let t = tally(
                &m,
                &mat,
                &p.s.u,
                &p.s.states,
                prev.borrow().as_deref(),
                opt,
                var == Var::Ws,
            );
            print_tally(&format!("{tag} a {:+.9e}", p.a), &t);
            let fint = internal_force(&m, &p.s.u, &p.s.states).unwrap();
            let (r, _) = block_measures(&m, &p.s.u, &fint);
            println!(
                "TABLE {tag} a {:+.9e} lambda {:.12} dlambda/da {:+.6e} nu {:?} | plain failures {} (regular {}) | element {E_W} \
                 among them {} | R/mu {r:.8} | tally {:.1} s",
                p.a,
                p.s.p,
                p.sigma,
                p.info.nu,
                t.entries.len(),
                t.regular(),
                t.entries.iter().any(|x| x.e == E_W),
                t1.elapsed().as_secs_f64()
            );
            *prev.borrow_mut() = Some(t.roots.clone());
            tallies.borrow_mut().push((p.a, Some(t)));
        };
        let o = FineOpts {
            da,
            steps: 16,
            halvings: 6,
            cap: 40,
            time_cap: 300.0,
        };
        let ft = fine_trace(&fb, tag, p0, phi, &o, &lmin, &local, &commit, &mut at);
        let rd = reading(&ft);
        let n = ft.points.len();
        let last = &ft.points[n - 1].pt;
        let bracket = ft.sign.map(|j| (ft.points[j - 1].pt.a, ft.points[j].pt.a));
        let brackets = bracket
            .map(|(lo, hi)| ((lo / FOLD_LO - 1.0).abs() <= 1e-3) && ((hi / FOLD_HI - 1.0).abs() <= 1e-3))
            .unwrap_or(false);
        let its: Vec<usize> = ft.points[1..].iter().map(|f| f.its).collect();
        println!(
            "TRC {tag}: stop {}, reading {rd}, {} accepted states after the start, last a {:+.9e} (lambda {:.12}), {} solves, {} \
             failures, bordered iterations {its:?}, the sign change of dlambda/da {bracket:?}, brackets ADR 0723's fold {brackets}; \
             {}; {:.1} s",
            ft.stop,
            n - 1,
            last.a,
            last.s.p,
            ft.solves,
            ft.fails.len(),
            companions(&ft),
            t0.elapsed().as_secs_f64()
        );
        for (k, f) in ft.fails.iter().enumerate() {
            println!(
                "TRC {tag} failure {k} (from state {}, to {:+.9e}, local {}): {}",
                f.from, f.target, f.local, f.text
            );
        }
        let cmax = ft.points[1..]
            .iter()
            .map(|f| ((f.pt.a - f.target) / f.target).abs())
            .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
        if n > 1 {
            ch.at_most(
                &format!("FW1 {tag}: the constraint at the accepted states, relative"),
                cmax,
                1e-9,
            );
            ch.holds(
                &format!("FW1 {tag}: the continuity test at every accepted state"),
                ft.points[1..].iter().all(|f| f.corr <= f.step),
            );
        }
        let tallies = tallies.into_inner();
        let c6: Vec<String> = tallies
            .iter()
            .filter_map(|(a, t)| t.as_ref().map(|t| (a, t)))
            .filter(|(_, t)| t.regular() > 0)
            .map(|(a, t)| format!("a {a:.9e}: {} plain failures at regular roots", t.regular()))
            .collect();
        println!("TAL {tag}: the c6 states ({}): {c6:?}", c6.len());
        Some(TraceOut { trace: ft, tallies })
    }

    #[test]
    #[ignore = "ADR 0725: the heavy record of WS on ADR 0723's trace (D2), run once with --ignored"]
    fn f844_trace_warm_heavy() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let mesh16 = block_mesh(16);
        let Some((phi, s4, da)) = rebuild_to_rung_4(&mut ch, &mesh16) else {
            ch.finish();
            return;
        };
        if let Some(out) = trace_of(&mut ch, "WS", &mesh16, &phi, &s4, da, Var::Ws) {
            let ft = &out.trace;
            let lam: Vec<f64> = ft.points[1..].iter().map(|f| f.pt.s.p).collect();
            ch.equals("RW1: accepted states after the start", lam.len(), LAMBDA_WARM.len());
            for (k, (&l, &r)) in lam.iter().zip(&LAMBDA_WARM).enumerate() {
                ch.at_most(
                    &format!("RW1: |lambda - ADR 0723| at state {}", k + 1),
                    (l - r).abs(),
                    1e-11,
                );
            }
            ch.holds(&format!("RW2: the stop fold + 2 ({})", ft.stop), ft.stop == "fold + 2");
            ch.equals("RW2: solves", ft.solves, 12);
            let bracket = ft.sign.map(|j| (ft.points[j - 1].pt.a, ft.points[j].pt.a));
            ch.holds(
                &format!("RW2: the sign change {bracket:?} brackets ADR 0723's fold"),
                bracket
                    .map(|(lo, hi)| (lo / FOLD_LO - 1.0).abs() <= 1e-3 && (hi / FOLD_HI - 1.0).abs() <= 1e-3)
                    .unwrap_or(false),
            );
            let at_3720: Vec<f64> = out
                .tallies
                .iter()
                .filter(|(_, t)| {
                    t.as_ref()
                        .map(|t| t.entries.iter().any(|x| x.e == E_W))
                        .unwrap_or(false)
                })
                .map(|(a, _)| *a)
                .collect();
            ch.equals(
                &format!("RW3: states where element 3720's plain iteration from 0 fails ({at_3720:?})"),
                at_3720.len(),
                4,
            );
            if at_3720.len() == 4 {
                let worst = at_3720
                    .iter()
                    .zip(&A_PLAIN_FAILS)
                    .map(|(a, b)| (a / b - 1.0).abs())
                    .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
                ch.at_most("RW3: those states against ADR 0723's, relative", worst, 1e-8);
            }
            // TAL on WS's trace: BT and TR beside, at the plain failures.
            let (mut n, mut bt_same, mut tr_same, mut cont_same) = (0, 0, 0, 0);
            let mut c_bt = 0;
            let mut c_tr = 0;
            for (_, t) in &out.tallies {
                let Some(t) = t else { continue };
                let reg: Vec<&Entry> = t.entries.iter().filter(|x| x.lmin > 0.0).collect();
                n += reg.len();
                bt_same += reg.iter().filter(|x| x.beside.map(|b| b.0 .1).unwrap_or(false)).count();
                tr_same += reg.iter().filter(|x| x.beside.map(|b| b.1 .1).unwrap_or(false)).count();
                cont_same += reg.iter().filter(|x| x.same == Some(true)).count();
                if !reg.is_empty() {
                    c_bt += reg.iter().all(|x| x.beside.map(|b| b.0 .1).unwrap_or(false)) as usize;
                    c_tr += reg.iter().all(|x| x.beside.map(|b| b.1 .1).unwrap_or(false)) as usize;
                }
            }
            println!(
                "TAL WS: {n} plain failures at regular warm roots over the trace; the warm root the continuation at {cont_same}; BT \
                 from 0 reaches it at {bt_same}, TR from 0 at {tr_same}; C(BT) {c_bt}, C(TR) {c_tr} of the c6 states"
            );
        }
        println!("f844_trace_warm_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }

    #[test]
    #[ignore = "ADR 0725: the heavy record of BT on ADR 0723's trace with the plain control PLN (D2), run once with --ignored"]
    fn f844_trace_backtracking_heavy() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let mesh16 = block_mesh(16);
        let Some((phi, s4, da)) = rebuild_to_rung_4(&mut ch, &mesh16) else {
            ch.finish();
            return;
        };
        match trace_of(&mut ch, "PLN", &mesh16, &phi, &s4, da, Var::P) {
            Some(out) => {
                let ft = &out.trace;
                let lam1 = ft.points.get(1).map(|f| f.pt.s.p).unwrap_or(f64::NAN);
                ch.at_most("PLN: |lambda(t_1) - 0.959749331159|", (lam1 - LAMBDA_T1).abs(), 1e-9);
                let a2 = ft.points.get(2).map(|f| f.pt.a).unwrap_or(f64::NAN);
                ch.at_most(
                    "PLN: the second accepted a against 1.649773378e-4, relative",
                    (a2 / A_HALVED_PLAIN - 1.0).abs(),
                    1e-9,
                );
                ch.holds(&format!("PLN: the stop lost ({})", ft.stop), ft.stop == "lost");
                let rd = reading(ft);
                ch.holds(
                    &format!("PLN: the reading local breakdown: the local iteration ({rd})"),
                    rd == "local breakdown: the local iteration",
                );
            }
            None => ch.holds("PLN: the plain trace starts", false),
        }
        let _ = trace_of(&mut ch, "BT", &mesh16, &phi, &s4, da, Var::Bt);
        println!("f844_trace_backtracking_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }

    #[test]
    #[ignore = "ADR 0725: the heavy record of TR on ADR 0723's trace (D2), run once with --ignored"]
    fn f844_trace_trust_region_heavy() {
        let mut ch = Checks::new();
        let t_all = Instant::now();
        let mesh16 = block_mesh(16);
        let Some((phi, s4, da)) = rebuild_to_rung_4(&mut ch, &mesh16) else {
            ch.finish();
            return;
        };
        let _ = trace_of(&mut ch, "TR", &mesh16, &phi, &s4, da, Var::Tr);
        println!("f844_trace_trust_region_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
    }
}
