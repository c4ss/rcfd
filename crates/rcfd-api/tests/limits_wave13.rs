//! Solution limits (ADR 0016 check L1): a temperature band narrower than the natural range holds to 1e-14 with a
//! positive clipped count; the same case without limits leaves the band (positive control).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn cavity(limits: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
reference_temperature = 0.5
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.01
end_time = 0.2
{limits}
"#
    )
}

fn run(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(1000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    s
}

#[test]
fn l1_temperature_band_is_enforced() {
    let free = run(&cavity(""));
    let t = free.field("T").unwrap();
    let (lo, hi) = (
        t.iter().cloned().fold(f64::INFINITY, f64::min),
        t.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    println!("L1 free run: T in [{lo:.4}, {hi:.4}], clipped {}", free.clipped_cells());
    assert!(
        lo < 0.2 - 0.01 && hi > 0.8 + 0.01,
        "positive control: natural range must exceed the band"
    );
    assert_eq!(free.clipped_cells(), 0);

    let lim = run(&cavity("[limits.temperature]\nenabled = true\nmin = 0.2\nmax = 0.8\n"));
    let t = lim.field("T").unwrap();
    let (lo, hi) = (
        t.iter().cloned().fold(f64::INFINITY, f64::min),
        t.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    println!(
        "L1 limited run: T in [{lo:.6}, {hi:.6}], clipped {}",
        lim.clipped_cells()
    );
    assert!(lo >= 0.2 - 1e-14 && hi <= 0.8 + 1e-14);
    assert!(lim.clipped_cells() > 0);
    assert!(lim.status_json()["clipped_cells"].as_u64().unwrap() > 0);
}

#[test]
fn limits_are_validated() {
    let bad = cavity("[limits.temperature]\nenabled = true\nmin = 0.9\nmax = 0.1\n");
    let err = match Case::parse_toml(&bad) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    };
    assert!(err.iter().any(|d| d.path == "limits.temperature.min"), "{err:?}");
}
