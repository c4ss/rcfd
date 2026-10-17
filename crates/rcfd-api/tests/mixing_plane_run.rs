//! Mixing plane in the steady solver loop (F-126, ADR 0059 checks MP1-MP9). The limits are those registered in the ADR before
//! this file was written. A criterion that misses moves unchanged into an ignored `*_known_miss` test.
//!
//! Set-up MP: `annulus_3d_stage`, r_in = 0.5, r_out = 1, lz = 0.8, 48 cells around, 8 radial layers, 4 + 6 axial layers; the exact
//! solution is the free vortex `u = W z + (Gamma/r) theta`, `p = p0 - Gamma^2/(2 r^2)` plus uniform axial flow, exact for any frozen
//! rotor speed. No published data is used.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_fv::pressure::{boundary_values, pressure_bc};

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const W: f64 = 1.0;
const GAMMA: f64 = 0.4;
const VREF: f64 = 1.2806248474865698; // sqrt(W^2 + (Gamma/r_in)^2)
const PSCALE: f64 = 0.24; // Gamma^2/2 (1/r_in^2 - 1/r_out^2)

fn exact_u(x: [f64; 3]) -> [f64; 3] {
    let r2 = x[0] * x[0] + x[1] * x[1];
    [-GAMMA * x[1] / r2, GAMMA * x[0] / r2, W]
}

fn exact_p(x: [f64; 3]) -> f64 {
    -GAMMA * GAMMA / (2.0 * (x[0] * x[0] + x[1] * x[1]))
}

struct Opt {
    nx_right: usize,
    relax: f64,
    tol: f64,
    distort: bool,
    hub: f64,
    iterations: usize,
    extra: String,
}

impl Default for Opt {
    fn default() -> Opt {
        Opt {
            nx_right: 0,
            relax: 0.5,
            tol: 1e-9,
            distort: false,
            hub: 0.5,
            iterations: 3000,
            extra: String::new(),
        }
    }
}

fn text(o: &Opt) -> String {
    let uz = if o.distort {
        "1 [m/s] * (1 + 0.3 * x / sqrt(x * x + y * y))"
    } else {
        "1 [m/s]"
    };
    let swirl = |name: &str| {
        format!(
            "[boundaries.{name}]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-0.4 [m/s] * y * 1 [m] / (x * x + y * y)\"\nvelocity_expression_y = \"0.4 [m/s] * x * 1 [m] / (x * x + y * y)\"\nvelocity_expression_z = \"1 [m/s]\"\n"
        )
    };
    let p_hub = -GAMMA * GAMMA / (2.0 * o.hub * o.hub);
    format!(
        r#"
[mesh]
generator = "annulus_3d_stage"
nx = 48
ny = 8
nz = 4
nx_right = {nx_right}
nz_right = 6
r_in = {RI}
r_out = {RO}
lz = 0.8
[physics]
nu = 0.1
rho = 1.0
[initial]
velocity_expression_z = "1 [m/s]"
{inner}{outer}[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "-0.4 [m/s] * y * 1 [m] / (x * x + y * y)"
velocity_expression_y = "0.4 [m/s] * x * 1 [m] / (x * x + y * y)"
velocity_expression_z = "{uz}"
[boundaries.plane_out]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.plane_in]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.zmax]
bc_kind = "radial_equilibrium_outlet"
swirl_axis = [0.0, 0.0, 1.0]
bc_pressure = {p_hub:?}
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = {iterations}
tolerance_u = 1e-10
tolerance_p = 1e-10
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 5.0
zone = "box"
box_min = [-2.0, -2.0, -1.0]
box_max = [2.0, 2.0, 0.4]
[mixing_plane]
enabled = true
upstream_patch = "plane_out"
downstream_patch = "plane_in"
bands = 8
relax = {relax:?}
tolerance = {tol:e}
{extra}
"#,
        nx_right = o.nx_right,
        inner = swirl("inner"),
        outer = swirl("outer"),
        relax = o.relax,
        tol = o.tol,
        iterations = o.iterations,
        extra = o.extra,
    )
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}\n{t}"))
}

