//! ADR 0077 part B: diagnostics of the `linear_upwind` divergence on the FAN-01 H-slit mesh (ignored diagnostic runs; their outcome is recorded
//! in the ADR). The registered stability criterion: 300 iterations; STABLE iff no steady-guard trip, the largest cell speed over iterations 10..300
//! is at most 3 times the largest boundary-face speed, and the continuity is finite with its maximum over 200..300 not above its value at 10.
//! D2-D5 need scratch-tree switches of the solver and are not part of the repository.

mod w35b_common;

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_fv::gradient::least_squares;

fn max_speed(v: &[[f64; 3]]) -> f64 {
    v.iter()
        .map(|u| vec3::norm(*u))
        .fold(0.0f64, |a, s| if s.is_nan() { f64::INFINITY } else { a.max(s) })
}

fn boundary_speed(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let ub = sim.velocity().boundary_values(g);
    (0..ub.len())
        .filter(|&i| !g.empty[g.n_internal + i])
        .map(|i| vec3::norm(ub[i]))
        .fold(0.0, f64::max)
}

/// The registered classification of one run of `n` iterations (300) started from `sim`; prints a summary line.
fn classify_sim(name: &str, mut sim: Simulation, n: usize) -> bool {
    let ub = boundary_speed(&sim);
    let (mut umax, mut c10, mut c_late, mut cont_ok) = (0.0f64, f64::NAN, 0.0f64, true);
    let mut why = String::new();
    for it in 1..=n {
        let s = sim.step();
        if let Some(d) = sim.steady_divergence() {
            why = format!("guard trip at {it}: {} {}", d.kind, d.quantity);
            break;
        }
        let u = max_speed(&sim.velocity().values);
        if it >= 10 {
            umax = umax.max(u);
        }
        if !s.continuity.is_finite() {
            cont_ok = false;
        }
        if it == 10 {
            c10 = s.continuity;
        }
        if it >= 200 {
            c_late = c_late.max(s.continuity);
        }
    }
    let stable = why.is_empty() && umax <= 3.0 * ub && cont_ok && c_late <= c10;
    println!(
        "{name}: {} (max speed 10..{n} {umax:.3e} vs 3 x {ub:.3e}; continuity at 10 {c10:.3e}, max 200..{n} {c_late:.3e}){}",
        if stable { "STABLE" } else { "UNSTABLE" },
        if why.is_empty() { String::new() } else { format!("; {why}") }
    );
    stable
}

fn classify(name: &str, text: &str, n: usize) -> bool {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    println!("{name}: {} cells", sim.geom.n_cells());
    classify_sim(name, sim, n)
}

/// Class of every cell: 0 interior, 1 seam (a face on seam_lo/seam_hi), 2 inlet, 3 outlet, 4 wall (blade, hub, shroud); the highest wins.
fn cell_class(sim: &Simulation) -> Vec<u8> {
    let mesh = &sim.geom.mesh;
    let mut cls = vec![0u8; mesh.n_cells()];
    for p in mesh.patches() {
        let c = match p.name.as_str() {
            "seam_lo" | "seam_hi" => 1,
            "inlet" => 2,
            "outlet" => 3,
            _ => 4,
        };
        for f in p.faces() {
            let o = mesh.owner()[f];
            cls[o] = cls[o].max(c);
        }
    }
    cls
}

const CLASS: [&str; 5] = ["interior", "seam", "inlet", "outlet", "wall"];

/// Largest least-squares gradient error / Omega of the initial field (Omega y, -Omega x, U_in) per cell class (interior, seam, inlet,
/// outlet, wall); `partner`: the F-H0 gradient (partner cells on the periodic faces).
fn d0_errors(partner: bool) -> [(f64, usize, usize); 5] {
    let case = Case::parse_toml(&w35b_common::laminar("linear_upwind")).unwrap();
    let sim = Simulation::new(&case).unwrap();
    let g = &sim.geom;
    let w = w35b_common::omega();
    let u = sim.velocity();
    let ub = u.boundary_values(g);
    let pts = if partner {
        rcfd_fv::gradient::periodic_partner_points(g, u, &u.values)
    } else {
        Vec::new()
    };
    let exact = [[0.0, w, 0.0], [-w, 0.0, 0.0], [0.0; 3]];
    let mut err = vec![0.0f64; g.n_cells()];
    for k in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let pk: Vec<(usize, [f64; 3], f64)> = pts.iter().map(|&(i, d, v)| (i, d, v[k])).collect();
        let gr = rcfd_fv::gradient::least_squares_partner(g, &cells, &bv, &pk);
        for c in 0..g.n_cells() {
            err[c] = err[c].max(vec3::norm(vec3::sub(gr[c], exact[k])) / w);
        }
    }
    let cls = cell_class(&sim);
    let cc = g.mesh.cell_centres();
    // (max error, worst cell, cell count) per class
    let mut worst = [(0.0f64, 0usize, 0usize); 5];
    for c in 0..g.n_cells() {
        let k = cls[c] as usize;
        worst[k].2 += 1;
        if err[c] > worst[k].0 {
            worst[k] = (err[c], c, worst[k].2);
        }
    }
    for (k, &(e, c, n)) in worst.iter().enumerate() {
        println!(
            "D0{} {}: {n} cells, max gradient error / Omega {e:.3e} at cell {c} ({:.4}, {:.4}, {:.4})",
            if partner { " partner_cell" } else { "" },
            CLASS[k],
            cc[c][0],
            cc[c][1],
            cc[c][2]
        );
    }
    worst
}

