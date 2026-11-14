//! F-518 (ADR 0429): the entry refusal of beams on the transformation, observed before the transformation is built.
//! Checks E1 to E8 of the registration.

use rcfd_fem::beam;
use rcfd_fem::builders::hex_box;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Material, SolidMesh, SolidModel,
};

struct Checks(Vec<String>);

impl Checks {
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

/// The model of S2 of ADR 0426. Returns the model, the shared node (or the beam's first node when not shared) and the
/// free node of the beam. `constraints`: 0 = set A (`u = 0` and `u = 1e-3`), 1 = set B (a cancelling row).
fn model(shared: bool, set_b: bool) -> (SolidModel, usize, usize) {
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut nodes = hex.nodes().to_vec();
    let on_block = nodes.iter().position(|y| *y == [2.0, 0.0, 0.0]).unwrap();
    nodes.push([3.0, 0.0, 0.0]);
    let free = nodes.len() - 1;
    let other = if shared {
        on_block
    } else {
        nodes.push([4.0, 0.0, 0.0]);
        nodes.len() - 1
    };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![free, other]).unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_element_set("structural", vec![2]).unwrap();
    mesh.add_node_set("xmin", hex.node_set("xmin").unwrap().to_vec())
        .unwrap();
    mesh.add_node_set("xmax", hex.node_set("xmax").unwrap().to_vec())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    m.add_material("all", steel.clone()).unwrap();
    m.add_material("structural", steel).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::rigid_set("xmax", [2.0, 0.5, 0.5]))
        .unwrap();
    if set_b {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(free, 0, 1.0), LinearTerm::new(free, 0, -1.0)],
            0.0,
        ))
        .unwrap();
    } else {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(free, 0, 1.0)], 0.0))
            .unwrap();
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(free, 0, 1.0)], 1.0e-3))
            .unwrap();
    }
    (m, on_block, free)
}

fn is_shared_node(r: &Result<(), FemError>, node: usize) -> bool {
    matches!(r, Err(e @ FemError::BeamSolidNode { node: n, .. })
        if *n == node && e.to_string().contains(&format!("node {node}")))
}

fn is_constraint_error(r: &Result<(), FemError>, text: &str) -> bool {
    matches!(r, Err(FemError::InvalidValue { key: "constraint", reason }) if reason.contains(text))
}

fn stat(m: &SolidModel) -> Result<(), FemError> {
    solve_static_mpc(m, &[], &[], &[], &StaticOptions::default()).map(|_| ())
}

fn modal(m: &SolidModel) -> Result<(), FemError> {
    modal_analysis_mpc(m, &[], &ModalOptions::new(2)).map(|_| ())
}

#[test]
fn e1_to_e8_entry_refusal_before_the_transformation() {
    let mut c = Checks(Vec::new());
    let (ma, shared, _) = model(true, false);
    let (mb, shared_b, _) = model(true, true);
    assert_eq!(shared, shared_b);
    let r1 = stat(&ma);
    c.holds(
        "E1 solve_static_mpc, shared, set A",
        is_shared_node(&r1, shared),
        &format!("{r1:?}"),
    );
    let r2 = modal(&ma);
    c.holds(
        "E2 modal_analysis_mpc, shared, set A",
        is_shared_node(&r2, shared),
        &format!("{r2:?}"),
    );
    let r3 = modal(&mb);
    c.holds(
        "E3 modal_analysis_mpc, shared, set B",
        is_shared_node(&r3, shared),
        &format!("{r3:?}"),
    );
    let (ca, _, _) = model(false, false);
    let (cb, _, _) = model(false, true);
    let r4 = stat(&ca);
    c.holds(
        "E4 solve_static_mpc, control, set A",
        is_constraint_error(&r4, "contradicts"),
        &format!("{r4:?}"),
    );
    let r5 = modal(&ca);
    c.holds(
        "E5 modal_analysis_mpc, control, set A",
        is_constraint_error(&r5, "takes zero values only"),
        &format!("{r5:?}"),
    );
    let r6 = modal(&cb);
    c.holds(
        "E6 modal_analysis_mpc, control, set B",
        is_constraint_error(&r6, "cancel to zero"),
        &format!("{r6:?}"),
    );
    let r7 = stat(&mb);
    c.holds(
        "E7 solve_static_mpc, shared, set B",
        is_shared_node(&r7, shared),
        &format!("{r7:?}"),
    );
    let direct = beam::check_beam_model(&ma).map(|_| ());
    let fields = |r: &Result<(), FemError>| match r {
        Err(FemError::BeamSolidNode { node, beam, solid }) => Some((*node, *beam, *solid)),
        _ => None,
    };
    c.holds(
        "E8 error fields equal those of check_beam_model",
        fields(&r1).is_some() && fields(&r1) == fields(&direct),
        &format!("{:?} vs {:?}", fields(&r1), fields(&direct)),
    );
    c.finish();
}
