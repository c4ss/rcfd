//! F-298 (ADR 0221) part (b): what the flux correction's missing boundary term on skewed fixed-pressure faces (ADR 0204 Q3) does
//! to the mass balance of a converged run. Limits are those registered in
//! docs/adr/0221-f298-registration-electric-current-and-joule-heat-with-the-gradient-options-and-the-fixed-pressure-flux-correction.md
//! before this file was written.
//!
//! CH: ADR 0204's channel on the skewed wedge mesh W (velocity inlet `xmin`, fixed-pressure outlet `xmax`), SIMPLE with one
//! non-orthogonal corrector, run to its iteration cap past the residual floor. S-CH: the default run is converged (residuals and
//! the continuity of the cells away from the outlet). M: the default run's domain mass balance `m_dom` and the largest continuity
//! error `e_b` of the outlet cells, in units of the inflow; the mismatch matters if either exceeds 1e-6. M-O3: with
//! `gradient_boundary = "dropped"` on the pressure (no boundary term in the equation either) both fall to the converged level.
//! M-sets (B-tree-T): the other option sets without a boundary term in the flux keep the miss. M-C: the default with SIMPLEC and
//! its compensation (supplementary).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::pressure::divergence;

/// S-CH: the converged level of the momentum and pressure residuals and of the continuity away from the outlet.
const RES_U: f64 = 1e-12;
const RES_P: f64 = 1e-10;
const CONVERGED: f64 = 1e-8;
/// M: the mismatch matters above this (relative to the inflow).
const MATTERS: f64 = 1e-6;
/// P: the smallest maximum non-orthogonality of W, degrees.
const SKEWED: f64 = 20.0;

/// One option set: name, the `[solver]` lines.
const SETS: [(&str, &str); 7] = [
    ("O1", "gradient_face_correction = true\n"),
    ("O2", "gradient_boundary = \"corrected\"\n"),
    ("O3", "gradient_boundary = \"dropped\"\n"),
    ("O4", "gradient_secondary_limiter = \"barth_jespersen\"\n"),
    ("O5", "gradient_secondary_limiter = \"venkatakrishnan\"\n"),
    (
        "O6",
        "gradient_secondary_limiter = \"venkatakrishnan\"\ngradient_limiter_k = 0.5\n",
    ),
    (
        "C2",
        "gradient_face_correction = true\ngradient_boundary = \"corrected\"\ngradient_secondary_limiter = \"venkatakrishnan\"\n",
    ),
];

fn keys(name: &str) -> &'static str {
    SETS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k).expect("a set")
}

/// The run lines of CH (`simplec` false) and CH-C.
fn algorithm(simplec: bool) -> &'static str {
    if simplec {
        "algorithm = \"simplec\"\nalpha_u = 0.7\nsimplec_non_orthogonal_compensation = true\nmax_iterations = 3000\ntolerance_u = 1e-12\ntolerance_p = 1e-12\n"
    } else {
        "algorithm = \"simple\"\nalpha_u = 0.7\nalpha_p = 0.3\nmax_iterations = 3000\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n"
    }
}

/// The case text of CH or CH-C with a set's keys (empty: the default, without the table).
fn text(simplec: bool, keys: &str) -> String {
    let table = if keys.is_empty() {
        ""
    } else {
        "[solver.gradient_equations]\npressure = true\n"
    };
    format!(
        r#"title = "f298 channel"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
nu = 0.1
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
convection = "linear_upwind"
non_orthogonal_correctors = 1
{}{keys}{table}"#,
        algorithm(simplec)
    )
}

/// What a finished run of CH gives.
struct Run {
    iterations: usize,
    failure: Option<String>,
    res_u: f64,
    res_p: f64,
    q_in: f64,
    m_dom: f64,
    e_b: f64,
    e_i: f64,
    hash: u64,
}

impl Run {
    /// The convergence of S-CH.
    fn converged(&self) -> bool {
        self.failure.is_none() && self.res_u <= RES_U && self.res_p <= RES_P && self.e_i <= CONVERGED
    }
    fn line(&self, label: &str) -> String {
        format!(
            "{label}: {} iterations, failure {:?}, res_u {:.2e}, res_p {:.2e}, Q_in {:.6}, m_dom {:.3e}, e_b {:.3e}, e_i {:.3e}, hash {:016x}",
            self.iterations, self.failure, self.res_u, self.res_p, self.q_in, self.m_dom, self.e_b, self.e_i, self.hash
        )
    }
}