/// D0 (H0): the least-squares gradient of the initial field (Omega y, -Omega x, U_in) against its exact constant gradient.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D0)"]
fn d0_least_squares_gradient_of_the_initial_field() {
    let worst = d0_errors(false);
    let h0 = worst[0].0.max(worst[1].0) > 1e-6;
    println!("D0: H0 {}", if h0 { "STANDS" } else { "falsified" });
}

/// H0-a (amendment 2): with the F-H0 gradient the interior and seam cells are exact for the linear field.
#[test]
fn h0a_partner_cell_gradient_is_exact_for_the_linear_field() {
    let worst = d0_errors(true);
    assert!(
        worst[0].0 <= 1e-10 && worst[1].0 <= 1e-10,
        "H0-a: interior {:e}, seam {:e}",
        worst[0].0,
        worst[1].0
    );
}

/// H0-b (amendment 2): `face_value` (the default) is bit-identical to the key absent.
#[test]
fn h0b_face_value_is_the_default_bit_for_bit() {
    let run = |text: &str| {
        let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
        for _ in 0..20 {
            sim.step();
        }
        let mut b: Vec<u64> = Vec::new();
        for f in ["ux", "uy", "uz", "p"] {
            b.extend(sim.field(f).unwrap().into_iter().map(f64::to_bits));
        }
        b.extend(sim.flux().iter().map(|x| x.to_bits()));
        b
    };
    let base = w35b_common::laminar("upwind");
    let explicit = base.replace(
        "[solver]
",
        "[solver]
periodic_gradient = \"face_value\"
",
    );
    assert_ne!(base, explicit);
    assert!(run(&base) == run(&explicit), "H0-b: fields differ");
}

/// H0-c (amendment 2, reported): FAN-01 laminar `linear_upwind` with the F-H0 gradient.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (H0-c)"]
fn h0c_linear_upwind_with_partner_cell_gradient() {
    let t = w35b_common::laminar("linear_upwind").replace(
        "[solver]
",
        "[solver]
periodic_gradient = \"partner_cell\"
",
    );
    classify("H0-c linear_upwind, partner_cell", &t, 300);
}

