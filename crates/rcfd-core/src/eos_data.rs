//! Critical constants and acentric factors for the cubic equations of state (ADR 0007).
//!
//! Published critical-point data of the common gases (NIST Chemistry WebBook fluid pages / Reid, Prausnitz and Poling).
//! The numbers were written from memory of those sources and are listed as unverified in `docs/reference-data.md`.

use crate::fluids::species;

/// Critical data of one fluid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CriticalData {
    /// Name as accepted by [`critical_data`].
    pub name: &'static str,
    /// Critical temperature (K).
    pub tc: f64,
    /// Critical pressure (Pa).
    pub pc: f64,
    /// Acentric factor.
    pub omega: f64,
    /// Molar mass (kg/mol) from the species table.
    pub molar_mass: f64,
}

/// Names for which [`critical_data`] has data.
pub const NAMES: [&str; 7] = ["N2", "O2", "AR", "CO2", "CH4", "H2", "H2O"];

/// Critical data of a built-in species (case-insensitive), `None` for an unknown name.
pub fn critical_data(name: &str) -> Option<CriticalData> {
    let upper = name.to_ascii_uppercase();
    let (tc, pc, omega) = match upper.as_str() {
        "N2" => (126.192, 3.3958e6, 0.0372),
        "O2" => (154.581, 5.0430e6, 0.0222),
        "AR" => (150.687, 4.8630e6, -0.002),
        "CO2" => (304.1282, 7.3773e6, 0.22394),
        "CH4" => (190.564, 4.5992e6, 0.01142),
        "H2" => (33.145, 1.2964e6, -0.219),
        "H2O" => (647.096, 22.064e6, 0.3443),
        _ => return None,
    };
    let sp = species(&upper)?;
    let name = NAMES.iter().find(|n| **n == upper).copied()?;
    Some(CriticalData {
        name,
        tc,
        pc,
        omega,
        molar_mass: sp.molar_mass,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete_and_case_insensitive() {
        for n in NAMES {
            let d = critical_data(n).unwrap_or_else(|| panic!("{n}"));
            assert!(d.tc > 0.0 && d.pc > 0.0 && d.molar_mass > 0.0);
        }
        assert_eq!(critical_data("co2").unwrap().name, "CO2");
        assert!(critical_data("unobtainium").is_none());
        assert!((critical_data("CO2").unwrap().molar_mass - 0.0440098).abs() < 1e-4);
    }
}
