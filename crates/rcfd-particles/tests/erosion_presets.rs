//! ADR 0197 (F-254 follow-up): the named steel preset of the erosion constants, in the library. Registered checks G1 (the
//! preset values equal Table 7-2 as quoted in `docs/reference-data.md`, exactly) and G2 (the run of ADR 0194 B with the
//! preset equals the run with the same constants typed in, bit for bit). The limits are those of ADR 0197, fixed before
//! this code ran.

use rcfd_core::vec3::{scale, Vec3};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::path::PathBuf;
use std::sync::Arc;

const NAME: &str = "dnv_o501_2007_steel";

/// The F-254 table of `docs/reference-data.md`, whitespace collapsed.
fn reference_data() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference-data.md");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// G1: one preset, the steel grades of Table 7-2, equal by `==` to the literals and to the numbers of the quoted row.
#[test]
fn g1_preset_values_equal_table_7_2() {
    assert_eq!(EROSION_PRESETS.len(), 1, "G1: one preset");
    let p = &EROSION_PRESETS[0];
    println!("G1 preset {p:?}");
    assert_eq!(p.name, NAME);
    assert_eq!(p.row, "steel grades");
    assert_eq!(p.source, "DNV RP O501 Rev 4.2-2007 Table 7-2");
    assert!(p.k == 2.0e-9 && p.n == 2.6 && p.density == 7800.0, "G1 literals");
    // the row as docs/reference-data.md quotes it
    let doc = reference_data();
    let quote = "steel grades 2.0e-9 (m/s)^-n, 2.6, 7800 kg/m^3";
    assert!(
        doc.contains(quote),
        "G1: the Table 7-2 row is not quoted in docs/reference-data.md"
    );
    let nums: Vec<f64> = quote
        .replace("steel grades ", "")
        .replace(" (m/s)^-n", "")
        .replace(" kg/m^3", "")
        .split(", ")
        .map(|s| s.parse::<f64>().unwrap())
        .collect();
    println!("G1 numbers of the quoted row {nums:?}");
    assert_eq!(nums.len(), 3);
    assert!(
        p.k == nums[0] && p.n == nums[1] && p.density == nums[2],
        "G1: preset {p:?} against the quoted row {nums:?}"
    );
    assert_eq!(erosion_preset(NAME), Some(p));
    for other in ["steel", "none", "", "DNV_O501_2007_STEEL"] {
        assert!(erosion_preset(other).is_none(), "G1: '{other}' is not a preset");
    }
    let s = ErosionSpec::from_preset(vec!["ymin".into()], p);
    assert_eq!(s.patches, vec!["ymin".to_string()]);
    assert_eq!(
        [s.k.to_bits(), s.n.to_bits(), s.density.to_bits()],
        [p.k.to_bits(), p.n.to_bits(), p.density.to_bits()],
        "G1 from_preset"
    );
    assert!(PRESET_PROVENANCE.contains("has not been checked against the maintainer's edition"));
    assert!(PRESET_PROVENANCE.contains("indicative, not certified"));
}

/// Geometry P of ADR 0194: the plate `ymin` of `box_mesh([6, 4, 2], [0.06, 0.04, 0.02])`.
fn plate() -> Arc<Mesh> {
    Arc::new(box_mesh([6, 4, 2], [0.06, 0.04, 0.02], [Grading::Uniform; 3]).unwrap())
}

/// The tracker of ADR 0194 B: ballistic parcels, the plate reflecting with `e = 1`, every other patch `Escape`.
fn plate_tracker() -> Tracker {
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut overrides: Vec<(String, WallAction)> = ["xmin", "xmax", "ymax", "zmin", "zmax"]
        .iter()
        .map(|p| (p.to_string(), WallAction::Escape))
        .collect();
    overrides.push(("ymin".into(), WallAction::Reflect(1.0)));
    Tracker::new(plate(), opts, &overrides).unwrap()
}

/// The streams of ADR 0194 B: (angle [deg], target, rate [1/s]); `U = 20` m/s, `d = 2e-4` m, density 2650.
const B_STREAMS: [(f64, Vec3, f64); 3] = [
    (15.0, [0.045, 0.0, 0.005], 1000.0),
    (35.0, [0.035, 0.0, 0.015], 2000.0),
    (90.0, [0.015, 0.0, 0.005], 500.0),
];

/// The run of ADR 0194 B with the erosion spec `spec` (600 steps of 1 ms, carrier at rest).
fn b_run(spec: ErosionSpec) -> Tracker {
    let mut tr = plate_tracker();
    tr.set_erosion(spec).unwrap();
    for (i, (a, x, r)) in B_STREAMS.iter().enumerate() {
        let a = a.to_radians();
        let dir = [a.cos(), -a.sin(), 0.0];
        tr.add_injector(InjectorSpec {
            name: format!("s{}", i + 1),
            kind: InjKind::Point([x[0] - 0.03 * dir[0], x[1] - 0.03 * dir[1], x[2]]),
            rate: *r,
            t_start: 0.0,
            t_end: 0.5,
            velocity: VelMode::Given(scale(dir, 20.0)),
            dist: SizeDist::Constant(2e-4),
            density: 2650.0,
            parcel_count: 1.0,
            seed: 7,
        })
        .unwrap();
    }
    let u = vec![[0.0; 3]; tr.locator().mesh().n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 1.2,
        mu: 1.8e-5,
    };
    for _ in 0..600 {
        tr.advance(&flow, 1e-3).unwrap();
    }
    tr
}

/// G2: the run of ADR 0194 B with the preset against the same run with the constants typed in.
#[test]
fn g2_preset_rate_equals_the_explicit_rate_bit_for_bit() {
    let preset = erosion_preset(NAME).unwrap();
    let a = b_run(ErosionSpec::from_preset(vec!["ymin".into()], preset));
    let b = b_run(ErosionSpec {
        patches: vec!["ymin".into()],
        k: 2.0e-9,
        n: 2.6,
        density: 7800.0,
    });
    let (ea, eb) = (a.erosion().unwrap(), b.erosion().unwrap());
    let (ta, tb) = (a.time, b.time);
    let (fa, fb) = (a.fingerprint(), b.fingerprint());
    let mut rate_diff = 0;
    for k in 0..ea.len() {
        if ea.rate(k, ta).to_bits() != eb.rate(k, tb).to_bits()
            || ea.depth_rate(k, ta).to_bits() != eb.depth_rate(k, tb).to_bits()
        {
            rate_diff += 1;
        }
    }
    println!(
        "G2: fingerprint words {} and {}, equal {}; {} faces, {} with a differing rate or depth rate; hits {}, largest rate {:e}; csv equal {}",
        fa.len(),
        fb.len(),
        fa == fb,
        ea.len(),
        rate_diff,
        ea.total_hits(),
        ea.max_rate(ta),
        ea.csv(ta) == eb.csv(tb)
    );
    assert!(ea.total_hits() > 0, "G2 set-up");
    assert!(fa == fb, "G2 fingerprints");
    assert_eq!(ea.len(), eb.len());
    assert_eq!(rate_diff, 0, "G2 rates");
    assert_eq!(ea.csv(ta), eb.csv(tb), "G2 csv");
}