/// D1 (localisation): the cell of the largest speed at iterations 1-3 of the `linear_upwind` run, its class and quality; the faces with the
/// largest |ho - ud| of the linear-upwind correction at iteration 1.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D1)"]
fn d1_localisation() {
    let case = Case::parse_toml(&w35b_common::laminar("linear_upwind")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let mesh = sim.geom.mesh.clone();
    let m = rcfd_mesh::quality::compute_metrics(&mesh);
    let cls = cell_class(&sim);
    let cc = mesh.cell_centres();
    let cell_metrics = |c: usize| {
        let (mut no, mut sk) = (0.0f64, 0.0f64);
        for &f in mesh.cell_faces(c) {
            no = no.max(m.non_orthogonality[f]);
            sk = sk.max(m.skewness[f]);
        }
        (no, sk, m.aspect_ratio[c])
    };
    // the largest |ho - ud| at iteration 1, from the initial field (the deferred correction of the first momentum solve)
    {
        let g = &sim.geom;
        let u = sim.velocity();
        let ub = u.boundary_values(g);
        let phi = sim.flux();
        let (fc, own, nei) = (mesh.face_centres(), mesh.owner(), mesh.neighbour());
        let mut faces: Vec<(f64, usize, usize)> = Vec::new();
        for k in 0..3 {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            let gr = least_squares(g, &cells, &bv);
            for f in 0..g.n_internal {
                let up = if phi[f] >= 0.0 { own[f] } else { nei[f] };
                let d = vec3::dot(gr[up], vec3::sub(fc[f], cc[up])).abs();
                faces.push((d, f, k));
            }
        }
        faces.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(d, f, k) in faces.iter().take(5) {
            let up = if phi[f] >= 0.0 { own[f] } else { nei[f] };
            let (no, sk, ar) = cell_metrics(up);
            println!(
                "D1 |ho - ud| {d:.3e} m/s (component {k}) at face {f}, upwind cell {up} ({}) at ({:.4}, {:.4}, {:.4}); face non-orth {:.1} deg, skew {:.2}; cell max non-orth {no:.1}, skew {sk:.2}, aspect {ar:.0}",
                CLASS[cls[up] as usize],
                cc[up][0],
                cc[up][1],
                cc[up][2],
                m.non_orthogonality[f],
                m.skewness[f]
            );
        }
    }
    for it in 1..=3 {
        sim.step();
        let v = &sim.velocity().values;
        let c = (0..v.len())
            .max_by(|&a, &b| vec3::norm(v[a]).total_cmp(&vec3::norm(v[b])))
            .unwrap();
        let (no, sk, ar) = cell_metrics(c);
        let r = (cc[c][0] * cc[c][0] + cc[c][1] * cc[c][1]).sqrt();
        println!(
            "D1 iteration {it}: max speed {:.3e} at cell {c} ({}), r {r:.4} z {:.4}; cell max non-orth {no:.1} deg, skew {sk:.2}, aspect {ar:.0}",
            vec3::norm(v[c]),
            CLASS[cls[c] as usize],
            cc[c][2]
        );
    }
    let all_max = m.non_orthogonality.iter().copied().fold(0.0, f64::max);
    println!("D1: mesh max non-orthogonality {all_max:.1} deg");
}

#[test]
#[ignore = "diagnostic of ADR 0077 part B (controls)"]
fn d_controls() {
    classify("control linear_upwind", &w35b_common::laminar("linear_upwind"), 300);
    classify("control upwind", &w35b_common::laminar("upwind"), 300);
}

/// D6 (H3): the FAN-01 mesh at rest.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D6)"]
fn d6_at_rest() {
    classify(
        "D6 at rest, linear_upwind",
        &w35b_common::laminar_at_rest("linear_upwind"),
        300,
    );
}

/// D7 (H4): one non-orthogonal pressure corrector with the SIMPLEC compensation.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D7)"]
fn d7_non_orthogonal_corrector() {
    let t = w35b_common::laminar("linear_upwind").replace(
        "[solver]\n",
        "[solver]\nnon_orthogonal_correctors = 1\nsimplec_non_orthogonal_compensation = true\n",
    );
    classify("D7 nc 1 + compensation, linear_upwind", &t, 300);
}

/// D8 (H5): 300 iterations upwind, then 300 linear_upwind from that state.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D8)"]
fn d8_switch_after_upwind() {
    let case = Case::parse_toml(&w35b_common::laminar("upwind")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    for _ in 0..300 {
        sim.step();
    }
    println!(
        "D8: after 300 upwind iterations, max speed {:.3e}",
        max_speed(&sim.velocity().values)
    );
    sim.simple_solver_mut().unwrap().settings.convection = rcfd_fv::convection::ConvScheme::LinearUpwind;
    classify_sim("D8 linear_upwind after 300 upwind", sim, 300);
}

/// D9 (information): the bounded second-order schemes.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (D9)"]
fn d9_tvd_schemes() {
    classify("D9 tvd_van_leer", &w35b_common::laminar("tvd_van_leer"), 300);
    classify("D9 tvd_minmod", &w35b_common::laminar("tvd_minmod"), 300);
}

/// Smallest reproducing case (registered order): (a) the shipped `axial_fan_sector` example, (b) FAN-01 with n_span 6.
#[test]
#[ignore = "diagnostic of ADR 0077 part B (smallest reproducing case)"]
fn smallest_reproducing_case() {
    let path = format!(
        "{}/../../cases/axial_fan_sector/axial_fan_sector.toml",
        env!("CARGO_MANIFEST_DIR")
    );
    let ex = std::fs::read_to_string(path).unwrap();
    let a_lu = classify(
        "(a) axial_fan_sector linear_upwind",
        &ex.replace("convection = \"upwind\"", "convection = \"linear_upwind\""),
        300,
    );
    let a_up = classify("(a) axial_fan_sector upwind", &ex, 300);
    let b = w35b_common::laminar_on(&w35b_common::mesh(6), "linear_upwind");
    // measured: the generator refuses n_span 6 (`mesh.fan.n_span`: the boundary layers and the tip gap leave no room), a set-up defect
    let b_ok = Case::parse_toml(&b)
        .map_err(|e| format!("{e:?}"))
        .and_then(|c| Simulation::new(&c).map(|_| ()).map_err(|e| format!("{e:?}")));
    let b_rep = match b_ok {
        Err(e) => {
            println!("(b) FAN-01 n_span 6: not runnable: {e}");
            false
        }
        Ok(()) => {
            let b_lu = classify("(b) FAN-01 n_span 6 linear_upwind", &b, 300);
            let b_up = classify(
                "(b) FAN-01 n_span 6 upwind",
                &b.replace("\"linear_upwind\"", "\"upwind\""),
                300,
            );
            !b_lu && b_up
        }
    };
    println!("smallest: (a) reproduces {}, (b) reproduces {b_rep}", !a_lu && a_up);
}
