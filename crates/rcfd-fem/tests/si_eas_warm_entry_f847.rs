//! F-847 (ADR 0729): the public element entry points of the incompatible Hex8 given a committed state with the nine modes of
//! the warm start (ADR 0660 D1). The rule D1 (a state of the material layout followed by nine values is a warm state; the
//! material layout takes the path it took before; any other length is refused as before), the entry points against the
//! driver's own evaluation on the ADR 0727 patch tests under the warm start (WL, A1, WD1 to WD4, WP), the rule on one element
//! (WI, WS, NC1 to NC3, EX, EN, J), the text of the transient restore's refusal (RT1 to RT4) and the digest of the default
//! calls (DB, compared across two copies). The helpers are copies of those of `si_eas.rs`, `si_eas_hg.rs` and
//! `si_eas_warm_default_f846.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, Matrix6, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, condensed_force_and_tangent_backtracked, condensed_force_and_tangent_update,
    condensed_force_and_tangent_warm, enhanced_force_and_tangent, enhanced_force_and_tangent_with,
    mode_iteration_record, mode_iteration_record_from, modes_and_strains, modes_and_strains_with, trust_region_record,
    IncompatibleModeUpdate, IncompatibleStabilisation, ModeRecord, TrustRegionRecord,
};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_internal_force_at, hex8_second_piola_kirchhoff, solve, NewtonOptions,
};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::BTreeSet;

const INC: SolidFormulation = SolidFormulation::Incompatible;
/// The nine modes of an Incompatible Hex8.
const MODES: usize = 9;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const PLAIN: IncompatibleModeUpdate = IncompatibleModeUpdate::Plain;

// ------------------------------------------------------------------------------------------------------------- checks

struct Checks {
    missed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { missed: Vec::new() }
    }
    fn row(&mut self, name: &str, ok: bool, text: String) {
        println!("{} {name}: {text}", if ok { "met   " } else { "MISSED" });
        if !ok {
            self.missed.push(format!("{name}: {text}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool) {
        self.row(name, ok, String::new());
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        self.row(
            name,
            value.is_finite() && value <= limit,
            format!("{value:e} (limit <= {limit:e})"),
        );
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        self.row(
            name,
            value.is_finite() && value >= limit,
            format!("{value:e} (limit >= {limit:e})"),
        );
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        self.row(
            name,
            value.is_finite() && value > limit,
            format!("{value:e} (limit > {limit:e})"),
        );
    }
    fn text(&mut self, name: &str, got: &str, expected: &str) {
        self.row(name, got == expected, format!("{got:?} (expected {expected:?})"));
    }
    fn finish(self) {
        assert!(self.missed.is_empty(), "missed: {:#?}", self.missed);
    }
}

/// The values whose bits differ (infinite for different lengths).
fn bits(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return f64::INFINITY;
    }
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count() as f64
}

fn worst<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = 0.0f64;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    w
}

fn max_abs(v: &[f64]) -> f64 {
    worst(v.iter().map(|x| x.abs()))
}

fn flat6(s: &[[f64; 6]]) -> Vec<f64> {
    s.iter().flatten().copied().collect()
}

/// The values of a record of the local iteration (every field of every iterate, then the modes or NaN for an error).
fn mode_record_values(r: &ModeRecord) -> Vec<f64> {
    let mut v: Vec<f64> =
        r.0.iter()
            .flat_map(|i| [i.residual_norm, i.residual_max, i.scale, i.halvings as f64])
            .collect();
    match &r.1 {
        Ok(a) => v.extend_from_slice(a),
        Err(e) => v.extend(e.to_string().bytes().map(f64::from)),
    }
    v
}

fn trust_record_values(r: &TrustRegionRecord) -> Vec<f64> {
    let mut v: Vec<f64> =
        r.0.iter()
            .flat_map(|i| {
                [
                    i.residual_norm,
                    i.residual_max,
                    i.scale,
                    i.rejections as f64,
                    i.damping,
                    i.gain,
                ]
            })
            .collect();
    match &r.1 {
        Ok(a) => v.extend_from_slice(a),
        Err(e) => v.extend(e.to_string().bytes().map(f64::from)),
    }
    v
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
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

// ------------------------------------------------------------------------------------------ copies of the patch helpers

const E_MOD: f64 = 2.0e11;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, 7850.0, 1.2e-5).unwrap()
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

fn element_u(model: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    model
        .mesh()
        .element_nodes(e)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect()
}

fn voigt_green_lagrange(a: &[[f64; 3]; 3]) -> [f64; 6] {
    let e = |i: usize, j: usize| a[i][j] + a[j][i] + a[0][i] * a[0][j] + a[1][i] * a[1][j] + a[2][i] * a[2][j];
    [0.5 * e(0, 0), 0.5 * e(1, 1), 0.5 * e(2, 2), e(0, 1), e(1, 2), e(2, 0)]
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|i| (0..6).map(|j| d[i][j] * v[j]).sum())
}

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn newton(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps).threads(4)
}

