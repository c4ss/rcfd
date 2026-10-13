//! Tables called from expressions (F-105).

use rcfd_core::expr::{Context, Expr};
use rcfd_core::table::{CsvSpec, Table};
use rcfd_core::units::Dimension;
use std::sync::Arc;

fn nu_table() -> Arc<Table> {
    let spec = CsvSpec {
        inputs: vec!["T".into()],
        ..Default::default()
    };
    Arc::new(
        Table::from_csv(
            "nu_t",
            "T [K], nu [m^2/s], rho [kg/m^3]\n280, 1.4e-6, 999.9\n300, 0.85e-6, 996.5\n320, 0.58e-6, 989.9\n",
            &spec,
        )
        .unwrap(),
    )
}

#[test]
fn table_calls_are_dimension_checked_and_interpolate() {
    let t = nu_table();
    let mut ctx = Context::new();
    ctx.add_table("nu_t", t.clone(), 0);
    ctx.add_table("nu_t_rho", t.clone(), 1);
    let temp = ctx.add_var("Tw", Dimension::TEMPERATURE);
    let c = Expr::parse("nu_t(Tw) * nu_t_rho(Tw)").unwrap().compile(&ctx).unwrap();
    assert_eq!(c.dimension(), Dimension::KINEMATIC_VISCOSITY * Dimension::DENSITY);
    let mut slots = [0.0];
    slots[temp] = 310.0;
    // mu(310) from linear interpolation of both columns
    let want = (0.5 * (0.85e-6 + 0.58e-6)) * (0.5 * (996.5 + 989.9));
    assert!(
        (c.eval(&slots) - want).abs() < 1e-15 * want.abs().max(1.0),
        "{}",
        c.eval(&slots)
    );
    slots[temp] = 400.0;
    assert!(c.eval(&slots).is_nan(), "out of range must not extrapolate silently");
    // constant arguments fold into a constant expression
    let k = Expr::parse("nu_t(300 [K])").unwrap().compile(&ctx).unwrap();
    assert!(k.is_constant());
    assert_eq!(k.constant_value(), Some(0.85e-6));
}

#[test]
fn bad_calls_are_rejected_with_a_span() {
    let t = nu_table();
    let mut ctx = Context::new();
    ctx.add_table("nu_t", t, 0);
    ctx.add_var("L", Dimension::LENGTH);
    let e = Expr::parse("nu_t(L)").unwrap().compile(&ctx).unwrap_err();
    assert!(e.message.contains("must have dimension"), "{e}");
    let e = Expr::parse("nu_t(1 [K], 2 [K])").unwrap().compile(&ctx).unwrap_err();
    assert!(e.message.contains("takes 1 argument"), "{e}");
    let e = Expr::parse("nu_t(300)").unwrap().compile(&ctx).unwrap_err();
    assert!(e.message.contains("must have dimension"), "{e}");
    // a table input without a unit cannot be used in a dimension-checked expression
    let spec = CsvSpec {
        inputs: vec!["s".into()],
        ..Default::default()
    };
    let bare = Arc::new(Table::from_csv("bare", "s,v\n0,1\n1,2\n", &spec).unwrap());
    let mut ctx = Context::new();
    ctx.add_table("bare", bare, 0);
    let e = Expr::parse("bare(0.5)").unwrap().compile(&ctx).unwrap_err();
    assert!(e.message.contains("has no unit"), "{e}");
}
