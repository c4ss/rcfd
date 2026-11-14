//! F-416 (ADR 0476) B2: the bits of the rigid rotor of ADR 0120 (`rotordyn::critical_speeds`, `natural_frequencies` and
//! `response` of the rotor K of ADR 0476), recorded on main 57808e24 before any library line of F-416 was written. Uses only the
//! API of that commit.

use rcfd_post::rotordyn::*;

/// The FNV-1a hash of a sequence of 64-bit words.
fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The rotor K of ADR 0476.
fn rotor_k() -> Rotor {
    Rotor {
        mass: 1.0,
        it: 4e-3,
        ip: 2e-3,
        bearings: [
            Bearing {
                z: -0.04,
                k: 2e5,
                c: 20.0,
                eta: 0.0,
            },
            Bearing {
                z: 0.06,
                k: 3e5,
                c: 30.0,
                eta: 0.0,
            },
        ],
    }
}

fn unbalances() -> [PointUnbalance; 2] {
    [
        PointUnbalance {
            mass: 1e-3,
            radius: 0.01,
            angle: 0.0,
            z: -0.02,
        },
        PointUnbalance {
            mass: 1e-3,
            radius: 0.01,
            angle: 2.0,
            z: 0.03,
        },
    ]
}

fn digests() -> [u64; 3] {
    let r = rotor_k();
    let cs = critical_speeds(&r).unwrap();
    let nf = natural_frequencies(&r, cs[0]).unwrap();
    let mut resp = Vec::new();
    for f in [0.5, 1.0, 2.0] {
        let spin = f * cs[0];
        let l = unbalance_load(&unbalances(), spin).unwrap();
        let out = response(&r, spin, &[l]).unwrap();
        for d in out[0].disp {
            resp.push(d.re.to_bits());
            resp.push(d.im.to_bits());
        }
    }
    [
        fnv(cs.iter().map(|x| x.to_bits())),
        fnv(nf.iter().flat_map(|c| [c.re.to_bits(), c.im.to_bits()])),
        fnv(resp),
    ]
}

/// Prints the digests (run at main 57808e24 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B2 (ADR 0476); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 57808e24 (ADR 0476 B2).
const BASE: [u64; 3] = [0xd9be_4ed4_88d3_a3c5, 0x8537_fb28_619f_1ddd, 0xd591_9262_e967_4929];

#[test]
fn b2_rigid_rotor_unchanged() {
    let got = digests();
    let names = [
        "rotordyn critical_speeds",
        "rotordyn natural_frequencies",
        "rotordyn response",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "B2 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "B2 differs: {bad:?}");
}