/// The distorted unit Hex8 of `si_fbar.rs` (seed `0xF587_0506`).
fn distorted() -> Vec<Vec3> {
    let mut rng = Rng::new(0xF587_0506);
    let unit: [Vec3; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    unit.iter()
        .map(|p| [p[0] + rng.sym(0.1), p[1] + rng.sym(0.1), p[2] + rng.sym(0.1)])
        .collect()
}

const SEED: u64 = 0xF374_0275;
const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const P_DIV: [usize; 3] = [4, 3, 3];
const FIELD_C: Vec3 = [1e-3, -2e-3, 0.5e-3];
const FIELD_A: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, -0.2e-3],
    [0.5e-3, -0.7e-3, 0.4e-3],
    [0.1e-3, 0.6e-3, 0.9e-3],
];

fn field_a(scale: f64) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| scale * FIELD_A[i][j]))
}

fn rotation(n: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let (c, s) = (angle.cos(), angle.sin());
    let k = [[0.0, -n[2], n[1]], [n[2], 0.0, -n[0]], [-n[1], n[0], 0.0]];
    let k2 = mat3_mul(&k, &k);
    std::array::from_fn(|i| {
        std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 } + s * k[i][j] + (1.0 - c) * k2[i][j])
    })
}

fn field_grad(scale: f64, rotated: bool) -> [[f64; 3]; 3] {
    let a = field_a(scale);
    if !rotated {
        return a;
    }
    let s3 = 1.0 / 3f64.sqrt();
    let r = rotation([s3, s3, s3], std::f64::consts::PI / 3.0);
    let ia: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| a[i][j] + if i == j { 1.0 } else { 0.0 }));
    let f = mat3_mul(&r, &ia);
    std::array::from_fn(|i| std::array::from_fn(|j| f[i][j] - if i == j { 1.0 } else { 0.0 }))
}

fn linear_field(x: Vec3, g: &[[f64; 3]; 3]) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += g[i][j] * x[j];
        }
    }
    u
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

fn patch_mesh(amp: f64, tag: u64) -> SolidMesh {
    let base = hex_box(P_ORIGIN, P_LENGTHS, P_DIV).unwrap();
    let mut rng = Rng::new(SEED.wrapping_add(tag));
    let boundary = boundary_nodes(&base);
    let h = [
        P_LENGTHS[0] / P_DIV[0] as f64,
        P_LENGTHS[1] / P_DIV[1] as f64,
        P_LENGTHS[2] / P_DIV[2] as f64,
    ];
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for k in 0..3 {
                p[k] += amp * h[k] * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    rebuild(&base, nodes)
}

/// The patch model of the four patch tests with the warm start set (ADR 0729 WP): `Off` as `si_eas.rs` (no stabilisation
/// set), otherwise the rule set as `si_eas_hg.rs` and `f703/r_body.rs`.
fn warm_patch_model(material: Material, rule: IncompatibleStabilisation) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(patch_mesh(0.25, 1001), INC));
    if rule != OFF {
        m.set_incompatible_stabilisation(rule);
    }
    m.set_incompatible_mode_warm_start(true);
    m.add_material("all", material).unwrap();
    m
}

