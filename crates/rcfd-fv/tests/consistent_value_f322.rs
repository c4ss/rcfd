//! F-322 (ADR 0258), the library part: the consistent-corrector flags as one settings value (`ConsistentCorrectors`, V1), one
//! validation with located refusals (`ConsistentCorrectors::conflicts`, V2) and the refusals through the solver (R3, R4), and the
//! flags without `enabled` (N4). The set-ups are ADR 0214's 2-D box of R1 (copied; the registered tests are not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::diffusion::GradientOptions;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{
    ConsistentContext, ConsistentCorrectors, ConsistentRefusal, Divergence, OuterHook, OuterUpdate, PimpleSettings,
    PimpleSolver,
};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn ubits(v: &[Vec3]) -> Vec<u64> {
    v.iter().flat_map(|x| x.iter().map(|y| y.to_bits())).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// ADR 0214's 2-D box solver for the refusals (periodic in x for the periodic ones).
fn box_solver(periodic: bool, settings: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d(8, 6, 1.0, 0.5, periodic, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let walls: &[&str] = if periodic {
        &["ymin", "ymax"]
    } else {
        &["xmin", "xmax", "ymin", "ymax"]
    };
    for name in walls {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

fn with_value(c: ConsistentCorrectors) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.01);
    s.consistent = c;
    s
}

fn only_enabled() -> ConsistentCorrectors {
    ConsistentCorrectors {
        enabled: true,
        ..Default::default()
    }
}

/// ADR 0258 V1: the value, its default, the older fields read together with it, and the setter.
#[test]
fn v1_one_value_carries_the_flags() {
    for s in [PimpleSettings::piso(0.01), PimpleSettings::pimple(0.01, 3)] {
        assert_eq!(s.consistent, ConsistentCorrectors::default());
        let c = s.consistent;
        assert!(
            !c.enabled && !c.periodic && !c.body_force && !c.non_orthogonal && !c.component_diagonal && !c.face_jump
        );
        assert!(!s.consistent_correctors && !s.consistent_periodic && !s.consistent_body_force);
        assert_eq!(s.consistent_flags(), ConsistentCorrectors::default());
    }
    // the older fields alone
    for (a, b, c) in [
        (true, false, false),
        (true, true, false),
        (true, false, true),
        (true, true, true),
        (false, true, true),
    ] {
        let mut s = PimpleSettings::piso(0.01);
        s.consistent_correctors = a;
        s.consistent_periodic = b;
        s.consistent_body_force = c;
        assert_eq!(
            s.consistent_flags(),
            ConsistentCorrectors {
                enabled: a,
                periodic: b,
                body_force: c,
                ..Default::default()
            }
        );
        assert_eq!(
            s.consistent,
            ConsistentCorrectors::default(),
            "the older fields do not write the value"
        );
    }
    // the value alone
    let c = ConsistentCorrectors {
        enabled: true,
        periodic: false,
        body_force: true,
        non_orthogonal: true,
        component_diagonal: false,
        face_jump: true,
    };
    assert_eq!(with_value(c).consistent_flags(), c);
    // the setter: the value and the older fields
    for c in [
        c,
        ConsistentCorrectors::all(),
        ConsistentCorrectors::default(),
        only_enabled(),
    ] {
        let mut s = PimpleSettings::piso(0.01);
        s.consistent_correctors = true;
        s.consistent_periodic = true;
        s.consistent_body_force = true;
        s.set_consistent(c);
        assert_eq!(s.consistent_flags(), c);
        assert_eq!(s.consistent, c);
        assert_eq!(
            (s.consistent_correctors, s.consistent_periodic, s.consistent_body_force),
            (c.enabled, c.periodic, c.body_force)
        );
    }
    let a = ConsistentCorrectors::all();
    assert!(a.enabled && a.periodic && a.body_force && a.non_orthogonal && a.component_diagonal && a.face_jump);
}

/// The 13 rows of ADR 0258 V2: the refusal, the flags and the context that produce it first, its stage, flag and setting.
#[allow(clippy::type_complexity)]
fn rows() -> Vec<(
    ConsistentRefusal,
    ConsistentCorrectors,
    ConsistentContext,
    &'static str,
    &'static str,
    &'static str,
)> {
    use ConsistentRefusal as R;
    let all = ConsistentCorrectors::all();
    let on = only_enabled();
    let ctx = ConsistentContext::default();
    let non_orth = ConsistentContext {
        non_orth_correctors: 1,
        ..ctx
    };
    let diag = ConsistentContext {
        component_diagonal: true,
        ..ctx
    };
    let jump = ConsistentContext { face_jump: true, ..ctx };
    let per = "periodic pressure faces";
    let mov = "the moving-mesh step";
    vec![
        (
            R::FractionalStep,
            all,
            ConsistentContext {
                fractional: true,
                ..ctx
            },
            "fractional step",
            "enabled",
            "PimpleSettings::fractional",
        ),
        (
            R::NonOrthogonal,
            on,
            non_orth,
            "non-orthogonal correctors",
            "non_orthogonal",
            "PimpleSettings::non_orth_correctors",
        ),
        (
            R::NonOrthogonalGradientOptions,
            all,
            ConsistentContext {
                gradient_pressure: true,
                ..non_orth
            },
            "non-orthogonal correctors with pressure gradient options",
            "non_orthogonal",
            "PimpleSettings::gradient_pressure",
        ),
        (
            R::NonOrthogonalPeriodic,
            all,
            ConsistentContext {
                periodic_faces: true,
                ..non_orth
            },
            "non-orthogonal correctors with periodic faces",
            "non_orthogonal",
            per,
        ),
        (
            R::NonOrthogonalMovingMesh,
            all,
            ConsistentContext {
                moving_mesh: true,
                ..non_orth
            },
            "non-orthogonal correctors on a moving mesh",
            "non_orthogonal",
            mov,
        ),
        (
            R::ComponentDiagonal,
            on,
            diag,
            "component diagonal",
            "component_diagonal",
            "PimpleSettings::component_diagonal",
        ),
        (
            R::ComponentDiagonalPeriodic,
            all,
            ConsistentContext {
                periodic_faces: true,
                ..diag
            },
            "component diagonal with periodic faces",
            "component_diagonal",
            per,
        ),
        (
            R::ComponentDiagonalMovingMesh,
            all,
            ConsistentContext {
                moving_mesh: true,
                ..diag
            },
            "component diagonal on a moving mesh",
            "component_diagonal",
            mov,
        ),
        (
            R::BodyForce,
            on,
            ConsistentContext {
                body_force: true,
                ..ctx
            },
            "body force",
            "body_force",
            "PimpleSolver::body_force",
        ),
        (
            R::FaceJump,
            on,
            jump,
            "face jump",
            "face_jump",
            "PimpleSolver::face_jump",
        ),
        (
            R::FaceJumpPeriodic,
            all,
            ConsistentContext {
                periodic_faces: true,
                ..jump
            },
            "face jump with periodic faces",
            "face_jump",
            per,
        ),
        (
            R::FaceJumpMovingMesh,
            all,
            ConsistentContext {
                moving_mesh: true,
                ..jump
            },
            "face jump on a moving mesh",
            "face_jump",
            mov,
        ),
        (
            R::PeriodicFaces,
            on,
            ConsistentContext {
                periodic_faces: true,
                ..ctx
            },
            "periodic faces",
            "periodic",
            per,
        ),
    ]
}

/// ADR 0258 V2: `conflicts` and `validate` as pure functions.
#[test]
fn v2_the_validation_names_flag_setting_and_reason() {
    let rows = rows();
    assert_eq!(rows.len(), 13);
    assert_eq!(
        rows.iter().map(|r| r.0).collect::<Vec<_>>(),
        ConsistentRefusal::ALL.to_vec(),
        "the order of the table"
    );
    for (refusal, flags, ctx, stage, flag, setting) in &rows {
        let found = flags.conflicts(ctx);
        assert_eq!(found.first(), Some(refusal), "{refusal:?}: {found:?}");
        assert_eq!(flags.validate(ctx), Err(*refusal));
        assert_eq!(refusal.stage(), *stage);
        assert_eq!(refusal.flag(), *flag);
        assert!(
            refusal.setting().contains(setting),
            "{refusal:?}: {}",
            refusal.setting()
        );
        assert!(!refusal.reason().is_empty());
        let m = refusal.message();
        assert!(
            m.contains(&format!("ConsistentCorrectors::{flag}")) && m.contains(setting) && m.contains("ADR 0258"),
            "{m}"
        );
        assert!(refusal.to_string().contains(stage) && refusal.to_string().contains("ADR 0214"));
        assert_eq!(ConsistentRefusal::from_stage(stage), Some(*refusal));
        // without `enabled` nothing is refused
        let off = ConsistentCorrectors {
            enabled: false,
            ..*flags
        };
        assert!(off.conflicts(ctx).is_empty() && off.validate(ctx).is_ok());
    }
    assert_eq!(ConsistentRefusal::from_stage("momentum predictor"), None);
    // every flag, a fixed mesh, no periodic faces, no gradient options, no fractional step: every other setting is accepted
    let busy = ConsistentContext {
        fractional: false,
        non_orth_correctors: 3,
        gradient_pressure: false,
        component_diagonal: true,
        body_force: true,
        face_jump: true,
        periodic_faces: false,
        moving_mesh: false,
    };
    assert_eq!(ConsistentCorrectors::all().validate(&busy), Ok(()));
    // the flags of ADR 0223 keep what they accepted: periodic faces and body forces, also on a moving mesh
    let f301 = ConsistentContext {
        body_force: true,
        periodic_faces: true,
        moving_mesh: true,
        ..Default::default()
    };
    let c = ConsistentCorrectors {
        enabled: true,
        periodic: true,
        body_force: true,
        ..Default::default()
    };
    assert_eq!(c.validate(&f301), Ok(()));
    // several conflicts come in the table's order
    use ConsistentRefusal as R;
    let everything = ConsistentContext {
        fractional: true,
        non_orth_correctors: 1,
        gradient_pressure: true,
        component_diagonal: true,
        body_force: true,
        face_jump: true,
        periodic_faces: true,
        moving_mesh: true,
    };
    assert_eq!(
        only_enabled().conflicts(&everything),
        vec![
            R::FractionalStep,
            R::NonOrthogonal,
            R::ComponentDiagonal,
            R::BodyForce,
            R::FaceJump,
            R::PeriodicFaces
        ]
    );
    assert_eq!(
        ConsistentCorrectors::all().conflicts(&everything),
        vec![
            R::FractionalStep,
            R::NonOrthogonalGradientOptions,
            R::NonOrthogonalPeriodic,
            R::NonOrthogonalMovingMesh,
            R::ComponentDiagonalPeriodic,
            R::ComponentDiagonalMovingMesh,
            R::FaceJumpPeriodic,
            R::FaceJumpMovingMesh
        ]
    );
}

/// How a step of R3 and R4 is taken.
#[derive(Clone, Copy, PartialEq)]
enum Via {
    /// `step`.
    Step,
    /// `step_with` and a hook that supplies a body force.
    HookForce,
    /// `step_with` and a hook that supplies a face jump.
    HookJump,
    /// `step_ale` with the same geometry and zero mesh flux.
    Ale,
}

/// One step of `flow`; the divergence record if the run was stopped, with the state checked unchanged.
fn one_step(label: &str, mut flow: PimpleSolver, via: Via) -> Option<Divergence> {
    let before = (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi));
    let geom = flow.geom.clone();
    match via {
        Via::Step => {
            flow.step(0.01);
        }
        Via::HookForce => {
            let g = geom.clone();
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                source_force: Some(BodyForce::uniform(&g, [0.0, -9.81, 0.0])),
                ..Default::default()
            };
            flow.step_with(0.01, Some(&mut hook as &mut OuterHook));
        }
        Via::HookJump => {
            let n = geom.mesh.n_faces();
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                face_jump: Some(FaceJump::zeros(n)),
                ..Default::default()
            };
            flow.step_with(0.01, Some(&mut hook as &mut OuterHook));
        }
        Via::Ale => {
            let n = geom.mesh.n_faces();
            flow.step_ale(0.01, geom.clone(), vec![0.0; n]).expect("step_ale");
        }
    }
    let d = flow.diverged.clone()?;
    assert_eq!(
        (ubits(&flow.u.values), bits(&flow.p.values), bits(&flow.phi)),
        before,
        "{label}: the state changed"
    );
    Some(d)
}