/// The hub reference of the outlet: the smallest face-centre radius of `zmax`.
fn hub_radius(o: &Opt) -> f64 {
    let sim = Simulation::new(&parse(&text(o))).unwrap_or_else(|e| panic!("{e:?}"));
    let m = &sim.geom.mesh;
    let p = m.patch("zmax").unwrap();
    p.faces()
        .map(|f| m.face_centres()[f])
        .map(|x| x[0].hypot(x[1]))
        .fold(f64::MAX, f64::min)
}

fn run(mut o: Opt) -> Simulation {
    o.hub = hub_radius(&o);
    let t = text(&o);
    let mut sim = Simulation::new(&parse(&t)).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(o.iterations);
    println!(
        "  {n} iterations, converged {}, stop: {:?}, res_mixing {:?}, res_u {:?}",
        sim.is_converged(),
        sim.stop_reason(),
        sim.monitors.last("res_mixing"),
        sim.monitors.last("res_u")
    );
    sim
}

struct Planes {
    /// Per band: volume flux, angular momentum flux `sum phi r u_theta`, mechanical energy flux.
    up: Vec<[f64; 3]>,
    dn: Vec<[f64; 3]>,
    /// Polar velocity (u_r, u_theta, u_z) of every plane_in face with its band.
    inlet: Vec<(usize, [f64; 3], f64)>,
}

fn planes(sim: &Simulation) -> Planes {
    let g = &sim.geom;
    let m = &g.mesh;
    let mx = sim.mixing().expect("mixing plane");
    let s = sim.simple_solver().unwrap();
    let ub = s.u.boundary_values(g);
    let pbc = pressure_bc(g, &s.u, &s.p);
    let pb = boundary_values(g, &pbc.coeffs, &s.p.values);
    let (fc, own, ni) = (m.face_centres(), m.owner(), g.n_internal);
    let nb = mx.plane().n_bands();
    let phi = sim.flux();
    let torque = |x: [f64; 3], u: [f64; 3]| x[0] * u[1] - x[1] * u[0];
    let mut up = vec![[0.0; 3]; nb];
    for &f in mx.up_faces() {
        let b = mx.plane().band_of(fc[f]).unwrap();
        let u = s.u.values[own[f]];
        up[b][0] += phi[f];
        up[b][1] += phi[f] * torque(fc[f], u);
        up[b][2] += phi[f] * (pb[f - ni] + 0.5 * (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]));
    }
    let mut dn = vec![[0.0; 3]; nb];
    let mut inlet = Vec::new();
    for &f in mx.dn_faces() {
        let b = mx.plane().band_of(fc[f]).unwrap();
        let u = ub[f - ni];
        dn[b][0] -= phi[f];
        dn[b][1] -= phi[f] * torque(fc[f], u);
        dn[b][2] -= phi[f] * (pb[f - ni] + 0.5 * (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]));
        let (r, x) = (fc[f][0].hypot(fc[f][1]), fc[f]);
        let ur = (x[0] * u[0] + x[1] * u[1]) / r;
        let ut = (x[0] * u[1] - x[1] * u[0]) / r;
        inlet.push((b, [ur, ut, u[2]], r));
    }
    Planes { up, dn, inlet }
}

/// Worst relative per-band mismatch of component `k` and of the totals.
fn mismatch(p: &Planes, k: usize) -> (f64, f64) {
    let mut worst: f64 = 0.0;
    let (mut su, mut sd) = (0.0, 0.0);
    for (u, d) in p.up.iter().zip(&p.dn) {
        worst = worst.max((u[k] - d[k]).abs() / u[k].abs());
        su += u[k];
        sd += d[k];
    }
    (worst, (su - sd).abs() / su.abs())
}

