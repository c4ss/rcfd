//! F-237 (ADR 0244) registered checks H6, H8, NC3, NC4: the `RCFDQ` shell heat-flux card of the load deck.
use rcfd_io::nastran::*;

fn quad_tria_deck() -> Deck {
    Deck {
        grids: vec![
            (1, [0.0, 0.0, 0.0]),
            (2, [1.0 / 3.0, 0.0, 0.0]),
            (3, [1.0, 1.0, 1e-300]),
            (4, [0.0, 1.0, -2.5e17]),
            (5, [2.0, 0.5, 0.1 + 0.2]),
        ],
        elements: vec![
            Element {
                kind: ElemKind::Quad4,
                id: 10,
                pid: 1,
                nodes: vec![1, 2, 3, 4],
            },
            Element {
                kind: ElemKind::Tria3,
                id: 11,
                pid: 2,
                nodes: vec![1, 2, 5],
            },
        ],
        pload4: vec![Pload4 {
            sid: 1,
            eid: 10,
            p: [3.7e5, f64::NAN, f64::NAN, f64::NAN],
        }],
        temps: vec![(7, 1, 300.0), (7, 2, 1.0 / 7.0), (7, 5, 1e-310)],
        forces: vec![],
        moments: vec![],
    }
}

fn heat() -> Vec<HeatFlux> {
    vec![
        HeatFlux {
            sid: 1,
            eid: 10,
            q: 1.0 / 3.0,
        },
        HeatFlux {
            sid: 1,
            eid: 11,
            q: 1e-300,
        },
        HeatFlux {
            sid: 2,
            eid: 10,
            q: -2.5e17,
        },
        HeatFlux {
            sid: 2,
            eid: 11,
            q: 0.1 + 0.2,
        },
        HeatFlux {
            sid: 3,
            eid: 10,
            q: 17.25,
        },
    ]
}

fn same_deck(a: &Deck, b: &Deck) -> bool {
    // PartialEq on f64 NaN fields of PLOAD4 fails, so compare by bits.
    a.grids == b.grids
        && a.elements == b.elements
        && a.temps == b.temps
        && a.forces == b.forces
        && a.moments == b.moments
        && a.pload4.len() == b.pload4.len()
        && a.pload4.iter().zip(&b.pload4).all(|(x, y)| {
            x.sid == y.sid && x.eid == y.eid && x.p.iter().zip(&y.p).all(|(u, v)| u.to_bits() == v.to_bits())
        })
}

#[test]
fn h6_heat_card_round_trip_is_bit_exact_and_opt_in() {
    let d = quad_tria_deck();
    let h = heat();
    let txt = write_with_heat_flux(&d, &h).unwrap();
    let (back, hb) = parse_with_heat_flux(&txt).unwrap();
    assert!(same_deck(&d, &back), "deck changed in the round trip");
    assert_eq!(h.len(), hb.len());
    for (a, b) in h.iter().zip(&hb) {
        assert_eq!((a.sid, a.eid, a.q.to_bits()), (b.sid, b.eid, b.q.to_bits()));
    }
    // with no heat-flux loads the text is byte-identical to the existing writer
    assert_eq!(write_with_heat_flux(&d, &[]).unwrap(), d.to_string_checked().unwrap());
    // a deck without RCFDQ parses the same with both readers
    let plain = d.to_string_checked().unwrap();
    let (b2, h2) = parse_with_heat_flux(&plain).unwrap();
    assert!(same_deck(&b2, &Deck::parse(&plain).unwrap()) && h2.is_empty());
    eprintln!(
        "F237 H6: {} RCFDQ cards bit-exact, empty-heat text byte-identical",
        hb.len()
    );
}

const HEAD: &str = "$ rcfd load deck (free field)\nBEGIN BULK\nGRID,1,,0.0,0.0,0.0\nGRID,2,,1.0,0.0,0.0\nGRID,3,,1.0,1.0,0.0\nGRID,4,,0.0,1.0,0.0\nCQUAD4,10,1,1,2,3,4\n";