fn patch_prescribed(mesh: &SolidMesh, g: &[[f64; 3]; 3]) -> Vec<Prescribed> {
    let mut pres = Vec::new();
    for &n in &boundary_nodes(mesh) {
        let u = linear_field(mesh.nodes()[n], g);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    pres
}

fn interior_error(model: &SolidModel, u: &[f64], g: &[[f64; 3]; 3]) -> f64 {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let boundary = boundary_nodes(mesh);
    let mut umax = 0.0f64;
    let mut errs = Vec::new();
    for (n, x) in mesh.nodes().iter().enumerate() {
        let ue = linear_field(*x, g);
        for k in 0..3 {
            umax = umax.max(ue[k].abs());
            if !boundary.contains(&n) {
                errs.push((u[dofs.equation(n, k).unwrap()] - ue[k]).abs());
            }
        }
    }
    worst(errs) / umax
}

fn j2_patch_material() -> Material {
    Material::j2(steel(0.3), 1.0e8, 2.0e9, 1.0e9).unwrap()
}

/// `max |F - I| * max element edge` of the PM row.
fn pm_scale(model: &SolidModel, g: &[[f64; 3]; 3]) -> f64 {
    let gmax = worst(g.iter().flat_map(|row| row.iter().map(|v| v.abs())));
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let edge = worst((0..model.mesh().n_elements()).flat_map(|e| {
        let x = element_coords(model.mesh(), e);
        EDGES
            .iter()
            .map(move |&(a, b)| ((0..3).map(|k| (x[a][k] - x[b][k]).powi(2)).sum::<f64>()).sqrt())
    }));
    gmax * edge
}

// ----------------------------------------------------------------------------- WL, A1, WD1 to WD4, WP: the patch tests

#[test]
#[ignore = "ADR 0729 amendment 1: the record of the defective set-up as registered (the exact stress D E(F) computed for every case; the J2 material has no element-constant elasticity matrix, so the first J2 case panics before any WD row); superseded by wp_patch_tests_under_the_warm_start_amended"]
fn wp_patch_tests_under_the_warm_start() {
    let mut ch = Checks::new();
    let rules: [(&str, IncompatibleStabilisation); 4] = [
        ("si_eas", OFF),
        ("si_eas_hg", IncompatibleStabilisation::RestStress),
        ("si_eas_hg3_rd", IncompatibleStabilisation::Deviatoric),
        ("si_eas_hg3_rl", IncompatibleStabilisation::LinearStress),
    ];
    let ctx = PointContext::default();
    let (mut refusals, mut wd1, mut wd2, mut wd3, mut wd4_elastic) = (0usize, 0.0, 0.0, 0.0, 0.0);
    let mut wd4_j2 = 0.0f64;
    let mut wl = true;
    for (file, rule) in rules {
        let cases: Vec<(&str, Material, f64, bool, usize)> = vec![
            ("PT1 nu 0.3 FIELD_A", steel(0.3), 1.0, false, 1),
            ("PT1 nu 0.4999 FIELD_A", steel(0.4999), 1.0, false, 1),
            ("PT2 100 FIELD_A", steel(0.3), 100.0, false, 4),
            ("PT3 R(60 deg) (I + 100 FIELD_A)", steel(0.3), 100.0, true, 8),
            ("PT4 J2 FIELD_A", j2_patch_material(), 1.0, false, 4),
        ];
        for (name, mat, scale, rotated, steps) in cases {
            let label = format!("{file} {name}");
            let g = field_grad(scale, rotated);
            let m = warm_patch_model(mat.clone(), rule);
            let r = solve(&m, &patch_prescribed(m.mesh(), &g), &[], &newton(steps)).unwrap();
            let ns = mat.state_size();
            let n = 8 * ns;
            wl &= r.states.iter().all(|s| s.values().len() == n + MODES);
            let u = r.result.displacement_values();
            let scale_pm = pm_scale(&m, &g);
            let mut amax = Vec::new();
            let mut ps_errs = Vec::new();
            let e_ex = voigt_green_lagrange(&g);
            let s_ex = apply(&mat.elasticity_matrix().unwrap(), &e_ex);
            let smax = max_abs(&s_ex);
            for e in 0..m.mesh().n_elements() {
                let coords = element_coords(m.mesh(), e);
                let ue = element_u(&m, u, e);
                let st = &r.states[e];
                // The driver's own evaluation (ADR 0660 D1) of the same state.
                let warm = |upd| condensed_force_and_tangent_warm(&coords, &mat, &ue, st, &ctx, rule, upd);
                let Ok((df, dk, ds, dt)) = warm(PLAIN) else {
                    refusals += 1;
                    continue;
                };
                let dmodes = &dt[dt.len() - MODES..];
                match modes_and_strains_with(&coords, &mat, &ue, st, &ctx, rule) {
                    Ok((alpha, _)) => {
                        wd1 += bits(&alpha, dmodes);
                        amax.extend(alpha.iter().map(|a| a.abs()));
                        let committed = &st.values()[n..];
                        if ns == 0 {
                            wd4_elastic += bits(&alpha, committed);
                        } else {
                            let d = alpha.iter().zip(committed).map(|(a, b)| (a - b).abs());
                            wd4_j2 = wd4_j2.max(worst(d) / scale_pm);
                        }
                    }
                    Err(_) => refusals += 1,
                }
                match condensed_force_and_tangent(&coords, &mat, &ue, st, &ctx, rule) {
                    Ok((f, k, s)) => {
                        wd2 += bits(&f, &df) + bits(k.values(), dk.values()) + bits(&flat6(&s), &flat6(&ds));
                        if name.starts_with("PT2") && rule != OFF {
                            ps_errs.extend(s.iter().flat_map(|sq| (0..6).map(|k| (sq[k] - s_ex[k]).abs() / smax)));
                        }
                    }
                    Err(_) => refusals += 1,
                }
                for (upd, got) in [
                    (
                        IncompatibleModeUpdate::Backtracking,
                        condensed_force_and_tangent_backtracked(&coords, &mat, &ue, st, &ctx, rule),
                    ),
                    (
                        IncompatibleModeUpdate::BacktrackingWithDecrease,
                        condensed_force_and_tangent_update(
                            &coords,
                            &mat,
                            &ue,
                            st,
                            &ctx,
                            rule,
                            IncompatibleModeUpdate::BacktrackingWithDecrease,
                        ),
                    ),
                ] {
                    match (got, warm(upd)) {
                        (Ok((f, k, s)), Ok((wf, wk, ws, _))) => {
                            wd2 += bits(&f, &wf) + bits(k.values(), wk.values()) + bits(&flat6(&s), &flat6(&ws));
                        }
                        _ => refusals += 1,
                    }
                }
                if rule == OFF {
                    match (
                        hex8_force_and_tangent_at(INC, &coords, &mat, &ue, st, &ctx),
                        hex8_internal_force_at(INC, &coords, &mat, &ue, st, &ctx),
                        hex8_second_piola_kirchhoff(INC, &coords, &mat, &ue, st),
                    ) {
                        (Ok((f, k)), Ok(fi), Ok(s)) => {
                            wd3 += bits(&f, &df) + bits(k.values(), dk.values()) + bits(&fi, &df);
                            wd3 += bits(&flat6(&s), &flat6(&ds));
                            if name.starts_with("PT2") {
                                ps_errs.extend(s.iter().flat_map(|sq| (0..6).map(|k| (sq[k] - s_ex[k]).abs() / smax)));
                            }
                        }
                        _ => refusals += 1,
                    }
                }
            }
            if name.starts_with("PT4") {
                let yielded = r.states.iter().all(|s| {
                    s.values()[..n]
                        .chunks(rcfd_fem::mat_j2::STATE_SIZE)
                        .all(|p| p[12] > 0.0)
                });
                ch.holds(
                    &format!("WP {label} set-up: every point yielded (material part)"),
                    yielded,
                );
            }
            ch.at_most(&format!("WP {label}: interior error"), interior_error(&m, u, &g), 1e-10);
            ch.at_most(
                &format!("WP PM {label}: max |alpha| / (max |F - I| h)"),
                worst(amax) / scale_pm,
                1e-10,
            );
            if name.starts_with("PT2") {
                ch.at_most(
                    &format!("WP PS {label}: S at every point against D E(F)"),
                    worst(ps_errs),
                    1e-10,
                );
            }
        }
    }
    ch.holds("WL every committed state of the 20 warm solves has n + 9 values", wl);
    ch.at_most(
        "A1 refusals of the entry points given the warm states",
        refusals as f64,
        0.0,
    );
    ch.at_most(
        "WD1 modes_and_strains_with against the driver's trial modes: values differing",
        wd1,
        0.0,
    );
    ch.at_most(
        "WD2 condensed_force_and_tangent{,_backtracked,_update} against the driver: values differing",
        wd2,
        0.0,
    );
    ch.at_most("WD3 hex8_* (Off) against the driver: values differing", wd3, 0.0);
    ch.at_most(
        "WD4 elastic: modes against the committed modes, values differing",
        wd4_elastic,
        0.0,
    );
    ch.at_most("WD4 J2: max |d alpha| / (max |F - I| h)", wd4_j2, 2e-10);
    ch.finish();
}

/// ADR 0729 amendment 1: as `wp_patch_tests_under_the_warm_start`, with the exact stress of PS computed for PT2 only (as
/// the originals do).
#[test]
fn wp_patch_tests_under_the_warm_start_amended() {
    let mut ch = Checks::new();
    let rules: [(&str, IncompatibleStabilisation); 4] = [
        ("si_eas", OFF),
        ("si_eas_hg", IncompatibleStabilisation::RestStress),
        ("si_eas_hg3_rd", IncompatibleStabilisation::Deviatoric),
        ("si_eas_hg3_rl", IncompatibleStabilisation::LinearStress),
    ];
    let ctx = PointContext::default();
    let (mut refusals, mut wd1, mut wd2, mut wd3, mut wd4_elastic) = (0usize, 0.0, 0.0, 0.0, 0.0);
    let mut wd4_j2 = 0.0f64;
    let mut wl = true;
    for (file, rule) in rules {
        let cases: Vec<(&str, Material, f64, bool, usize)> = vec![
            ("PT1 nu 0.3 FIELD_A", steel(0.3), 1.0, false, 1),
            ("PT1 nu 0.4999 FIELD_A", steel(0.4999), 1.0, false, 1),
            ("PT2 100 FIELD_A", steel(0.3), 100.0, false, 4),
            ("PT3 R(60 deg) (I + 100 FIELD_A)", steel(0.3), 100.0, true, 8),
            ("PT4 J2 FIELD_A", j2_patch_material(), 1.0, false, 4),
        ];
        for (name, mat, scale, rotated, steps) in cases {
            let label = format!("{file} {name}");
            let g = field_grad(scale, rotated);
            let m = warm_patch_model(mat.clone(), rule);
            let r = solve(&m, &patch_prescribed(m.mesh(), &g), &[], &newton(steps)).unwrap();
            let ns = mat.state_size();
            let n = 8 * ns;
            wl &= r.states.iter().all(|s| s.values().len() == n + MODES);
            let u = r.result.displacement_values();
            let scale_pm = pm_scale(&m, &g);
            let mut amax = Vec::new();
            let mut ps_errs = Vec::new();
            // Amendment 1: the exact stress of PS for PT2 only (the J2 material has no elasticity matrix).
            let s_ex = if name.starts_with("PT2") {
                apply(&mat.elasticity_matrix().unwrap(), &voigt_green_lagrange(&g))
            } else {
                [0.0; 6]
            };
            let smax = max_abs(&s_ex);
            for e in 0..m.mesh().n_elements() {
                let coords = element_coords(m.mesh(), e);
                let ue = element_u(&m, u, e);
                let st = &r.states[e];
                // The driver's own evaluation (ADR 0660 D1) of the same state.
                let warm = |upd| condensed_force_and_tangent_warm(&coords, &mat, &ue, st, &ctx, rule, upd);
                let Ok((df, dk, ds, dt)) = warm(PLAIN) else {
                    refusals += 1;
                    continue;
                };
                let dmodes = &dt[dt.len() - MODES..];
                match modes_and_strains_with(&coords, &mat, &ue, st, &ctx, rule) {
                    Ok((alpha, _)) => {
                        wd1 += bits(&alpha, dmodes);
                        amax.extend(alpha.iter().map(|a| a.abs()));
                        let committed = &st.values()[n..];
                        if ns == 0 {
                            wd4_elastic += bits(&alpha, committed);
                        } else {
                            let d = alpha.iter().zip(committed).map(|(a, b)| (a - b).abs());
                            wd4_j2 = wd4_j2.max(worst(d) / scale_pm);
                        }
                    }
                    Err(_) => refusals += 1,
                }
                match condensed_force_and_tangent(&coords, &mat, &ue, st, &ctx, rule) {
                    Ok((f, k, s)) => {
                        wd2 += bits(&f, &df) + bits(k.values(), dk.values()) + bits(&flat6(&s), &flat6(&ds));
                        if name.starts_with("PT2") && rule != OFF {
                            ps_errs.extend(s.iter().flat_map(|sq| (0..6).map(|k| (sq[k] - s_ex[k]).abs() / smax)));
                        }
                    }
                    Err(_) => refusals += 1,
                }
                for (upd, got) in [
                    (
                        IncompatibleModeUpdate::Backtracking,
                        condensed_force_and_tangent_backtracked(&coords, &mat, &ue, st, &ctx, rule),
                    ),
                    (
                        IncompatibleModeUpdate::BacktrackingWithDecrease,
                        condensed_force_and_tangent_update(
                            &coords,
                            &mat,
                            &ue,
                            st,
                            &ctx,
                            rule,
                            IncompatibleModeUpdate::BacktrackingWithDecrease,
                        ),
                    ),
                ] {
                    match (got, warm(upd)) {
                        (Ok((f, k, s)), Ok((wf, wk, ws, _))) => {
                            wd2 += bits(&f, &wf) + bits(k.values(), wk.values()) + bits(&flat6(&s), &flat6(&ws));
                        }
                        _ => refusals += 1,
                    }
                }
                if rule == OFF {
                    match (
                        hex8_force_and_tangent_at(INC, &coords, &mat, &ue, st, &ctx),
                        hex8_internal_force_at(INC, &coords, &mat, &ue, st, &ctx),
                        hex8_second_piola_kirchhoff(INC, &coords, &mat, &ue, st),
                    ) {
                        (Ok((f, k)), Ok(fi), Ok(s)) => {
                            wd3 += bits(&f, &df) + bits(k.values(), dk.values()) + bits(&fi, &df);
                            wd3 += bits(&flat6(&s), &flat6(&ds));
                            if name.starts_with("PT2") {
                                ps_errs.extend(s.iter().flat_map(|sq| (0..6).map(|k| (sq[k] - s_ex[k]).abs() / smax)));
                            }
                        }
                        _ => refusals += 1,
                    }
                }
            }
            if name.starts_with("PT4") {
                let yielded = r.states.iter().all(|s| {
                    s.values()[..n]
                        .chunks(rcfd_fem::mat_j2::STATE_SIZE)
                        .all(|p| p[12] > 0.0)
                });
                ch.holds(
                    &format!("WP {label} set-up: every point yielded (material part)"),
                    yielded,
                );
            }
            ch.at_most(&format!("WP {label}: interior error"), interior_error(&m, u, &g), 1e-10);
            ch.at_most(
                &format!("WP PM {label}: max |alpha| / (max |F - I| h)"),
                worst(amax) / scale_pm,
                1e-10,
            );
            if name.starts_with("PT2") {
                ch.at_most(
                    &format!("WP PS {label}: S at every point against D E(F)"),
                    worst(ps_errs),
                    1e-10,
                );
            }
        }
    }
    ch.holds("WL every committed state of the 20 warm solves has n + 9 values", wl);
    ch.at_most(
        "A1 refusals of the entry points given the warm states",
        refusals as f64,
        0.0,
    );
    ch.at_most(
        "WD1 modes_and_strains_with against the driver's trial modes: values differing",
        wd1,
        0.0,
    );
    ch.at_most(
        "WD2 condensed_force_and_tangent{,_backtracked,_update} against the driver: values differing",
        wd2,
        0.0,
    );
    ch.at_most("WD3 hex8_* (Off) against the driver: values differing", wd3, 0.0);
    ch.at_most(
        "WD4 elastic: modes against the committed modes, values differing",
        wd4_elastic,
        0.0,
    );
    ch.at_most("WD4 J2: max |d alpha| / (max |F - I| h)", wd4_j2, 2e-10);
    ch.finish();
}

// ------------------------------------------------------------------------- WI, WS, NC1 to NC3, EX, EN, J: one element

fn neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(8.0e10, 1.6e11).unwrap(), 7850.0).unwrap()
}