/// 64-bit FNV-1a over the bits of the values.
fn fnv(h: &mut u64, v: &[f64]) {
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Run CH (or CH-C) with a set's keys to its end, step by step, and measure it.
fn run(simplec: bool, keys: &str) -> Run {
    let t = text(simplec, keys);
    let case = Case::parse_toml(&t).unwrap_or_else(|d| panic!("case does not parse: {d:?}\n{t}"));
    let mut s = Simulation::new(&case).unwrap_or_else(|d| panic!("not built: {d:?}"));
    let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
    assert!(
        q.max_non_orthogonality >= SKEWED,
        "P set-up: W {} degrees",
        q.max_non_orthogonality
    );
    let (mut iterations, mut res_u, mut res_p) = (0, f64::NAN, f64::NAN);
    for _ in 0..10_000 {
        let st = s.step();
        iterations = st.index;
        res_u = st.res_u;
        res_p = st.res_p;
        if st.done {
            break;
        }
    }
    let failure = s.failure().map(|f| format!("{f:?}"));
    let geom = s.geom.clone();
    let mesh = &geom.mesh;
    let phi = s.flux().to_vec();
    let q_in = -mesh.patch("xmin").expect("xmin").faces().map(|f| phi[f]).sum::<f64>();
    let net: f64 = (geom.n_internal..mesh.n_faces())
        .filter(|&f| !geom.empty[f])
        .map(|f| phi[f])
        .sum();
    let div = divergence(&geom, &phi);
    let mut beside = vec![false; geom.n_cells()];
    for f in mesh.patch("xmax").expect("xmax").faces() {
        beside[mesh.owner()[f]] = true;
    }
    assert!(beside.iter().any(|&b| b), "set-up: xmax has cells");
    let (mut e_b, mut e_i) = (0.0_f64, 0.0_f64);
    for (c, d) in div.iter().enumerate() {
        if beside[c] {
            e_b = e_b.max(d.abs());
        } else {
            e_i = e_i.max(d.abs());
        }
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    fnv(&mut hash, &phi);
    for name in ["ux", "uy", "p"] {
        fnv(&mut hash, &s.field(name).unwrap());
    }
    Run {
        iterations,
        failure,
        res_u,
        res_p,
        q_in,
        m_dom: net.abs() / q_in,
        e_b: e_b / q_in,
        e_i: e_i / q_in,
        hash,
    }
}

/// S-CH and M (and M-C, supplementary): the converged default run and what the missing boundary term does to its mass balance.
#[test]
fn s_ch_and_m_the_default_mismatch_in_a_converged_run() {
    let r = run(false, "");
    println!("{}", r.line("M CH default"));
    assert!(
        r.converged(),
        "S-CH set-up: the default run is not converged (limits res_u {RES_U:e}, res_p {RES_P:e}, e_i {CONVERGED:e})"
    );
    let matters = r.m_dom > MATTERS || r.e_b > MATTERS;
    println!(
        "M decision: the mismatch {} (m_dom {:.3e}, e_b {:.3e} against {MATTERS:e})",
        if matters { "MATTERS" } else { "does not matter" },
        r.m_dom,
        r.e_b
    );
    let c = run(true, "");
    println!(
        "{} (supplementary, converged {})",
        c.line("M-C CH-C default"),
        c.converged()
    );
}

/// M-O3: with the boundary term dropped from the pressure equation too, the converged run closes its mass balance.
#[test]
fn m_o3_control_dropped_boundary_term_closes_the_balance() {
    let r = run(false, keys("O3"));
    println!("{}", r.line("M-O3 CH O3"));
    let mut missed = Vec::new();
    if !r.converged() {
        missed.push("M-O3: the run is not converged".to_string());
    }
    if r.m_dom > CONVERGED || r.e_b > CONVERGED {
        missed.push(format!(
            "M-O3: m_dom {:e}, e_b {:e} (limit {CONVERGED:e})",
            r.m_dom, r.e_b
        ));
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// M-sets and B-tree-T: the option sets without a boundary term in the flux (F-291's code) keep the outlet's continuity error;
/// O1, O2 and C2 must miss by more than 1e-6.
#[test]
#[ignore = "record of commit 3834e1af (ADR 0221 amendment 1): it measures F-291's flux correction, which D3 replaced; numbers in the addendum"]
fn m_sets_before_d3() {
    let mut missed = Vec::new();
    for (name, k) in SETS {
        if name == "O3" {
            continue;
        }
        let r = run(false, k);
        println!("{} (converged {})", r.line(&format!("M-sets CH {name}")), r.converged());
        if matches!(name, "O1" | "O2" | "C2") && r.e_b <= MATTERS {
            missed.push(format!("B-tree-T {name}: e_b {:e} (limit > {MATTERS:e})", r.e_b));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// B-tree: after D3 every option set's converged run closes its mass balance, the outlet cells included.
#[test]
fn b_tree_the_fixed_pressure_term_closes_the_mass_balance() {
    let mut missed = Vec::new();
    for (name, k) in SETS {
        if name == "O3" {
            continue;
        }
        let r = run(false, k);
        println!("{} (converged {})", r.line(&format!("B-tree CH {name}")), r.converged());
        if !r.converged() {
            missed.push(format!("B-tree {name}: the run is not converged (NOT MET)"));
        } else if r.m_dom > CONVERGED || r.e_b > CONVERGED {
            missed.push(format!(
                "B-tree {name}: m_dom {:e}, e_b {:e} (limit {CONVERGED:e})",
                r.m_dom, r.e_b
            ));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// The hashes M and M-O3 printed at commit 3834e1af (before D3), recorded for B-D.
const HASH_DEFAULT_M: u64 = 0x4a8d_6c1e_2e8a_48f2;
const HASH_O3_M: u64 = 0x85ca_4cac_6091_df27;

/// B-D: D3 changes no bit of the default run and of the O3 run (no boundary term under `dropped`).
#[test]
fn b_d_default_and_dropped_runs_unchanged_by_d3() {
    let d = run(false, "");
    let o3 = run(false, keys("O3"));
    println!(
        "B-D: default hash {:016x} (M {HASH_DEFAULT_M:016x}), O3 hash {:016x} (M-O3 {HASH_O3_M:016x})",
        d.hash, o3.hash
    );
    assert_eq!((d.hash, o3.hash), (HASH_DEFAULT_M, HASH_O3_M), "B-D");
}