fn body(mut f: PimpleSolver) -> PimpleSolver {
    f.body_force = Some(BodyForce::uniform(&f.geom.clone(), [0.0, -9.81, 0.0]));
    f
}

fn jump(mut f: PimpleSolver) -> PimpleSolver {
    f.face_jump = Some(FaceJump::zeros(f.geom.mesh.n_faces()));
    f
}

/// ADR 0258 R3: every row of the table through the solver, with its located message.
#[test]
fn r3_every_refusal_through_the_solver_is_located() {
    use ConsistentRefusal as R;
    let all = ConsistentCorrectors::all();
    let on = only_enabled();
    let set = |c: ConsistentCorrectors, f: &dyn Fn(&mut PimpleSettings)| {
        let mut s = with_value(c);
        f(&mut s);
        s
    };
    let non_orth = |s: &mut PimpleSettings| s.non_orth_correctors = 1;
    let diag = |s: &mut PimpleSettings| s.component_diagonal = true;
    let none = |_: &mut PimpleSettings| {};
    let cases: Vec<(R, PimpleSolver, Via)> = vec![
        (
            R::FractionalStep,
            box_solver(false, set(all, &|s| s.fractional = Some(FractionalSettings::default()))),
            Via::Step,
        ),
        (R::NonOrthogonal, box_solver(false, set(on, &non_orth)), Via::Step),
        (
            R::NonOrthogonalGradientOptions,
            box_solver(
                false,
                set(all, &|s| {
                    s.non_orth_correctors = 1;
                    s.gradient_pressure = Some(GradientOptions::default());
                }),
            ),
            Via::Step,
        ),
        (
            R::NonOrthogonalPeriodic,
            box_solver(true, set(all, &non_orth)),
            Via::Step,
        ),
        (
            R::NonOrthogonalMovingMesh,
            box_solver(false, set(all, &non_orth)),
            Via::Ale,
        ),
        (R::ComponentDiagonal, box_solver(false, set(on, &diag)), Via::Step),
        (
            R::ComponentDiagonalPeriodic,
            box_solver(true, set(all, &diag)),
            Via::Step,
        ),
        (
            R::ComponentDiagonalMovingMesh,
            box_solver(false, set(all, &diag)),
            Via::Ale,
        ),
        (R::BodyForce, body(box_solver(false, set(on, &none))), Via::Step),
        (R::BodyForce, box_solver(false, set(on, &none)), Via::HookForce),
        (R::FaceJump, jump(box_solver(false, set(on, &none))), Via::Step),
        (R::FaceJump, box_solver(false, set(on, &none)), Via::HookJump),
        (R::FaceJumpPeriodic, jump(box_solver(true, set(all, &none))), Via::Step),
        (R::FaceJumpPeriodic, box_solver(true, set(all, &none)), Via::HookJump),
        (
            R::FaceJumpMovingMesh,
            jump(box_solver(false, set(all, &none))),
            Via::Ale,
        ),
        (R::PeriodicFaces, box_solver(true, set(on, &none)), Via::Step),
    ];
    let mut seen = Vec::new();
    for (refusal, flow, via) in cases {
        let label = format!("R3 {refusal:?}");
        let d = one_step(&label, flow, via).unwrap_or_else(|| panic!("{label}: not refused"));
        let msg = d.message(None);
        println!("{label}: {msg}");
        assert_eq!(d.step, 1, "{label} step");
        assert_eq!(d.quantity, "consistent correctors", "{label} quantity");
        assert_eq!(d.stage, refusal.stage(), "{label} stage");
        for part in [
            "ADR 0214".to_string(),
            format!("are not defined with the {}:", refusal.stage()),
            format!("ConsistentCorrectors::{}", refusal.flag()),
            refusal.setting().to_string(),
            refusal.reason().to_string(),
            "ADR 0258".to_string(),
        ] {
            assert!(msg.contains(&part), "{label}: '{part}' missing in '{msg}'");
        }
        seen.push(refusal);
    }
    for r in ConsistentRefusal::ALL {
        assert!(seen.contains(&r), "R3: {r:?} was not produced");
    }
}