/// The J2 case (c) and the neo-Hookean case (e) of `si_eas.rs` `t1_t2_t3_t4_x1_consistency` (the same draws of the same
/// generator): `(material, displacement, material state)`.
fn unit_cases() -> [(Material, Vec<f64>, ElementState); 2] {
    let x = distorted();
    let mut rng = Rng::new(0xF596_0608 + 1);
    // (a), (b): their draws.
    for _ in 0..48 {
        rng.sym(1.0);
    }
    // (c) J2 from a committed state.
    let j2 = Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap();
    let mut st = Vec::new();
    for _ in 0..8 {
        let ep: Vec<f64> = (0..6).map(|_| rng.sym(2e-4)).collect();
        let mut beta: Vec<f64> = (0..6).map(|_| rng.sym(1e7)).collect();
        let mean = (beta[0] + beta[1] + beta[2]) / 3.0;
        for b in beta.iter_mut().take(3) {
            *b -= mean;
        }
        st.extend(ep);
        st.extend(beta);
        st.push(3e-4);
    }
    let uc: Vec<f64> = (0..24).map(|_| rng.sym(3e-3)).collect();
    // (d): its draws.
    for _ in 0..(8 * 6 + 24) {
        rng.sym(1.0);
    }
    // (e) neo-Hookean at a large strain.
    let b = [[0.5, 0.2, -0.1], [0.1, -0.3, 0.2], [0.3, 0.1, 0.4]];
    let ue: Vec<f64> = x
        .iter()
        .flat_map(|p| (0..3).map(move |i| 0.3 * (b[i][0] * p[0] + b[i][1] * p[1] + b[i][2] * p[2])))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|v| v + rng.sym(5e-2))
        .collect();
    [
        (j2, uc, ElementState::new(st)),
        (neo_hookean(), ue, ElementState::new(Vec::new())),
    ]
}

