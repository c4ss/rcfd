//! Wave 27C (ADR 0051) registered checks R1 and R2.
use rcfd_io::nastran::*;
use rcfd_io::registry::{self, Status};

#[test]
fn r1_every_supported_format_has_a_passing_conformance_case() {
    let all = registry::all();
    let mut names: Vec<&str> = all.iter().map(|i| i.name).collect();
    let supported: Vec<_> = all.iter().filter(|i| i.status == Status::Supported).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), all.len(), "duplicate names");
    assert!(supported.len() >= 10);
    for (name, r) in registry::run_all() {
        eprintln!(
            "W27C R1 {name}: {}",
            if r.is_ok() {
                "pass".to_string()
            } else {
                format!("{r:?}")
            }
        );
        assert!(r.is_ok(), "{name}: {r:?}");
    }
    for n in ["cgns", "hdf5"] {
        let i = all.iter().find(|i| i.name == n).unwrap();
        match i.status {
            Status::NotSupported(why) => assert!(!why.is_empty()),
            Status::Supported => panic!("{n} must be listed as not supported"),
        }
        assert!(registry::conformance_for(n).unwrap_err().contains(n));
    }
    assert!(registry::conformance_for("nope").unwrap_err().contains("nope"));
}

#[test]
fn r1_tampered_data_fails_with_the_format_name() {
    for n in ["vtu", "stl", "tecplot", "ensight", "nastran_deck"] {
        let e = registry::conformance_with(n, true).expect_err(n);
        eprintln!("W27C R1 tampered {n}: {e}");
        assert!(e.contains(n), "{e}");
    }
}

fn sample_deck() -> Deck {
    Deck {
        grids: vec![
            (1, [0.0, 0.0, 0.0]),
            (2, [1.0 / 3.0, 0.0, 0.0]),
            (3, [1.0, 1.0, 1e-300]),
            (4, [0.0, 1.0, -2.5e17]),
            (5, [0.0, 0.0, 1.0]),
            (6, [1.0, 0.0, 1.0]),
            (7, [1.0, 1.0, 1.0]),
            (8, [0.0, 1.0, 1.0]),
        ],
        elements: vec![
            Element {
                kind: ElemKind::Quad4,
                id: 1,
                pid: 1,
                nodes: vec![1, 2, 3, 4],
            },
            Element {
                kind: ElemKind::Tria3,
                id: 2,
                pid: 1,
                nodes: vec![1, 2, 3],
            },
            Element {
                kind: ElemKind::Hexa8,
                id: 3,
                pid: 2,
                nodes: vec![1, 2, 3, 4, 5, 6, 7, 8],
            },
        ],
        pload4: vec![
            Pload4 {
                sid: 5,
                eid: 1,
                p: [1.0e5 / 3.0, f64::NAN, f64::NAN, f64::NAN],
            },
            Pload4 {
                sid: 5,
                eid: 2,
                p: [1.5, 2.5, -3.5, 4.5],
            },
        ],
        temps: (1..=8).map(|g| (9, g, 300.0 + g as f64 / 7.0)).collect(),
        ..Default::default()
    }
}

#[test]
fn r2_deck_round_trip_is_bit_exact() {
    let d = sample_deck();
    let txt = d.to_string_checked().unwrap();
    let back = Deck::parse(&txt).unwrap();
    assert_eq!(back.grids, d.grids);
    assert_eq!(back.elements, d.elements);
    assert_eq!(back.temps, d.temps);
    for (a, b) in back.pload4.iter().zip(&d.pload4) {
        assert_eq!((a.sid, a.eid), (b.sid, b.eid));
        for k in 0..4 {
            assert!(a.p[k].to_bits() == b.p[k].to_bits() || (a.p[k].is_nan() && b.p[k].is_nan()));
        }
    }
    // idempotent
    assert_eq!(back.to_string_checked().unwrap(), txt);
}

#[test]
fn r2_deck_located_errors() {
    let d = sample_deck();
    let txt = d.to_string_checked().unwrap();
    let line_of = |needle: &str| txt.lines().position(|l| l.starts_with(needle)).unwrap() + 1;
    let cases: Vec<(String, String)> = vec![
        (
            txt.replacen("GRID,2,", "GRDX,2,", 1),
            format!("line {}", line_of("GRID,2,")),
        ),
        (
            txt.replacen("GRID,3,,1.0", "GRID,3,,abc", 1),
            format!("line {}", line_of("GRID,3,")),
        ),
        (
            txt.replacen("GRID,2,", "GRID,1,", 1),
            format!("line {}", line_of("GRID,2,")),
        ),
        (
            txt.replacen("CQUAD4,1,1,1,2,3,4", "CQUAD4,1,1,1,2,3", 1),
            format!("line {}", line_of("CQUAD4")),
        ),
        (
            txt.replacen("CTRIA3,2,1,1,2,3", "CTRIA3,2,1,1,2,99", 1),
            format!("line {}", line_of("CTRIA3")),
        ),
        (
            txt.replacen("PLOAD4,5,2,", "PLOAD4,5,77,", 1),
            "undefined element 77".to_string(),
        ),
        (txt.replacen("ENDDATA\n", "", 1), "ENDDATA".to_string()),
    ];
    for (bad, want) in cases {
        let e = Deck::parse(&bad).unwrap_err().to_string();
        assert!(e.contains(&want), "'{e}' should contain '{want}'");
    }
    // writer-side checks
    let mut d2 = d.clone();
    d2.grids.push((1, [0.0; 3]));
    assert!(d2.to_string_checked().unwrap_err().to_string().contains("grid id 1"));
    let mut d3 = d.clone();
    d3.pload4[0].p[0] = f64::NAN;
    assert!(d3
        .to_string_checked()
        .unwrap_err()
        .to_string()
        .contains("non-finite pressure"));
    let mut d4 = d.clone();
    d4.elements[0].nodes.pop();
    assert!(d4.to_string_checked().unwrap_err().to_string().contains("element 1"));
}
