//! ADR 0058 check B10: distributed gradient and steady convection-diffusion assembly on a partitioned cavity.

mod common;

use common::*;
use rcfd_core::vec3::{self, Vec3};
use rcfd_dist::assemble::{
    default_linear_solve, green_gauss_gradient, serial_linear_solve, solve_transport, RankGeometry, ScalarTransport,
};
use rcfd_dist::{run_ranks, Communicator, HaloPlan, SerialComm};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::partition::Decomposition;

fn rotating(x: Vec3) -> Vec3 {
    [-(x[1] - 0.5), x[0] - 0.5, 0.0]
}

fn lid_bc(patch: &str, _x: Vec3) -> f64 {
    if patch == "lid" {
        1.0
    } else {
        0.0
    }
}

fn linear(x: Vec3) -> f64 {
    0.3 + 1.2 * x[0] - 0.7 * x[1]
}

fn linear_bc(_p: &str, x: Vec3) -> f64 {
    linear(x)
}

fn smooth(x: Vec3) -> f64 {
    (2.0 * x[0]).sin() * (3.0 * x[1]).cos() + x[0] * x[1]
}

fn smooth_bc(_p: &str, x: Vec3) -> f64 {
    smooth(x)
}

fn gradient_global(mesh: &Mesh, p: usize, f: fn(Vec3) -> f64, prob: &ScalarTransport) -> Vec<[f64; 3]> {
    let dec: Decomposition = decompose(mesh, p);
    let geoms: Vec<RankGeometry> = (0..p)
        .map(|r| RankGeometry::build(mesh, &dec, r, prob).expect("geometry"))
        .collect();
    let cc = mesh.cell_centres();
    let out = run_ranks(p, |comm| {
        let s = &dec.subdomains[comm.rank()];
        let plan = HaloPlan::from_subdomain(s);
        let mut phi: Vec<f64> = s.owned.iter().map(|&c| f(cc[c])).collect();
        phi.resize(s.n_local(), 0.0);
        green_gauss_gradient(&geoms[comm.rank()], &plan, comm, &mut phi).expect("gradient")
    });
    let mut g = vec![[0.0; 3]; mesh.n_cells()];
    for (r, comp) in out.iter().enumerate() {
        for (i, &c) in dec.subdomains[r].owned.iter().enumerate() {
            g[c] = [comp[0][i], comp[1][i], comp[2][i]];
        }
    }
    g
}

#[test]
fn b10_gradient() {
    let mesh = cavity(32);
    // linear field: exact in cells whose faces are all interior (and, with exact boundary values, also elsewhere)
    let prob = ScalarTransport {
        gamma: 0.05,
        velocity: rotating,
        bc: linear_bc,
    };
    let b = [1.2, -0.7, 0.0];
    let n_int = mesh.n_internal_faces();
    let mut touches_boundary = vec![false; mesh.n_cells()];
    for f in n_int..mesh.n_faces() {
        touches_boundary[mesh.owner()[f]] = true;
    }
    for p in [1usize, 2, 4, 8] {
        let g = gradient_global(&mesh, p, linear, &prob);
        let (mut worst_int, mut worst_all) = (0.0f64, 0.0f64);
        for c in 0..mesh.n_cells() {
            let e = vec3::norm(vec3::sub(g[c], b));
            worst_all = worst_all.max(e);
            if !touches_boundary[c] {
                worst_int = worst_int.max(e);
            }
        }
        println!("B10 linear gradient P={p}: max error interior cells {worst_int:e}, all cells {worst_all:e} (limit 1.2e-12 interior)");
        assert!(
            worst_int <= 1e-12 * vec3::norm(b),
            "P={p}: interior gradient error {worst_int:e}"
        );
    }
    // smooth field: distributed versus P = 1
    let prob = ScalarTransport {
        gamma: 0.05,
        velocity: rotating,
        bc: smooth_bc,
    };
    let g1 = gradient_global(&mesh, 1, smooth, &prob);
    let gmax = g1.iter().map(|g| vec3::norm(*g)).fold(0.0, f64::max);
    for p in [2usize, 4, 8] {
        let g = gradient_global(&mesh, p, smooth, &prob);
        let d = g
            .iter()
            .zip(&g1)
            .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
            .fold(0.0, f64::max);
        println!(
            "B10 smooth gradient P={p}: max |g_P - g_1| = {d:e} (limit {:e})",
            1e-13 * gmax
        );
        assert!(d <= 1e-13 * gmax, "P={p}: gradient differs by {d:e}");
    }
}