fn with_tail(st: &ElementState, tail: &[f64]) -> ElementState {
    let mut v = st.values().to_vec();
    v.extend_from_slice(tail);
    ElementState::new(v)
}

/// Every one of the 13 entry points of ADR 0729 D2 that take a state, given `st`: per entry its values or its error.
fn all_entries(x: &[Vec3], mat: &Material, u: &[f64], st: &ElementState) -> Vec<Result<Vec<f64>, FemError>> {
    let ctx = PointContext::default();
    let alpha = [1e-3; MODES];
    let s = Some(&alpha);
    let fkt = |r: rcfd_fem::Result<(Vec<f64>, rcfd_fem::element::ElementMatrix, Vec<[f64; 6]>)>| {
        r.map(|(f, k, s)| {
            let mut v = f;
            v.extend_from_slice(k.values());
            v.extend(flat6(&s));
            v
        })
    };
    let fk = |r: rcfd_fem::Result<(Vec<f64>, rcfd_fem::element::ElementMatrix)>| {
        r.map(|(f, k)| {
            let mut v = f;
            v.extend_from_slice(k.values());
            v
        })
    };
    let ms = |r: rcfd_fem::Result<([f64; MODES], Vec<[f64; 6]>)>| {
        r.map(|(a, s)| {
            let mut v = a.to_vec();
            v.extend(flat6(&s));
            v
        })
    };
    vec![
        ms(modes_and_strains(x, mat, u, st, &ctx)),
        ms(modes_and_strains_with(
            x,
            mat,
            u,
            st,
            &ctx,
            IncompatibleStabilisation::RestStress,
        )),
        fkt(condensed_force_and_tangent(x, mat, u, st, &ctx, OFF)),
        fkt(condensed_force_and_tangent_backtracked(x, mat, u, st, &ctx, OFF)),
        fkt(condensed_force_and_tangent_update(
            x,
            mat,
            u,
            st,
            &ctx,
            OFF,
            IncompatibleModeUpdate::BacktrackingWithDecrease,
        )),
        mode_iteration_record(x, mat, u, st, &ctx, OFF, PLAIN).map(|r| mode_record_values(&r)),
        mode_iteration_record_from(x, mat, u, st, &ctx, OFF, PLAIN, s).map(|r| mode_record_values(&r)),
        trust_region_record(x, mat, u, st, &ctx, OFF, s).map(|r| trust_record_values(&r)),
        fk(enhanced_force_and_tangent(x, mat, u, &alpha, st, &ctx)),
        fk(enhanced_force_and_tangent_with(
            x,
            mat,
            u,
            &alpha,
            st,
            &ctx,
            IncompatibleStabilisation::RestStress,
        )),
        fk(hex8_force_and_tangent_at(INC, x, mat, u, st, &ctx)),
        hex8_internal_force_at(INC, x, mat, u, st, &ctx),
        hex8_second_piola_kirchhoff(INC, x, mat, u, st).map(|s| flat6(&s)),
    ]
}