fn exact_error(sim: &Simulation) -> (f64, f64) {
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let p = sim.field("p").unwrap();
    let mut du: f64 = 0.0;
    let mut dp = Vec::new();
    for c in 0..cc.len() {
        let e = exact_u(cc[c]);
        du = du.max((ux[c] - e[0]).abs().max((uy[c] - e[1]).abs()).max((uz[c] - e[2]).abs()));
        dp.push(p[c] - exact_p(cc[c]));
    }
    let mean = dp.iter().sum::<f64>() / dp.len() as f64;
    (du, dp.iter().map(|d| (d - mean).abs()).fold(0.0, f64::max))
}

fn inlet_error(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let mx = sim.mixing().unwrap();
    let ub = sim.simple_solver().unwrap().u.boundary_values(g);
    let fc = g.mesh.face_centres();
    mx.dn_faces()
        .iter()
        .map(|&f| {
            let e = exact_u(fc[f]);
            let u = ub[f - g.n_internal];
            (u[0] - e[0]).abs().max((u[1] - e[1]).abs()).max((u[2] - e[2]).abs())
        })
        .fold(0.0, f64::max)
}

fn duct_mass(sim: &Simulation) -> f64 {
    let m = &sim.geom.mesh;
    let phi = sim.flux();
    let sum = |name: &str| m.patch(name).unwrap().faces().map(|f| phi[f]).sum::<f64>();
    (sum("zmin") + sum("zmax")).abs() / sum("zmin").abs()
}

/// Angular momentum flux through a patch from the solver flux and the boundary face velocity.
fn duct_torque(sim: &Simulation, name: &str) -> f64 {
    let g = &sim.geom;
    let ub = sim.simple_solver().unwrap().u.boundary_values(g);
    let (phi, fc) = (sim.flux(), g.mesh.face_centres());
    g.mesh
        .patch(name)
        .unwrap()
        .faces()
        .map(|f| {
            let u = ub[f - g.n_internal];
            phi[f] * (fc[f][0] * u[1] - fc[f][1] * u[0])
        })
        .sum()
}

#[test]
fn mp1_to_mp4_free_vortex_through_the_plane() {
    let sim = run(Opt::default());
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let res = sim.monitors.last("res_mixing").unwrap();
    let p = planes(&sim);
    let (mq_band, mq_tot) = mismatch(&p, 0);
    let (mg_band, _) = mismatch(&p, 1);
    let (du, dp) = exact_error(&sim);
    let din = inlet_error(&sim);
    let inout = duct_mass(&sim);
    let (g_in, g_out) = (duct_torque(&sim, "zmin"), duct_torque(&sim, "zmax"));
    let drel = (g_in + g_out).abs() / g_in.abs();
    println!(
        "MP1 res_mixing {res:.3e} (1e-8), converged {}; MP2 mass band {mq_band:.3e} (1e-7) total {mq_tot:.3e} (1e-9) duct {inout:.3e} (1e-9); MP3 torque band {mg_band:.3e} (1e-7) duct in {g_in:.6} out {g_out:.6} rel {drel:.3e} (1e-2); MP4 |u-ue| {:.3e} V (2e-2), p {:.3e} P (0.05), plane_in {:.3e} V (2e-2)",
        sim.is_converged(),
        du / VREF,
        dp / PSCALE,
        din / VREF
    );
    assert!(res <= 1e-8, "MP1 {res:e}");
    assert!(mq_band <= 1e-7, "MP2 band {mq_band:e}");
    assert!(mq_tot <= 1e-9, "MP2 total {mq_tot:e}");
    assert!(inout <= 1e-9, "MP2 duct {inout:e}");
    assert!(mg_band <= 1e-7, "MP3 band {mg_band:e}");
    assert!(drel <= 1e-2, "MP3 duct {drel:e}");
    assert!(du <= 2e-2 * VREF, "MP4 velocity {:e}", du / VREF);
    assert!(din <= 2e-2 * VREF, "MP4 plane_in {:e}", din / VREF);
}

