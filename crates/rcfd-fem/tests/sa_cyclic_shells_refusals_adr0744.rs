//! F-672 (ADR 0744), F-628: the negative controls of ADR 0556 restated after the Beam2 refusal of the cyclic constraint was
//! lifted by default. `nc1_to_nc6_refusals` of `sa_cyclic_shells.rs` is retired as an ignored record (its NC3 asserts the
//! refusal); here NC1, NC2, NC4, NC5 and NC6 are its lines copied, and NC3 is the acceptance row A1 of ADR 0744. The helpers
//! are those of `sa_cyclic_shells.rs`, copied.

use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::cyclic::{modal_analysis_harmonic, solve_static_harmonic, HarmonicLoad};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::shell;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie::{TieMethod, TieOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, Material, SetKind, SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The point of the D3 frustum at column `i` (of `N NC` around) and generator parameter `s`, radial offset `dr`.
fn point(i: usize, s: f64, dr: f64) -> Vec3 {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    let r = 1.0 - 0.4 * s + dr;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

/// Nodes of a quad, cut into two triangles along the diagonal from its first node for Shell3.
fn push_element(conn: &mut Vec<usize>, kind: ElementKind, q: [usize; 4]) {
    match kind {
        ElementKind::Shell4 => conn.extend(q),
        _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
    }
}

/// A shell mesh of `columns` columns of nodes (`periodic`: the last column's elements close on column 0) and `NA + 1` rows,
/// node `(i, a)` at `a columns + i`; `shift(i, a)` moves a node. Element set `all`, node set `clamp` (a = 0).
fn shell_mesh(kind: ElementKind, columns: usize, periodic: bool, shift: &dyn Fn(usize, usize) -> Vec3) -> SolidMesh {
    let id = |i: usize, a: usize| a * columns + i % columns;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..columns {
            let p = point(i, a as f64 / NA as f64, 0.0);
            let d = shift(i, a);
            nodes.push([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }
    let quads = if periodic { columns } else { columns - 1 };
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..quads {
            push_element(
                &mut conn,
                kind,
                [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..columns).map(|i| id(i, 0)).collect())
        .unwrap();
    mesh
}

fn no_shift(_: usize, _: usize) -> Vec3 {
    [0.0; 3]
}

/// The sector node of column `i <= NC`, row `a`.
fn sector_node(i: usize, a: usize) -> usize {
    a * (NC + 1) + i
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
}

fn shell_options() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The sector model: the D3 sector with node sets `low` and `high`, the clamp and the cyclic constraint with `options`.
fn sector(kind: ElementKind, options: CyclicOptions, shift: &dyn Fn(usize, usize) -> Vec3) -> SolidModel {
    let mut mesh = shell_mesh(kind, NC + 1, false, shift);
    mesh.add_node_set("low", (0..=NA).map(|a| sector_node(0, a)).collect())
        .unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| sector_node(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// `nc1_to_nc6_refusals` of `sa_cyclic_shells.rs` (ADR 0556) restated by ADR 0744: NC3 is the acceptance row A1.
#[test]
fn nc1_nc2_nc4_nc5_nc6_restated_and_a1_the_beam_arc_is_taken() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, detail: String| {
        println!("{name}: {detail} {}", met(ok));
        if !ok {
            fails.push(format!("{name}: {detail}"));
        }
    };
    // NC1: the shell sector with the default options (the boundaries must be surface sets; a shell has none).
    let mut m = SolidModel::new(shell_mesh(ElementKind::Shell4, NC + 1, false, &no_shift));
    let r = m.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N));
    hold(
        "NC1 default options on a shell sector",
        matches!(
            &r,
            Err(FemError::UnknownSet {
                kind: SetKind::Surface,
                ..
            })
        ),
        format!("{r:?}"),
    );
    // NC2: the existing N2 set-up (Hex8 box, disjoint Shell4 element, default options) on the harmonic paths.
    let hb = hex_box([0.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hb.nodes().len();
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 1.0, 0.0], [3.0, 1.0, 0.0]]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    for name in BOX_FACES {
        mesh.add_surface_set(name, hb.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let mut m2 = SolidModel::new(mesh);
    m2.add_material("all", mat()).unwrap();
    m2.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let shell4 = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Shell4", package, .. }) if *package == shell::FOLLOW_UP);
    let r = modal_analysis_harmonic(&m2, &[], 1, &ModalOptions::new(2)).map(|_| ());
    hold(
        "NC2 default options, Shell4: modal_analysis_harmonic",
        shell4(&r),
        format!("{r:?}"),
    );
    let n2 = m2.dof_map().n_equations();
    let r = solve_static_harmonic(
        &m2,
        &[],
        &HarmonicLoad::new(1, vec![0.0; n2], vec![0.0; n2]),
        &StaticOptions::default(),
    )
    .map(|_| ());
    hold(
        "NC2 default options, Shell4: solve_static_harmonic",
        shell4(&r),
        format!("{r:?}"),
    );
    // A1 (NC3 restated, ADR 0744): a Beam2 arc with the shells option is taken. (a) As NC3 built it, without supports:
    // not the refusal naming the beam; (b) with node 1 clamped: both calls Ok.
    for clamped in [false, true] {
        let mut mesh = SolidMesh::new(vec![point(0, 0.5, 0.0), point(1, 0.5, 0.0), point(2, 0.5, 0.0)]).unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
        let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section, axis()).unwrap())
            .unwrap();
        mesh.add_element_set("all", vec![0, 1]).unwrap();
        mesh.add_node_set("low", vec![0]).unwrap();
        mesh.add_node_set("high", vec![2]).unwrap();
        mesh.add_node_set("mid", vec![1]).unwrap();
        let mut mb = SolidModel::new(mesh);
        mb.add_material("all", mat()).unwrap();
        if clamped {
            for comp in 0..6 {
                mb.add_constraint(Constraint::displacement("mid", comp, 0.0)).unwrap();
            }
        }
        mb.add_constraint(Constraint::cyclic_with(
            "low",
            "high",
            axis(),
            ORIGIN,
            N,
            shell_options(),
        ))
        .unwrap();
        let beam = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == rcfd_fem::beam::FOLLOW_UP);
        let rs = solve_static_mpc(&mb, &[], &[], &[], &StaticOptions::default()).map(|_| ());
        let rm = modal_analysis_harmonic(&mb, &[], 1, &modal_options(1)).map(|_| ());
        if clamped {
            hold(
                "A1 (b) clamped Beam2 arc: solve_static_mpc",
                rs.is_ok(),
                format!("{rs:?}"),
            );
            hold(
                "A1 (b) clamped Beam2 arc: modal_analysis_harmonic",
                rm.is_ok(),
                format!("{rm:?}"),
            );
        } else {
            hold(
                "A1 (a) Beam2 arc as NC3 built it: solve_static_mpc is not the beam refusal",
                !beam(&rs),
                format!("{rs:?}"),
            );
            hold(
                "A1 (a) Beam2 arc as NC3 built it: modal_analysis_harmonic is not the beam refusal",
                !beam(&rm),
                format!("{rm:?}"),
            );
        }
    }
    // NC4: a six-unknown shell node (low) paired with a three-unknown Hex8 node (high).
    let p = point(0, 0.5, 0.0);
    let qh = point(NC, 0.5, 0.0);
    let hb = hex_box(qh, [0.05, 0.05, 0.05], [1, 1, 1]).unwrap();
    let corner = (0..hb.nodes().len())
        .min_by(|&a, &b| dist(hb.nodes()[a], qh).total_cmp(&dist(hb.nodes()[b], qh)))
        .unwrap();
    let nh = hb.nodes().len();
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&[
        p,
        [p[0] - 0.05, p[1], p[2]],
        [p[0] - 0.05, p[1] - 0.05, p[2]],
        [p[0], p[1] - 0.05, p[2]],
    ]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("low", vec![nh]).unwrap();
    mesh.add_node_set("high", vec![corner]).unwrap();
    let mut m4 = SolidModel::new(mesh);
    m4.add_material("all", mat()).unwrap();
    m4.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        shell_options(),
    ))
    .unwrap();
    let r = solve_static_mpc(&m4, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. })
        if e.to_string().contains(&format!("low node {nh} has 6 unknowns")) && e.to_string().contains(&format!("{corner} has 3")));
    hold("NC4 a shell node paired with a solid node", ok, format!("{r:?}"));
    // NC5: the shells option with nodes on the axis and with interpolation.
    for (label, opts) in [
        ("axis nodes", shell_options().axis_nodes(true)),
        (
            "interpolation",
            shell_options().interpolation(TieOptions::new(TieMethod::NodeToSurface)),
        ),
    ] {
        let m5 = sector(ElementKind::Shell4, opts, &no_shift);
        let r = solve_static_mpc(&m5, &[], &[], &[], &StaticOptions::default()).map(|_| ());
        let ok =
            matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("ADR 0556"));
        hold(&format!("NC5 the shells option with {label}"), ok, format!("{r:?}"));
    }
    // NC6: the high column moved by 1e-6 L (L = 1 m, the largest radius) along the axis.
    let moved = |i: usize, _a: usize| -> Vec3 {
        if i == NC {
            axis().map(|c| 1e-6 * c)
        } else {
            [0.0; 3]
        }
    };
    let m6 = sector(ElementKind::Shell4, shell_options(), &moved);
    let r = solve_static_mpc(&m6, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("has no node of the high boundary"));
    hold("NC6 the high column moved by 1e-6 L", ok, format!("{r:?}"));
    assert!(fails.is_empty(), "missed: {fails:#?}");
}