#[test]
fn wu_rule_on_one_element() {
    let mut ch = Checks::new();
    let x = distorted();
    let ctx = PointContext::default();
    let cases = unit_cases();
    let mut refusals = 0usize;

    // NC1: the lengths n + 1, n + 8, n + 10, n + 18, at the 13 entry points, refused as before.
    let mut nc1 = 0usize;
    let mut nc1_total = 0usize;
    for (mat, u, st) in &cases {
        let n = st.values().len();
        for extra in [1usize, 8, 10, 18] {
            let bad = with_tail(st, &vec![0.0; extra]);
            for out in all_entries(&x, mat, u, &bad) {
                nc1_total += 1;
                if let Err(FemError::FieldLength { key, expected, found }) = &out {
                    if *key == "state" && *expected == n && *found == n + extra {
                        nc1 += 1;
                        continue;
                    }
                }
                println!("NC1 n {n}, L {}: {:?}", n + extra, out.as_ref().err());
            }
        }
    }
    ch.row(
        "NC1 wrong lengths refused with FieldLength { state, n, L }",
        nc1 == 104 && nc1_total == 104,
        format!("{nc1} of {nc1_total} (limit 104 of 104)"),
    );

    // NC3: Full and BBar refuse a state of 9 values (rule of the Incompatible element only).
    let nh = neo_hookean();
    let nine = ElementState::new(vec![0.0; MODES]);
    let nc3 = [SolidFormulation::Full, SolidFormulation::BBar]
        .iter()
        .filter(|&&f| {
            matches!(
                hex8_force_and_tangent_at(f, &x, &nh, &cases[1].1, &nine, &ctx),
                Err(FemError::FieldLength { key: "state", .. })
            )
        })
        .count();
    ch.row(
        "NC3 Full and BBar refuse a 9-value state",
        nc3 == 2,
        format!("{nc3} of 2"),
    );

    for (ci, (mat, u, st)) in cases.iter().enumerate() {
        let tag = if ci == 0 { "(c) J2" } else { "(e) neo-Hookean" };
        let (a_star, _) = modes_and_strains(&x, mat, u, st, &ctx).unwrap();
        let warm = with_tail(st, &a_star);
        let shifted: Vec<f64> = a_star.iter().map(|a| a * (1.0 + 1e-3)).collect();
        let s_shift: [f64; MODES] = std::array::from_fn(|k| shifted[k]);
        let warm_nan = with_tail(st, &[f64::NAN; MODES]);

        // EN: enhanced_force_and_tangent{,_with} ignore the committed modes.
        let mut en = 0.0;
        for w in [&warm, &warm_nan] {
            for rule in [OFF, IncompatibleStabilisation::RestStress] {
                match (
                    enhanced_force_and_tangent_with(&x, mat, u, &a_star, w, &ctx, rule),
                    enhanced_force_and_tangent_with(&x, mat, u, &a_star, st, &ctx, rule),
                ) {
                    (Ok((f1, k1)), Ok((f0, k0))) => en += bits(&f1, &f0) + bits(k1.values(), k0.values()),
                    _ => refusals += 1,
                }
            }
            match (
                enhanced_force_and_tangent(&x, mat, u, &a_star, w, &ctx),
                enhanced_force_and_tangent(&x, mat, u, &a_star, st, &ctx),
            ) {
                (Ok((f1, k1)), Ok((f0, k0))) => en += bits(&f1, &f0) + bits(k1.values(), k0.values()),
                _ => refusals += 1,
            }
        }
        ch.at_most(
            &format!("EN {tag}: enhanced given the warm state against the material state"),
            en,
            0.0,
        );

        if ci == 0 {
            // J: the J2 element (n = 104), the warm state against the driver's evaluation of it.
            let mut j = 0.0;
            match (
                condensed_force_and_tangent(&x, mat, u, &warm, &ctx, OFF),
                condensed_force_and_tangent_warm(&x, mat, u, &warm, &ctx, OFF, PLAIN),
                modes_and_strains(&x, mat, u, &warm, &ctx),
            ) {
                (Ok((f, k, s)), Ok((wf, wk, ws, wt)), Ok((a, _))) => {
                    j += bits(&f, &wf) + bits(k.values(), wk.values()) + bits(&flat6(&s), &flat6(&ws));
                    j += bits(&a, &wt[wt.len() - MODES..]);
                }
                _ => refusals += 1,
            }
            ch.at_most(
                "J (c) J2: the warm state against the driver's evaluation: values differing",
                j,
                0.0,
            );
            continue;
        }

        // WI: the iteration from the committed root returns at iterate 0.
        let cold = mode_iteration_record(&x, mat, u, st, &ctx, OFF, PLAIN).unwrap();
        let from_root = mode_iteration_record(&x, mat, u, &warm, &ctx, OFF, PLAIN);
        ch.at_least("WI (e) cold record: iterates", cold.0.len() as f64, 2.0);
        match &from_root {
            Ok((its, Ok(a))) => {
                ch.at_most("WI (e) warm record from a*: iterates", its.len() as f64, 1.0);
                ch.at_least(
                    "WI (e) warm record from a*: iterates (at least 1)",
                    its.len() as f64,
                    1.0,
                );
                ch.at_most(
                    "WI (e) warm record outcome against a*: values differing",
                    bits(a, &a_star),
                    0.0,
                );
            }
            other => {
                refusals += 1;
                ch.holds(&format!("WI (e) warm record from a*: {other:?}"), false);
            }
        }
        match modes_and_strains(&x, mat, u, &warm, &ctx) {
            Ok((a, _)) => ch.at_most(
                "WI (e) modes_and_strains from a* against a*: values differing",
                bits(&a, &a_star),
                0.0,
            ),
            Err(_) => refusals += 1,
        }

        // WS: the start is used.
        match modes_and_strains(&x, mat, u, &with_tail(st, &shifted), &ctx) {
            Ok((a, _)) => {
                ch.above(
                    "WS (e) from a* (1 + 1e-3): values differing from a*",
                    bits(&a, &a_star),
                    0.0,
                );
                let d: Vec<f64> = a.iter().zip(&a_star).map(|(p, q)| p - q).collect();
                ch.at_most("WS (e) max |d alpha| / max |a*|", max_abs(&d) / max_abs(&a_star), 1e-8);
            }
            Err(_) => refusals += 1,
        }

        // NC2: NaN modes: the fallback from alpha = 0, the cold results.
        let mut nc2 = 0.0;
        match (
            modes_and_strains(&x, mat, u, &warm_nan, &ctx),
            modes_and_strains(&x, mat, u, st, &ctx),
        ) {
            (Ok((a1, s1)), Ok((a0, s0))) => nc2 += bits(&a1, &a0) + bits(&flat6(&s1), &flat6(&s0)),
            _ => refusals += 1,
        }
        match (
            condensed_force_and_tangent(&x, mat, u, &warm_nan, &ctx, OFF),
            condensed_force_and_tangent(&x, mat, u, st, &ctx, OFF),
        ) {
            (Ok((f1, k1, s1)), Ok((f0, k0, s0))) => {
                nc2 += bits(&f1, &f0) + bits(k1.values(), k0.values()) + bits(&flat6(&s1), &flat6(&s0))
            }
            _ => refusals += 1,
        }
        match mode_iteration_record(&x, mat, u, &warm_nan, &ctx, OFF, PLAIN) {
            Ok(r) => nc2 += bits(&mode_record_values(&r), &mode_record_values(&cold)),
            Err(_) => refusals += 1,
        }
        ch.at_most("NC2 (e) NaN modes against the cold calls: values differing", nc2, 0.0);

        // EX: a given start is used as given; None from a warm state as mode_iteration_record.
        let mut ex = 0.0;
        match (
            mode_iteration_record_from(&x, mat, u, &warm, &ctx, OFF, PLAIN, Some(&s_shift)),
            mode_iteration_record_from(&x, mat, u, st, &ctx, OFF, PLAIN, Some(&s_shift)),
        ) {
            (Ok(r1), Ok(r0)) => ex += bits(&mode_record_values(&r1), &mode_record_values(&r0)),
            _ => refusals += 1,
        }
        match (
            trust_region_record(&x, mat, u, &warm, &ctx, OFF, Some(&s_shift)),
            trust_region_record(&x, mat, u, st, &ctx, OFF, Some(&s_shift)),
        ) {
            (Ok(r1), Ok(r0)) => ex += bits(&trust_record_values(&r1), &trust_record_values(&r0)),
            _ => refusals += 1,
        }
        match (
            mode_iteration_record_from(&x, mat, u, &warm, &ctx, OFF, PLAIN, None),
            mode_iteration_record(&x, mat, u, &warm, &ctx, OFF, PLAIN),
        ) {
            (Ok(r1), Ok(r0)) => ex += bits(&mode_record_values(&r1), &mode_record_values(&r0)),
            _ => refusals += 1,
        }
        ch.at_most("EX (e) given starts and None: values differing", ex, 0.0);
    }
    ch.at_most(
        "A1 refusals of the entry points given the unit warm states",
        refusals as f64,
        0.0,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------- RT1 to RT4: the restore's text

fn bar(warm: bool) -> (SolidModel, Vec<LoadHistory>) {
    let mut m = SolidModel::new(with_formulation(
        hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(),
        INC,
    ));
    m.set_incompatible_mode_warm_start(warm);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let n = m.dof_map().n_equations();
    let mut pattern = vec![0.0; n];
    for &nd in m.mesh().node_set("xmax").unwrap() {
        pattern[m.dof_map().equation(nd, 1).unwrap()] = 1.0e8;
    }
    let loads = vec![LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())];
    (m, loads)
}

fn integrator(m: &SolidModel, loads: &[LoadHistory]) -> Integrator {
    Integrator::new(m, loads, &NlTransientOptions::new(1e-5).threads(4)).unwrap()
}

#[test]
fn rt_restore_refusal_text() {
    let mut ch = Checks::new();
    let (mw, loads) = bar(true);
    let (md, _) = bar(false);
    let n = mw.dof_map().n_equations();
    let mut first = integrator(&mw, &loads);
    first.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    first.run(3).unwrap();
    let saved = first.state().unwrap();
    let text = |r: rcfd_fem::Result<()>| r.err().map(|e| e.to_string()).unwrap_or_else(|| "accepted".to_string());
    ch.text(
        "RT1 the default integrator, the warm state",
        &text(integrator(&md, &loads).restore(saved.clone())),
        "key 'state': the material state of element 0 has 9 values where the model's layout has 0",
    );
    let mut five = saved.clone();
    five.states[2] = ElementState::new(vec![0.0; 5]);
    ch.text(
        "RT2 the warm integrator, element 2 of 5 values",
        &text(integrator(&mw, &loads).restore(five)),
        "key 'state': the material state of element 2 has 5 values where the model's layout has 0 (or 9 with the nine modes of the warm start)",
    );
    let mut short = saved.clone();
    short.states.pop();
    ch.text(
        "RT3 the warm integrator, 3 elements",
        &text(integrator(&mw, &loads).restore(short)),
        "key 'state': material states of 3 elements that do not match the model's layout (4 elements)",
    );
    ch.text(
        "RT4 the warm integrator, the warm state",
        &text(integrator(&mw, &loads).restore(saved)),
        "accepted",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------- DB: the digest of the default calls

#[test]
fn db_default_digest() {
    let x = distorted();
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    let mut values = 0usize;
    for (mat, u, st) in unit_cases() {
        let n = st.values().len();
        for len in [n, n + 1, n + 8, n + 10, n + 18] {
            let given = with_tail(&st, &vec![0.0; len - n]);
            for out in all_entries(&x, &mat, &u, &given) {
                match out {
                    Ok(v) => {
                        values += v.len();
                        for x in v {
                            eat(&x.to_bits().to_le_bytes());
                        }
                    }
                    Err(e) => eat(e.to_string().as_bytes()),
                }
            }
        }
    }
    println!("DB digest: {h:#018x} over {values} values");
}