fn bits(sim: &Simulation) -> Vec<u64> {
    let mut v: Vec<u64> = ["ux", "uy", "uz", "p"]
        .iter()
        .flat_map(|n| sim.field(n).unwrap())
        .map(f64::to_bits)
        .collect();
    v.extend(sim.flux().iter().map(|x| x.to_bits()));
    v
}

fn tc_text(extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "annulus_3d_interface"
nx = 24
ny = 6
nz = 2
ny_right = 8
nz_right = 3
r_in = 0.5
r_out = 1.0
lz = 0.4
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_expression_x = "-1.0 [m/s] * y / 1 [m]"
velocity_expression_y = "1.0 [m/s] * x / 1 [m]"
[boundaries.outer]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
[boundaries.zmin]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 60
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 0.5
zone = "box"
box_min = [-2.0, -2.0, -1.0]
box_max = [2.0, 2.0, 0.2]
{extra}
"#
    )
}

/// MP5: unset keys, bit-identical to the case without the section.
#[test]
fn mp5_disabled_section_is_bit_identical() {
    let mut a = Simulation::new(&parse(&tc_text(""))).unwrap();
    let mut b = Simulation::new(&parse(&tc_text("[mixing_plane]\nenabled = false\n"))).unwrap();
    let mut c = Simulation::new(&parse(&tc_text(
        "[mixing_plane]\nenabled = false\nbands = 5\nrelax = 0.3\nupstream_patch = \"zmin\"\n",
    )))
    .unwrap();
    a.run(60);
    b.run(60);
    c.run(60);
    assert!(b.mixing().is_none() && c.mixing().is_none());
    assert_eq!(bits(&a), bits(&b));
    assert_eq!(bits(&a), bits(&c));
    // the monitor is absent when the plane is off
    assert!(a.monitors.last("res_mixing").is_none());
}

/// MP6: the converged state does not depend on the relaxation.
#[test]
fn mp6_relaxation_independence() {
    let a = run(Opt {
        relax: 0.3,
        ..Opt::default()
    });
    let b = run(Opt {
        relax: 0.7,
        ..Opt::default()
    });
    let mut d: f64 = 0.0;
    for n in ["ux", "uy", "uz"] {
        let (x, y) = (a.field(n).unwrap(), b.field(n).unwrap());
        for c in 0..x.len() {
            d = d.max((x[c] - y[c]).abs());
        }
    }
    println!("MP6 max |u(0.3) - u(0.7)| = {:.3e} V (1e-6)", d / VREF);
    assert!(a.failure().is_none() && b.failure().is_none());
    assert!(d <= 1e-6 * VREF, "MP6 {:e}", d / VREF);
}

/// MP7: circumferential distortion of the inlet is mixed out by the plane.
#[test]
fn mp7_circumferential_mixing() {
    let sim = run(Opt {
        distort: true,
        ..Opt::default()
    });
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let p = planes(&sim);
    let (mq, _) = mismatch(&p, 0);
    let (mg, _) = mismatch(&p, 1);
    let nb = p.up.len();
    let mut spread: f64 = 0.0;
    for b in 0..nb {
        for k in 0..3 {
            let v: Vec<f64> = p.inlet.iter().filter(|x| x.0 == b).map(|x| x.1[k]).collect();
            let (lo, hi) = (
                v.iter().cloned().fold(f64::MAX, f64::min),
                v.iter().cloned().fold(f64::MIN, f64::max),
            );
            spread = spread.max(hi - lo);
        }
    }
    let (eu, ed): (f64, f64) = (p.up.iter().map(|x| x[2]).sum(), p.dn.iter().map(|x| x[2]).sum());
    let loss = (eu - ed) / eu.abs();
    println!(
        "MP7 mass band {mq:.3e} (1e-7), torque band {mg:.3e} (1e-7), plane_in polar spread {spread:.3e} (1e-12), energy flux up {eu:.6} down {ed:.6}, loss/E {loss:.3e} (>= -1e-6)"
    );
    assert!(mq <= 1e-7 && mg <= 1e-7, "conservation {mq:e} {mg:e}");
    assert!(spread <= 1e-12, "spread {spread:e}");
    assert!(eu - ed >= -1e-6 * eu.abs(), "energy created {loss:e}");
}