fn parse_err(text: &str) -> String {
    match parse_with_heat_flux(text) {
        Ok(_) => panic!("expected an error for:\n{text}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn h8_located_errors() {
    let cases: Vec<(String, &str, &str)> = vec![
        (format!("{HEAD}RCFDQ,1,10\nENDDATA\n"), "line 8", "missing Q"),
        (format!("{HEAD}RCFDQ,1,10,abc\nENDDATA\n"), "line 8", "not a number"),
        (format!("{HEAD}RCFDQ,1,10,inf\nENDDATA\n"), "line 8", "not finite"),
        (
            format!("{HEAD}RCFDQ,1,99,1.0\nENDDATA\n"),
            "line 8",
            "undefined element 99",
        ),
        (
            format!("{HEAD}RCFDQ,1,10,1.0\nRCFDQ,1,10,2.0\nENDDATA\n"),
            "line 9",
            "given twice",
        ),
        (
            format!("{HEAD}RCFDQ,1,10,1.0\nENDDATA\nRCFDQ,2,10,1.0\n"),
            "line 10",
            "after ENDDATA",
        ),
        (
            format!("{HEAD}RCFDQ,1,10,1.0\nGRID,5,,x,0.0,0.0\nENDDATA\n"),
            "line 9",
            "GRID",
        ),
        (format!("{HEAD}RCFDQ,x,10,1.0\nENDDATA\n"), "line 8", "SID"),
    ];
    for (text, line, what) in &cases {
        let e = parse_err(text);
        assert!(
            e.contains(line) && e.contains(what),
            "expected '{line}' and '{what}' in: {e}"
        );
    }
    let hexa = "BEGIN BULK\nGRID,1,,0.0,0.0,0.0\nGRID,2,,1.0,0.0,0.0\nGRID,3,,1.0,1.0,0.0\nGRID,4,,0.0,1.0,0.0\nGRID,5,,0.0,0.0,1.0\nGRID,6,,1.0,0.0,1.0\nGRID,7,,1.0,1.0,1.0\nGRID,8,,0.0,1.0,1.0\nCHEXA,20,1,1,2,3,4,5,6\n,7,8\nRCFDQ,1,20,1.0\nENDDATA\n";
    let e = parse_err(hexa);
    assert!(e.contains("line 12") && e.contains("CHEXA"), "{e}");
    // writer
    let d = quad_tria_deck();
    for (h, what) in [
        (
            HeatFlux {
                sid: 1,
                eid: 10,
                q: f64::NAN,
            },
            "non-finite",
        ),
        (
            HeatFlux {
                sid: 1,
                eid: 12,
                q: 1.0,
            },
            "element 12",
        ),
        (
            HeatFlux {
                sid: 0,
                eid: 10,
                q: 1.0,
            },
            "not positive",
        ),
    ] {
        let e = write_with_heat_flux(&d, &[h]).unwrap_err().to_string();
        assert!(e.contains(what), "expected '{what}' in: {e}");
    }
    eprintln!(
        "F237 H8: {} located reader errors and 3 writer errors as registered",
        cases.len() + 1
    );
}

#[test]
fn nc3_old_reader_rejects_the_heat_card() {
    let txt = write_with_heat_flux(&quad_tria_deck(), &heat()).unwrap();
    let e = Deck::parse(&txt).unwrap_err().to_string();
    eprintln!("F237 NC3: Deck::parse -> {e}");
    assert!(e.contains("unknown card 'RCFDQ'"), "{e}");
}

#[test]
fn nc4_tampered_heat_value_is_detected() {
    let d = quad_tria_deck();
    let h = heat();
    let txt = write_with_heat_flux(&d, &h).unwrap();
    assert!(txt.contains("RCFDQ,3,10,17.25\n"));
    let bad = txt.replace("RCFDQ,3,10,17.25\n", "RCFDQ,3,10,17.35\n");
    let (_, hb) = parse_with_heat_flux(&bad).unwrap();
    let changed = h
        .iter()
        .zip(&hb)
        .filter(|(a, b)| a.q.to_bits() != b.q.to_bits())
        .count();
    let rel = (hb[4].q - h[4].q).abs() / h[4].q.abs();
    eprintln!("F237 NC4: {changed} card(s) differ, relative change of Q {rel:.3e}");
    assert!(changed == 1 && rel > 0.0);
}
