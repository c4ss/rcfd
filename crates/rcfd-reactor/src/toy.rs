//! Hand-written toy mechanisms with constant-heat-capacity thermodynamics, for tests and examples (ADR 0027). Nothing here
//! is data of a published mechanism.

use crate::mech::Mechanism;

/// A fixed-column CHEMKIN thermodynamic record for `name` with atoms `atoms` and the two coefficient sets.
pub fn thermo_record(name: &str, atoms: &[(&str, f64)], low: [f64; 7], high: [f64; 7]) -> String {
    let mut el = String::new();
    for i in 0..4 {
        if let Some((n, c)) = atoms.get(i) {
            el.push_str(&format!("{:<2}{:>3}", n, *c as i64));
        } else {
            el.push_str("     ");
        }
    }
    let h = format!(
        "{:<18}{:<6}{}G{:>10.3}{:>10.3}{:>8.2}",
        name, "TOY", el, 200.0, 6000.0, 1000.0
    );
    let h = format!("{h:<79}1");
    let f = |v: f64| format!("{v:>15.8e}");
    let l2 = format!("{}{}{}{}{}", f(high[0]), f(high[1]), f(high[2]), f(high[3]), f(high[4]));
    let l2 = format!("{l2:<79}2");
    let l3 = format!("{}{}{}{}{}", f(high[5]), f(high[6]), f(low[0]), f(low[1]), f(low[2]));
    let l3 = format!("{l3:<79}3");
    let l4 = format!("{}{}{}{}", f(low[3]), f(low[4]), f(low[5]), f(low[6]));
    let l4 = format!("{l4:<79}4");
    format!("{h}\n{l2}\n{l3}\n{l4}\n")
}

/// Constant-`cp` coefficients: `cp/R = a1`, `h/RT = a1 + h0/T`, `s/R = a1 ln T + s0`.
pub fn cc(a1: f64, h0: f64, s0: f64) -> [f64; 7] {
    [a1, 0.0, 0.0, 0.0, 0.0, h0, s0]
}

/// A CHEMKIN thermodynamic file text from `(name, atoms, a1, h0, s0)` entries.
pub fn thermo_text(entries: &[(&str, Vec<(&str, f64)>, f64, f64, f64)]) -> String {
    let mut s = String::from("THERMO ALL\n   200.000  1000.000  6000.000\n");
    for (name, atoms, a1, h0, s0) in entries {
        s.push_str(&thermo_record(name, atoms, cc(*a1, *h0, *s0), cc(*a1, *h0, *s0)));
    }
    s.push_str("END\n");
    s
}

/// Mechanism and thermo text of a one-step isomerisation `A => B` with rate `A_pre exp(-ea/(R T))` (`ea` in cal/mol) and
/// `h0_b` the enthalpy offset (K) of B (A has 0): `h_A - h_B = R (0 - h0_b)` per mole. Species molar mass 20 g/mol.
pub fn isomer_texts(a_pre: f64, ea_cal: f64, h0_b: f64, with_inert: bool) -> (String, String) {
    let sp = if with_inert { "A B I" } else { "A B" };
    let mech = format!("ELEMENTS X/20.0/ END\nSPECIES {sp} END\nREACTIONS\nA => B  {a_pre:E} 0.0 {ea_cal}\nEND\n");
    let mut e = vec![
        ("A", vec![("X", 1.0)], 2.5, 0.0, 4.0),
        ("B", vec![("X", 1.0)], 2.5, h0_b, 4.0),
    ];
    if with_inert {
        e.push(("I", vec![("X", 1.0)], 2.5, 0.0, 4.0));
    }
    (mech, thermo_text(&e))
}

/// The isomerisation mechanism built from [`isomer_texts`].
pub fn isomer(a_pre: f64, ea_cal: f64, h0_b: f64, with_inert: bool) -> Mechanism {
    let (m, t) = isomer_texts(a_pre, ea_cal, h0_b, with_inert);
    Mechanism::from_chemkin(&m, Some(&t)).expect("toy isomer mechanism")
}

/// M3 of ADR 0027: elements X (20 g/mol), Y (30), Z (14); `X2 + Y2 <=> 2 XY`, inert I = Z2.
pub fn exchange_texts() -> (String, String) {
    let mech = "ELEMENTS X/20.0/ Y/30.0/ Z/14.0/ END\nSPECIES X2 Y2 XY I END\nREACTIONS\nX2 + Y2 <=> 2XY  1.0E9 0.0 0.0\nEND\n".to_string();
    let e = vec![
        ("X2", vec![("X", 2.0)], 2.5, 0.0, 4.0),
        ("Y2", vec![("Y", 2.0)], 2.5, 0.0, 4.0),
        ("XY", vec![("X", 1.0), ("Y", 1.0)], 2.5, -2000.0, 4.0),
        ("I", vec![("Z", 2.0)], 2.5, 0.0, 4.0),
    ];
    (mech, thermo_text(&e))
}

/// M4 of ADR 0027 (eddy dissipation): FU = X2, OX = Y2, PR = XY3, inert DI; `FU + 3 OX => 2 PR`.
pub fn burner_texts() -> (String, String) {
    let mech = "ELEMENTS X/12.0/ Y/16.0/ Z/14.0/ END\nSPECIES FU OX PR DI END\nREACTIONS\nFU + 3OX => 2PR  1.0E3 0.0 0.0\nEND\n".to_string();
    let e = vec![
        ("FU", vec![("X", 2.0)], 2.5, 0.0, 4.0),
        ("OX", vec![("Y", 2.0)], 2.5, 0.0, 4.0),
        ("PR", vec![("X", 1.0), ("Y", 3.0)], 2.5, -5000.0, 4.0),
        ("DI", vec![("Z", 2.0)], 2.5, 0.0, 4.0),
    ];
    (mech, thermo_text(&e))
}