/// MP8: different sector counts on the two sides.
#[test]
fn mp8_different_sector_counts() {
    let sim = run(Opt {
        nx_right: 36,
        ..Opt::default()
    });
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let p = planes(&sim);
    let (mq, _) = mismatch(&p, 0);
    let (mg, _) = mismatch(&p, 1);
    let (du, _) = exact_error(&sim);
    println!(
        "MP8 mass band {mq:.3e} (1e-7), torque band {mg:.3e} (1e-7), |u-ue| {:.3e} V (3e-2)",
        du / VREF
    );
    assert!(mq <= 1e-7 && mg <= 1e-7, "conservation {mq:e} {mg:e}");
    assert!(du <= 3e-2 * VREF, "MP8 {:e}", du / VREF);
}

fn errors(t: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(t) {
        Err(e) => e,
        Ok(c) => c.semantic_errors(),
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

/// MP9: located errors, one per mutation of a valid case.
#[test]
fn mp9_located_errors() {
    let base = text(&Opt::default());
    let ok = errors(&base);
    assert!(ok.is_empty(), "valid input: {ok:?}");
    let mutate = |from: &str, to: &str| {
        assert!(base.contains(from), "{from}");
        base.replacen(from, to, 1)
    };
    let cases: Vec<(String, &str)> = vec![
        (
            mutate("downstream_patch = \"plane_in\"", "downstream_patch = \"plane_out\""),
            "mixing_plane.downstream_patch",
        ),
        (
            mutate(
                "[boundaries.plane_in]\n",
                "[boundaries.plane_in]\nbc_kind = \"pressure_outlet\"\n",
            ),
            "boundaries.plane_in.bc_kind",
        ),
        (
            mutate(
                "[boundaries.plane_out]\nvelocity = \"zero_gradient\"",
                "[boundaries.plane_out]\nvelocity = \"fixed\"",
            ),
            "boundaries.plane_out.velocity",
        ),
        (
            mutate(
                "pressure = \"fixed\"\npressure_value = 0.0\n[boundaries.plane_in]",
                "[boundaries.plane_in]",
            ),
            "boundaries.plane_out.pressure",
        ),
        (
            mutate(
                "[boundaries.plane_in]\nvelocity = \"fixed\"",
                "[boundaries.plane_in]\nvelocity = \"zero_gradient\"",
            ),
            "boundaries.plane_in.velocity",
        ),
        (mutate("bands = 8\n", ""), "mixing_plane.bands"),
        (
            mutate("bands = 8\n", "bands = 8\nr_min = 0.9\nr_max = 0.6\n"),
            "mixing_plane.r_max",
        ),
        (
            mutate("bands = 8\n", "bands = 8\naxis = [0.0, 0.0, 0.0]\n"),
            "mixing_plane.axis",
        ),
        (mutate("relax = 0.5", "relax = 0.0"), "mixing_plane.relax"),
        (mutate("relax = 0.5", "relax = 1.5"), "mixing_plane.relax"),
        (mutate("bands = 8\n", "bands = 8\nevery = 0\n"), "mixing_plane.every"),
        (mutate("tolerance = 1e-9", "tolerance = 0.0"), "mixing_plane.tolerance"),
        (
            mutate("algorithm = \"simplec\"", "algorithm = \"piso\""),
            "mixing_plane.enabled",
        ),
        (
            mutate("algorithm = \"simplec\"", "algorithm = \"coupled\""),
            "mixing_plane.enabled",
        ),
        (
            mutate("[solver]\n", "[solver]\nfrozen_flow = true\n"),
            "mixing_plane.enabled",
        ),
    ];
    // patch names are known to the mesh only: the error comes from building the simulation
    for (from, to, path) in [
        (
            "upstream_patch = \"plane_out\"",
            "upstream_patch = \"nope\"",
            "mixing_plane.upstream_patch",
        ),
        (
            "downstream_patch = \"plane_in\"",
            "downstream_patch = \"nope\"",
            "mixing_plane.downstream_patch",
        ),
    ] {
        let e = match Simulation::new(&parse(&mutate(from, to))) {
            Err(e) => e,
            Ok(_) => panic!("{path}: no error"),
        };
        assert!(has(&e, path), "expected an error at {path}, got {e:?}");
    }
    for (t, path) in cases {
        let e = errors(&t);
        assert!(has(&e, path), "expected an error at {path}, got {e:?}");
    }
}

/// MP9: a band whose flux cancels at run time is a failure with the prefix `mixing_plane:`.
#[test]
fn mp9_cancelling_band_is_a_failure() {
    // a bulk axial flux of zero through the inlet: W = 0 and no swirl at the inlet and walls
    let t = text(&Opt::default())
        .replace(
            "velocity_expression_z = \"1 [m/s]\"",
            "velocity_expression_z = \"0 [m/s]\"",
        )
        .replace("[initial]\nvelocity_expression_z = \"0 [m/s]\"\n", "");
    let mut sim = Simulation::new(&parse(&t)).unwrap_or_else(|e| panic!("{e:?}"));
    sim.run(5);
    let f = sim.failure().expect("a failure");
    println!("MP9 failure: {f}");
    assert!(f.starts_with("mixing_plane:"), "{f}");
}

/// MP4 pressure limit as registered (`max |p - p_exact - mean| <= 0.05 P`). KNOWN MISS (ADR 0059 addendum 2): 6.85e-2 P, found
/// in the innermost radial cell of BOTH blocks (block means differ by 4e-3 P), the first-order zero-gradient wall pressure of the
/// fixed-velocity hub wall against the exact centripetal gradient `Gamma^2/r^3`; the plane-free control below shows the same.
#[test]
#[ignore = "KNOWN MISS: pressure error 6.85e-2 P against 5e-2 P, hub-wall pressure treatment, not the plane (ADR 0059 addendum 2)"]
fn mp4_pressure_known_miss() {
    let sim = run(Opt::default());
    let (_, dp) = exact_error(&sim);
    println!(
        "MP4 pressure max |p - p_exact - mean| = {:.4e} P (limit 0.05)",
        dp / PSCALE
    );
    assert!(dp <= 0.05 * PSCALE, "MP4 pressure {:e}", dp / PSCALE);
}

/// Control for the MP4 pressure miss (informational, no criterion): the same exact flow in ONE connected annulus without a
/// plane (`annulus_3d`, 10 axial layers, the same MRF box).
#[test]
#[ignore = "diagnostic control, prints only"]
fn mp4_control_single_block() {
    let o = Opt::default();
    let mut t = text(&Opt { hub: 0.5, ..o });
    let a = t.find("[mixing_plane]").unwrap();
    t.truncate(a);
    let t = t
        .replace("annulus_3d_stage", "annulus_3d")
        .replace(
            "nz = 4
nx_right = 0
nz_right = 6
",
            "nz = 10
",
        )
        .replace(
            "[boundaries.plane_out]
velocity = \"zero_gradient\"
pressure = \"fixed\"
pressure_value = 0.0
[boundaries.plane_in]
velocity = \"fixed\"
velocity_value = [0.0, 0.0, 1.0]
",
            "",
        );
    let mut sim = Simulation::new(&parse(&t)).unwrap_or_else(|e| {
        panic!(
            "{e:?}
{t}"
        )
    });
    sim.run(3000);
    let (du, dp) = exact_error(&sim);
    println!(
        "CONTROL single block: converged {}, |u-ue| {:.3e} V, |p-pe-mean| {:.4e} P",
        sim.is_converged(),
        du / VREF,
        dp / PSCALE
    );
}
