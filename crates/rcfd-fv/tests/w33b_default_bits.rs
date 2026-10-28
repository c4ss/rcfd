//! ADR 0069 acceptance: with every wave 33B option off (the defaults) the transient solver is bit-identical to the code before the wave.
//! The fingerprints were recorded from the unchanged code and written into the ADR (amendment 1) before any solver change.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, cavity_2d};
use std::f64::consts::PI;
use std::sync::Arc;

fn fingerprint(s: &PimpleSolver) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |x: f64| {
        h = (h.rotate_left(5) ^ x.to_bits()).wrapping_mul(0x9E3779B97F4A7C15);
    };
    for v in &s.u.values {
        mix(v[0]);
        mix(v[1]);
    }
    for &p in &s.p.values {
        mix(p);
    }
    for &f in &s.phi {
        mix(f);
    }
    h
}

fn tg(f: impl FnOnce(&mut PimpleSettings)) -> PimpleSolver {
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(32, 32, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut st = PimpleSettings::piso(0.05);
    st.convection = ConvScheme::Central;
    f(&mut st);
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.run_fixed(1.0, 0.02);
    s
}

fn cavity(f: impl FnOnce(&mut PimpleSettings)) -> PimpleSolver {
    let mesh = Arc::new(cavity_2d(16, 1.0).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(0.01);
    st.convection = ConvScheme::LinearUpwind;
    f(&mut st);
    let mut c = PimpleSolver::new(geom, u, p, st);
    c.run_fixed(1.0, 0.05);
    c
}

fn cases() -> Vec<(&'static str, PimpleSolver)> {
    vec![
        ("tg piso bdf2", tg(|s| s.time_scheme = TimeScheme::Bdf2)),
        (
            "tg pimple2 euler",
            tg(|s| {
                s.n_outer = 2;
                s.alpha_u = 0.9;
                s.alpha_p = 0.9;
            }),
        ),
        (
            "tg frac bdf2",
            tg(|s| {
                s.time_scheme = TimeScheme::Bdf2;
                s.fractional = Some(FractionalSettings::default());
            }),
        ),
        (
            "cavity piso bounded",
            cavity(|s| s.time_scheme = TimeScheme::Bdf2Bounded),
        ),
        (
            "cavity pimple2 bdf2",
            cavity(|s| {
                s.time_scheme = TimeScheme::Bdf2;
                s.n_outer = 2;
                s.alpha_u = 0.9;
                s.alpha_p = 0.9;
            }),
        ),
    ]
}

#[test]
fn defaults_are_bit_identical_to_the_code_before_wave_33b() {
    // Recorded on Windows. The Taylor-Green initial fields call the platform's libm (sin, cos), so their fingerprints differ
    // per platform: x86_64 Linux values proven equal at the amendment-1 code (c4a593d) and at HEAD (ADR 0137); other platforms
    // print and skip. The cavity fingerprints are the same on both.
    let windows: [(&str, &str); 5] = [
        ("tg piso bdf2", "e9dca234e513bec8"),
        ("tg pimple2 euler", "72d935a8d4993955"),
        ("tg frac bdf2", "e46fbe511183e9fc"),
        ("cavity piso bounded", "c21a6553acb2a7a6"),
        ("cavity pimple2 bdf2", "2c71144215f1d245"),
    ];
    let linux_x86_64: [(&str, &str); 5] = [
        ("tg piso bdf2", "793cc167a5c8dce3"),
        ("tg pimple2 euler", "7a9cd18ebf2fcbc2"),
        ("tg frac bdf2", "275aa31d35e717ff"),
        ("cavity piso bounded", "c21a6553acb2a7a6"),
        ("cavity pimple2 bdf2", "2c71144215f1d245"),
    ];
    let expected = if cfg!(windows) {
        Some(windows)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(linux_x86_64)
    } else {
        None
    };
    let mut bad = Vec::new();
    for (i, (name, s)) in cases().iter().enumerate() {
        let fp = format!("{:016x}", fingerprint(s));
        println!("FINGERPRINT {name} {fp}");
        let Some(expected) = expected else {
            continue;
        };
        let (ename, efp) = expected[i];
        assert_eq!(*name, ename);
        if fp != efp {
            bad.push(format!("{name}: {fp} != {efp}"));
        }
    }
    if expected.is_none() {
        eprintln!("no recorded fingerprints for this platform (printed above)");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