/// ADR 0258 R4: what the flags accept.
#[test]
fn r4_the_flags_accept_their_combinations() {
    let flag = |f: &dyn Fn(&mut ConsistentCorrectors)| {
        let mut c = only_enabled();
        f(&mut c);
        c
    };
    let mut s = with_value(flag(&|c| c.non_orthogonal = true));
    s.non_orth_correctors = 1;
    assert!(one_step("R4 non-orthogonal", box_solver(false, s), Via::Step).is_none());
    let mut s = with_value(flag(&|c| c.component_diagonal = true));
    s.component_diagonal = true;
    assert!(one_step("R4 component diagonal", box_solver(false, s), Via::Step).is_none());
    let s = with_value(flag(&|c| c.face_jump = true));
    assert!(one_step("R4 face jump", jump(box_solver(false, s.clone())), Via::Step).is_none());
    assert!(one_step("R4 face jump (hook)", box_solver(false, s), Via::HookJump).is_none());
    // the flags of ADR 0223 through the value alone (the older fields stay false)
    let s = with_value(flag(&|c| c.periodic = true));
    assert!(!s.consistent_correctors && !s.consistent_periodic);
    assert!(one_step("R4 periodic", box_solver(true, s), Via::Step).is_none());
    let s = with_value(flag(&|c| c.body_force = true));
    assert!(!s.consistent_correctors && !s.consistent_body_force);
    assert!(one_step("R4 body force", body(box_solver(false, s.clone())), Via::Step).is_none());
    assert!(one_step("R4 body force (hook)", box_solver(false, s), Via::HookForce).is_none());
    // every flag, every accepted setting at once
    let mut s = with_value(ConsistentCorrectors {
        periodic: false,
        ..ConsistentCorrectors::all()
    });
    s.non_orth_correctors = 1;
    s.component_diagonal = true;
    assert!(one_step("R4 all", jump(body(box_solver(false, s))), Via::Step).is_none());
}

/// ADR 0258 N4: the flags without `enabled` are inert.
#[test]
fn n4_the_flags_without_enabled_are_inert() {
    let run = |s: PimpleSettings| {
        let mut f = body(box_solver(true, s));
        for _ in 0..3 {
            f.step(0.01);
            assert!(f.diverged.is_none(), "N4 stopped");
        }
        (ubits(&f.u.values), bits(&f.p.values), bits(&f.phi))
    };
    let inert = ConsistentCorrectors {
        enabled: false,
        ..ConsistentCorrectors::all()
    };
    assert_eq!(
        run(with_value(inert)),
        run(PimpleSettings::piso(0.01)),
        "N4: the flags without `enabled` changed the run"
    );
}