fn run_transport(mesh: &Mesh, p: usize, serial_solver: bool) -> (Vec<f64>, usize, f64, usize) {
    let prob = ScalarTransport {
        gamma: 0.05,
        velocity: rotating,
        bc: lid_bc,
    };
    let dec = decompose(mesh, p);
    let geoms: Vec<RankGeometry> = (0..p)
        .map(|r| RankGeometry::build(mesh, &dec, r, &prob).expect("geometry"))
        .collect();
    let work = |comm: &dyn Communicator| {
        let r = comm.rank();
        let res = if serial_solver {
            solve_transport(
                &geoms[r],
                &dec.subdomains[r],
                comm,
                &prob,
                1e-12,
                300,
                &mut serial_linear_solve,
            )
        } else {
            solve_transport(
                &geoms[r],
                &dec.subdomains[r],
                comm,
                &prob,
                1e-12,
                300,
                &mut default_linear_solve,
            )
        };
        res.expect("transport solve")
    };
    let results = if p == 1 {
        vec![work(&SerialComm)]
    } else {
        run_ranks(p, |c| work(c))
    };
    let mut phi = vec![0.0; mesh.n_cells()];
    for (r, res) in results.iter().enumerate() {
        for (&c, v) in dec.subdomains[r].owned.iter().zip(&res.phi) {
            phi[c] = *v;
        }
    }
    (phi, results[0].outer, results[0].change, results[0].inner)
}

#[test]
fn b10_convection_diffusion() {
    let mesh = cavity(32);
    let (p1, outer1, change1, inner1) = run_transport(&mesh, 1, false);
    assert!(
        change1 <= 1e-12,
        "P=1 outer iteration did not converge: change {change1:e} after {outer1}"
    );
    let (lo, hi) = p1
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
    println!("B10 transport P=1: outer {outer1}, inner {inner1}, change {change1:e}, phi in [{lo:.6}, {hi:.6}]");
    assert!(
        hi > 0.5 && lo > -0.2 && hi < 1.2,
        "implausible solution range [{lo}, {hi}]"
    );
    // P = 1 equals the serial rcfd-linalg path bit for bit
    let (ps, outer_s, _, _) = run_transport(&mesh, 1, true);
    assert_eq!(outer_s, outer1);
    assert_eq!(
        ps.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        p1.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        "P=1 distributed assembly + solve must equal the serial linalg solve bitwise"
    );
    let range = hi - lo;
    for p in [2usize, 4, 8] {
        let (ph, outer, change, inner) = run_transport(&mesh, p, false);
        let d = ph.iter().zip(&p1).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        println!(
            "B10 transport P={p}: outer {outer}, inner {inner}, change {change:e}, max |phi_P - phi_1| = {d:e} (limit {:e})",
            1e-9 * range
        );
        assert!(change <= 1e-12, "P={p}: outer iteration did not converge");
        assert!(d <= 1e-9 * range, "P={p}: converged field differs by {d:e}");
    }
}

fn at_rest(_x: Vec3) -> Vec3 {
    [0.0; 3]
}

/// Sanity of the assembly itself (not a registered limit of the ADR): without convection the discrete problem reproduces a
/// linear field exactly (orthogonal mesh, Dirichlet values from the field), also when distributed.
#[test]
fn b10_pure_diffusion_reproduces_linear_field() {
    let mesh = cavity(16);
    let prob = ScalarTransport {
        gamma: 0.05,
        velocity: at_rest,
        bc: linear_bc,
    };
    for p in [1usize, 4] {
        let dec = decompose(&mesh, p);
        let geoms: Vec<RankGeometry> = (0..p)
            .map(|r| RankGeometry::build(&mesh, &dec, r, &prob).expect("geometry"))
            .collect();
        let out = run_ranks(p, |comm| {
            let r = comm.rank();
            solve_transport(
                &geoms[r],
                &dec.subdomains[r],
                comm,
                &prob,
                1e-12,
                50,
                &mut default_linear_solve,
            )
            .expect("solve")
        });
        let cc = mesh.cell_centres();
        let mut worst = 0.0f64;
        for (r, res) in out.iter().enumerate() {
            for (&c, v) in dec.subdomains[r].owned.iter().zip(&res.phi) {
                worst = worst.max((v - linear(cc[c])).abs());
            }
        }
        println!("B10 pure diffusion P={p}: max |phi - linear| = {worst:e}");
        assert!(worst <= 1e-9, "P={p}: {worst:e}");
    }
}
