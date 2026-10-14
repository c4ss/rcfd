//! Gas-phase mechanism: NASA-7 thermodynamics, reaction kinetics and the reader of the published CHEMKIN-II text format.
//!
//! Internal units are SI: concentrations mol/m3, energies J/mol, pressures Pa, molar masses kg/mol, rate constants
//! in (m3/mol)^(order-1)/s. Standard state pressure is 1 atm (101325 Pa) as in the NASA/CHEMKIN convention.

use std::collections::HashMap;
use std::fmt;

/// Universal gas constant, J/(mol K) (CODATA 2018).
pub const R_GAS: f64 = 8.314_462_618_153_24;
/// Standard state pressure, Pa.
pub const P_STD: f64 = 101_325.0;

/// Error from reading or checking a mechanism; `line` is 1-based in the file the text came from (0 = not tied to a line).
#[derive(Debug, Clone, PartialEq)]
pub struct MechError {
    /// Line number or 0.
    pub line: usize,
    /// Message.
    pub msg: String,
}

impl fmt::Display for MechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.msg)
        } else {
            f.write_str(&self.msg)
        }
    }
}

impl std::error::Error for MechError {}

fn merr<T>(line: usize, msg: impl Into<String>) -> Result<T, MechError> {
    Err(MechError { line, msg: msg.into() })
}

/// Species with NASA-7 thermodynamic data.
#[derive(Debug, Clone, PartialEq)]
pub struct Species {
    /// Name.
    pub name: String,
    /// Atoms of each mechanism element (same order as `Mechanism::elements`).
    pub atoms: Vec<f64>,
    /// Molar mass, kg/mol.
    pub w: f64,
    /// Lower temperature limit.
    pub t_low: f64,
    /// Switch temperature between the two polynomials.
    pub t_mid: f64,
    /// Upper temperature limit.
    pub t_high: f64,
    /// Coefficients for T < t_mid.
    pub low: [f64; 7],
    /// Coefficients for T >= t_mid.
    pub high: [f64; 7],
}

impl Species {
    fn coef(&self, t: f64) -> &[f64; 7] {
        if t < self.t_mid {
            &self.low
        } else {
            &self.high
        }
    }

    /// cp / R.
    pub fn cp_r(&self, t: f64) -> f64 {
        let a = self.coef(t);
        a[0] + t * (a[1] + t * (a[2] + t * (a[3] + t * a[4])))
    }

    /// h / (R T).
    pub fn h_rt(&self, t: f64) -> f64 {
        let a = self.coef(t);
        a[0] + t * (a[1] / 2.0 + t * (a[2] / 3.0 + t * (a[3] / 4.0 + t * a[4] / 5.0))) + a[5] / t
    }

    /// s / R at the standard pressure.
    pub fn s_r(&self, t: f64) -> f64 {
        let a = self.coef(t);
        a[0] * t.ln() + t * (a[1] + t * (a[2] / 2.0 + t * (a[3] / 3.0 + t * a[4] / 4.0))) + a[6]
    }
}

/// Reaction pressure dependence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Plain Arrhenius.
    Elementary,
    /// Rate multiplied by the third-body concentration.
    ThreeBody,
    /// Pressure-dependent falloff (Lindemann or Troe).
    Falloff,
}

/// One reaction. Rate parameters are SI: `[A, b, Ta]` with `k = A T^b exp(-Ta/T)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    /// Equation text as written.
    pub equation: String,
    /// Line in the source file.
    pub line: usize,
    /// Reactant species indices and stoichiometric coefficients.
    pub reactants: Vec<(usize, f64)>,
    /// Product species indices and coefficients.
    pub products: Vec<(usize, f64)>,
    /// Reversible by equilibrium constant (or by explicit REV parameters).
    pub reversible: bool,
    /// Pressure dependence.
    pub kind: Kind,
    /// Forward (high-pressure limit for falloff) Arrhenius parameters.
    pub arr: [f64; 3],
    /// Low-pressure limit parameters (falloff).
    pub low: Option<[f64; 3]>,
    /// Troe parameters alpha, T3, T1, T2 (T2 = infinity when absent).
    pub troe: Option<[f64; 4]>,
    /// Explicit reverse Arrhenius parameters.
    pub rev: Option<[f64; 3]>,
    /// Third-body efficiencies (all species, default 1) for `ThreeBody` and `Falloff`.
    pub eff: Vec<f64>,
    /// Specific collision partner of a `(+X)` falloff, if any.
    pub collider: Option<usize>,
    /// Sum of product coefficients minus sum of reactant coefficients.
    pub dnu: f64,
}

/// A mechanism: elements, species, reactions.
#[derive(Debug, Clone, PartialEq)]
pub struct Mechanism {
    /// Element names.
    pub elements: Vec<String>,
    /// Species.
    pub species: Vec<Species>,
    /// Reactions.
    pub reactions: Vec<Reaction>,
}

fn atomic_weight(el: &str) -> Option<f64> {
    // g/mol (IUPAC standard values used by CHEMKIN-style files)
    Some(match el {
        "H" => 1.00794,
        "D" => 2.014,
        "HE" => 4.002602,
        "C" => 12.0107,
        "N" => 14.0067,
        "O" => 15.9994,
        "F" => 18.998403,
        "NE" => 20.1797,
        "S" => 32.065,
        "CL" => 35.453,
        "AR" => 39.948,
        "BR" => 79.904,
        _ => return None,
    })
}

#[derive(Default)]
struct Parts {
    elements: Vec<(String, Option<f64>)>,
    species: Vec<String>,
    thermo: HashMap<String, (Species, Vec<(String, f64)>)>,
    thermo_default: Option<[f64; 3]>,
    reaction_lines: Vec<(usize, String)>,
    units: String,
}

fn parse_f(s: &str, line: usize, what: &str) -> Result<f64, MechError> {
    let t = s.trim().replace(['D', 'd'], "E");
    t.parse::<f64>()
        .or_else(|_| merr(line, format!("cannot read {what} from '{}'", s.trim())))
}

fn first_word(s: &str) -> String {
    s.split_whitespace().next().unwrap_or("").to_ascii_uppercase()
}

impl Mechanism {
    /// Read a mechanism from CHEMKIN text; `thermo_text` is the separate thermodynamic file if the mechanism has none inline.
    pub fn from_chemkin(mech_text: &str, thermo_text: Option<&str>) -> Result<Mechanism, MechError> {
        let mut parts = Parts {
            units: "CAL/MOLE".into(),
            ..Default::default()
        };
        parse_sections(mech_text, &mut parts)?;
        if let Some(t) = thermo_text {
            let mut p2 = Parts::default();
            parse_sections(t, &mut p2)?;
            if p2.thermo.is_empty() {
                return merr(0, "the thermodynamic file contains no species records");
            }
            parts.thermo.extend(p2.thermo);
        }
        build(parts)
    }

    /// Index of a species by name (case-insensitive).
    pub fn species_index(&self, name: &str) -> Option<usize> {
        self.species.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Check that every reaction balances every element; returns the largest absolute imbalance in atoms.
    pub fn max_element_imbalance(&self) -> f64 {
        let mut worst = 0.0_f64;
        for r in &self.reactions {
            for e in 0..self.elements.len() {
                let lhs: f64 = r.reactants.iter().map(|&(k, nu)| nu * self.species[k].atoms[e]).sum();
                let rhs: f64 = r.products.iter().map(|&(k, nu)| nu * self.species[k].atoms[e]).sum();
                worst = worst.max((lhs - rhs).abs());
            }
        }
        worst
    }

    /// Mole fractions from mass fractions.
    pub fn y_to_x(&self, y: &[f64]) -> Vec<f64> {
        let n: Vec<f64> = y.iter().zip(&self.species).map(|(y, s)| y / s.w).collect();
        let tot: f64 = n.iter().sum();
        n.iter().map(|v| v / tot).collect()
    }

    /// Mass fractions from mole fractions (normalised).
    pub fn x_to_y(&self, x: &[f64]) -> Vec<f64> {
        let m: Vec<f64> = x.iter().zip(&self.species).map(|(x, s)| x * s.w).collect();
        let tot: f64 = m.iter().sum();
        m.iter().map(|v| v / tot).collect()
    }

    /// Mean molar mass, kg/mol, from mass fractions.
    pub fn mean_w(&self, y: &[f64]) -> f64 {
        1.0 / y.iter().zip(&self.species).map(|(y, s)| y / s.w).sum::<f64>()
    }

    /// Mass of each element per unit mass of mixture, kg/kg (uses atomic weights through species composition).
    pub fn element_mass_fractions(&self, y: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.elements.len()];
        for (k, s) in self.species.iter().enumerate() {
            for e in 0..out.len() {
                out[e] += y[k] / s.w * s.atoms[e];
            }
        }
        out // moles of element atoms per kg of mixture; proportional to element mass
    }

    /// Mixture specific enthalpy, J/kg.
    pub fn h_mass(&self, t: f64, y: &[f64]) -> f64 {
        self.species
            .iter()
            .zip(y)
            .map(|(s, y)| y * R_GAS * t * s.h_rt(t) / s.w)
            .sum()
    }

    /// Mixture specific internal energy, J/kg.
    pub fn u_mass(&self, t: f64, y: &[f64]) -> f64 {
        self.h_mass(t, y) - R_GAS * t / self.mean_w(y)
    }

    /// Mixture specific heat at constant pressure, J/(kg K).
    pub fn cp_mass(&self, t: f64, y: &[f64]) -> f64 {
        self.species
            .iter()
            .zip(y)
            .map(|(s, y)| y * R_GAS * s.cp_r(t) / s.w)
            .sum()
    }

    /// Mixture specific heat at constant volume, J/(kg K).
    pub fn cv_mass(&self, t: f64, y: &[f64]) -> f64 {
        self.cp_mass(t, y) - R_GAS / self.mean_w(y)
    }

    /// Net molar production rates (mol/m3/s) for temperature `t` and concentrations `c` (mol/m3).
    pub fn production_rates(&self, t: f64, c: &[f64], wdot: &mut [f64]) {
        let ns = self.species.len();
        let g_rt: Vec<f64> = self.species.iter().map(|s| s.h_rt(t) - s.s_r(t)).collect();
        for w in wdot.iter_mut() {
            *w = 0.0;
        }
        let ln_t = t.ln();
        let c_ref = P_STD / (R_GAS * t);
        let _ = ns;
        for r in &self.reactions {
            let arrh = |p: &[f64; 3]| p[0] * (p[1] * ln_t - p[2] / t).exp();
            let m = || -> f64 {
                match r.collider {
                    Some(k) => c[k],
                    None => r.eff.iter().zip(c).map(|(e, c)| e * c).sum(),
                }
            };
            let kf = match r.kind {
                Kind::Elementary => arrh(&r.arr),
                Kind::ThreeBody => arrh(&r.arr) * m(),
                Kind::Falloff => {
                    let kinf = arrh(&r.arr);
                    let k0 = arrh(r.low.as_ref().expect("falloff has LOW"));
                    let pr = k0 * m() / kinf;
                    let f = match r.troe {
                        None => 1.0,
                        Some([alpha, t3, t1, t2]) => {
                            let mut fc = (1.0 - alpha) * (-t / t3).exp() + alpha * (-t / t1).exp();
                            if t2.is_finite() {
                                fc += (-t2 / t).exp();
                            }
                            let lfc = fc.max(1e-300).log10();
                            let cc = -0.4 - 0.67 * lfc;
                            let nn = 0.75 - 1.27 * lfc;
                            let lp = pr.max(1e-300).log10() + cc;
                            10f64.powf(lfc / (1.0 + (lp / (nn - 0.14 * lp)).powi(2)))
                        }
                    };
                    kinf * pr / (1.0 + pr) * f
                }
            };
            let kr = if !r.reversible {
                0.0
            } else if let Some(rv) = &r.rev {
                let k = arrh(rv);
                if r.kind == Kind::ThreeBody {
                    k * m()
                } else {
                    k
                }
            } else {
                let dg: f64 = r.products.iter().map(|&(k, nu)| nu * g_rt[k]).sum::<f64>()
                    - r.reactants.iter().map(|&(k, nu)| nu * g_rt[k]).sum::<f64>();
                let kc = (-dg).exp() * c_ref.powf(r.dnu);
                kf / kc
            };
            let conc = |list: &[(usize, f64)]| -> f64 {
                let mut p = 1.0;
                for &(k, nu) in list {
                    let ck = c[k].max(0.0);
                    p *= if nu == 1.0 {
                        ck
                    } else if nu == 2.0 {
                        ck * ck
                    } else {
                        ck.powf(nu)
                    };
                }
                p
            };
            let q = kf * conc(&r.reactants) - if r.reversible { kr * conc(&r.products) } else { 0.0 };
            for &(k, nu) in &r.reactants {
                wdot[k] -= nu * q;
            }
            for &(k, nu) in &r.products {
                wdot[k] += nu * q;
            }
        }
    }
}

fn parse_sections(text: &str, parts: &mut Parts) -> Result<(), MechError> {
    #[derive(PartialEq)]
    enum Sec {
        None,
        Elem,
        Spec,
        Thermo,
        React,
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut sec = Sec::None;
    let mut i = 0;
    let mut thermo_have_range = false;
    while i < lines.len() {
        let raw = lines[i];
        let ln = i + 1;
        i += 1;
        if sec == Sec::Thermo {
            // fixed-column records; comments only at line start
            let tr = raw.trim();
            if tr.is_empty() || tr.starts_with('!') {
                continue;
            }
            let fw = first_word(tr);
            if fw == "END" {
                sec = Sec::None;
                continue;
            }
            if !thermo_have_range && tr.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                let nums: Vec<&str> = tr.split_whitespace().collect();
                if nums.len() == 3 {
                    parts.thermo_default = Some([
                        parse_f(nums[0], ln, "Tlow")?,
                        parse_f(nums[1], ln, "Tmid")?,
                        parse_f(nums[2], ln, "Thigh")?,
                    ]);
                    thermo_have_range = true;
                    continue;
                }
            }
            if raw.len() < 79 {
                return merr(ln, "thermo record header shorter than 79 columns");
            }
            let sp = parse_thermo_record(&lines[i - 1..(i + 3).min(lines.len())], ln, parts.thermo_default)?;
            i += 3;
            parts.thermo.insert(sp.0.to_ascii_uppercase(), (sp.1, sp.2));
            continue;
        }
        let line = match raw.find('!') {
            Some(p) => &raw[..p],
            None => raw,
        };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let fw = first_word(t);
        let starts = |k: &str, full: &str| fw == k || fw == full;
        if sec == Sec::None
            || matches!(
                fw.as_str(),
                "ELEMENTS" | "ELEM" | "SPECIES" | "SPEC" | "THERMO" | "REACTIONS" | "REAC"
            )
        {
            if starts("ELEM", "ELEMENTS") {
                sec = Sec::Elem;
                parse_names(&t[fw.len()..], ln, parts, true)?;
                continue;
            } else if starts("SPEC", "SPECIES") {
                sec = Sec::Spec;
                parse_names(&t[fw.len()..], ln, parts, false)?;
                continue;
            } else if fw == "THERMO" {
                sec = Sec::Thermo;
                thermo_have_range = false;
                continue;
            } else if starts("REAC", "REACTIONS") {
                sec = Sec::React;
                let rest = t[fw.len()..].to_ascii_uppercase();
                for w in rest.split_whitespace() {
                    match w {
                        "MOLECULES" => return merr(ln, "MOLECULES units are not supported (use MOLES)"),
                        "MOLES" => {}
                        "CAL/MOLE" | "KCAL/MOLE" | "JOULES/MOLE" | "KJOULES/MOLE" | "KELVINS" | "EVOLTS" => {
                            parts.units = w.to_string()
                        }
                        other => return merr(ln, format!("unknown REACTIONS option '{other}'")),
                    }
                }
                continue;
            } else {
                return merr(ln, format!("unexpected text outside a section: '{t}'"));
            }
        }
        if fw == "END" {
            sec = Sec::None;
            continue;
        }
        match sec {
            Sec::Elem => parse_names(t, ln, parts, true)?,
            Sec::Spec => parse_names(t, ln, parts, false)?,
            Sec::React => parts.reaction_lines.push((ln, t.to_string())),
            _ => {}
        }
    }
    Ok(())
}

fn parse_names(text: &str, ln: usize, parts: &mut Parts, elem: bool) -> Result<(), MechError> {
    let mut toks: Vec<String> = Vec::new();
    // element weights may follow as NAME/value/
    let s = text.replace('/', " / ");
    let w: Vec<&str> = s.split_whitespace().collect();
    let mut k = 0;
    while k < w.len() {
        if w[k] == "END" && !elem {
            break;
        }
        if elem && k + 2 < w.len() + 1 && w.get(k + 1) == Some(&"/") {
            let val = parse_f(w.get(k + 2).copied().unwrap_or(""), ln, "element weight")?;
            parts.elements.push((w[k].to_ascii_uppercase(), Some(val)));
            k += 4;
            continue;
        }
        if w[k] == "END" {
            break;
        }
        toks.push(w[k].to_string());
        k += 1;
    }
    for t in toks {
        if elem {
            parts.elements.push((t.to_ascii_uppercase(), None));
        } else {
            parts.species.push(t);
        }
    }
    Ok(())
}

fn parse_thermo_record(
    rec: &[&str],
    ln: usize,
    default: Option<[f64; 3]>,
) -> Result<(String, Species, Vec<(String, f64)>), MechError> {
    if rec.len() < 4 {
        return merr(ln, "incomplete thermo record (need 4 lines)");
    }
    let h = rec[0];
    let col = |s: &str, a: usize, b: usize| -> String { s.chars().skip(a).take(b - a).collect() };
    let name = first_word(&col(h, 0, 18));
    if name.is_empty() {
        return merr(ln, "thermo record without species name");
    }
    let mut atoms: Vec<(String, f64)> = Vec::new();
    for e in 0..4 {
        let a = 24 + 5 * e;
        let ch = col(h, a, a + 5);
        let en = ch.chars().take(2).collect::<String>().trim().to_ascii_uppercase();
        let cn = ch.chars().skip(2).collect::<String>();
        if en.is_empty() || en == "0" {
            continue;
        }
        let cnt = parse_f(&cn, ln, "element count")?;
        if cnt != 0.0 {
            atoms.push((en, cnt));
        }
    }
    // temperatures: the three numbers after the phase column (Tlow, Thigh, Tmid); some files widen the Tmid field
    // into columns 74-75, so whitespace tokens are used instead of fixed columns
    let tail: Vec<String> = col(h, 45, 79).split_whitespace().map(str::to_string).collect();
    if tail
        .iter()
        .any(|t| t.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
    {
        return merr(
            ln,
            "a fifth element in columns 74-78 of the thermo header is not supported",
        );
    }
    let nums: Vec<f64> = tail.iter().filter_map(|t| t.parse::<f64>().ok()).collect();
    let (t_low, t_high, t_mid) = if nums.len() >= 4 || (nums.len() == 3 && default.is_none()) {
        (nums[0], nums[1], nums[2])
    } else {
        let d = default.unwrap_or([300.0, 1000.0, 5000.0]);
        (d[0], d[2], d[1])
    };
    let mut c = Vec::with_capacity(14);
    for (k, line) in rec[1..4].iter().enumerate() {
        let nf = if k == 2 { 4 } else { 5 };
        for f in 0..nf {
            let s = col(line, 15 * f, 15 * f + 15);
            c.push(parse_f(&s, ln + 1 + k, "thermo coefficient")?);
        }
    }
    let mut high = [0.0; 7];
    let mut low = [0.0; 7];
    high.copy_from_slice(&c[0..7]);
    low.copy_from_slice(&c[7..14]);
    let sp = Species {
        name: name.clone(),
        atoms: Vec::new(),
        w: 0.0,
        t_low,
        t_mid,
        t_high,
        low,
        high,
    };
    Ok((name, sp, atoms))
}

fn build(parts: Parts) -> Result<Mechanism, MechError> {
    // elements
    let mut elements: Vec<String> = Vec::new();
    let mut ew: Vec<f64> = Vec::new();
    for (name, w) in &parts.elements {
        let wt = match w.or_else(|| atomic_weight(name)) {
            Some(w) => w,
            None => {
                return merr(
                    0,
                    format!("element '{name}': no atomic weight known (give it as {name}/weight/)"),
                )
            }
        };
        elements.push(name.clone());
        ew.push(wt);
    }
    if elements.is_empty() {
        return merr(0, "no ELEMENTS section");
    }
    let names: Vec<String> = if parts.species.is_empty() {
        let mut v: Vec<String> = parts.thermo.keys().cloned().collect();
        v.sort();
        v
    } else {
        parts.species.clone()
    };
    let mut species = Vec::new();
    for n in &names {
        let Some((base, atoms_named)) = parts.thermo.get(&n.to_ascii_uppercase()).cloned() else {
            return merr(0, format!("species '{n}' has no thermodynamic data"));
        };
        let mut atoms = vec![0.0; elements.len()];
        for (en, cnt) in atoms_named {
            match elements.iter().position(|e| *e == en) {
                Some(k) => atoms[k] += cnt,
                None => {
                    return merr(
                        0,
                        format!("species '{n}' contains element '{en}' that is not listed in ELEMENTS"),
                    )
                }
            }
        }
        let w: f64 = atoms.iter().zip(&ew).map(|(a, w)| a * w).sum::<f64>() / 1000.0;
        let mut sp = base;
        sp.name = n.clone();
        sp.atoms = atoms;
        sp.w = w;
        species.push(sp);
    }
    let idx: HashMap<String, usize> = species
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.to_ascii_uppercase(), i))
        .collect();
    let (e_scale, _) = match parts.units.as_str() {
        "CAL/MOLE" => (4.184 / R_GAS, ()),
        "KCAL/MOLE" => (4184.0 / R_GAS, ()),
        "JOULES/MOLE" => (1.0 / R_GAS, ()),
        "KJOULES/MOLE" => (1000.0 / R_GAS, ()),
        "KELVINS" => (1.0, ()),
        "EVOLTS" => (96_485.332_12 / R_GAS, ()),
        u => return merr(0, format!("units {u}")),
    };
    let mut reactions: Vec<Reaction> = Vec::new();
    for (ln, text) in &parts.reaction_lines {
        let ln = *ln;
        let t = text.as_str();
        if t.contains('=') && !t.contains('/') {
            reactions.push(parse_reaction(t, ln, &idx, species.len(), e_scale)?);
            continue;
        }
        let Some(r) = reactions.last_mut() else {
            return merr(ln, "auxiliary line before any reaction");
        };
        parse_aux(t, ln, r, &idx, e_scale)?;
    }
    for r in &reactions {
        if r.kind == Kind::Falloff && r.low.is_none() {
            return merr(
                r.line,
                format!("falloff reaction '{}' has no LOW parameters", r.equation),
            );
        }
    }
    Ok(Mechanism {
        elements,
        species,
        reactions,
    })
}

fn convert_a(a: f64, order: f64) -> f64 {
    a * 1e-6_f64.powf(order - 1.0)
}

fn parse_reaction(
    text: &str,
    ln: usize,
    idx: &HashMap<String, usize>,
    ns: usize,
    e_scale: f64,
) -> Result<Reaction, MechError> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    if toks.len() < 4 {
        return merr(
            ln,
            format!("reaction needs an equation and three Arrhenius numbers: '{text}'"),
        );
    }
    let n = toks.len();
    let a = parse_f(toks[n - 3], ln, "A")?;
    let b = parse_f(toks[n - 2], ln, "b")?;
    let e = parse_f(toks[n - 1], ln, "E")?;
    let eq: String = toks[..n - 3].concat();
    let (lhs, rhs, reversible) = if let Some(p) = eq.find("<=>") {
        (&eq[..p], &eq[p + 3..], true)
    } else if let Some(p) = eq.find("=>") {
        (&eq[..p], &eq[p + 2..], false)
    } else if let Some(p) = eq.find('=') {
        (&eq[..p], &eq[p + 1..], true)
    } else {
        return merr(ln, "no reaction arrow");
    };
    let mut falloff: Option<String> = None;
    let mut strip = |s: &str| -> Result<String, MechError> {
        if let Some(p) = s.find("(+") {
            let end = s[p..].find(')').map(|q| p + q).ok_or(MechError {
                line: ln,
                msg: "unbalanced '(+'".into(),
            })?;
            let partner = s[p + 2..end].to_ascii_uppercase();
            if let Some(prev) = &falloff {
                if *prev != partner {
                    return merr(ln, "falloff collision partner differs between the sides");
                }
            }
            falloff = Some(partner);
            Ok(format!("{}{}", &s[..p], &s[end + 1..]))
        } else {
            Ok(s.to_string())
        }
    };
    let lhs = strip(lhs)?;
    let rhs = strip(rhs)?;
    let mut three_body = false;
    let mut side = |s: &str| -> Result<Vec<(usize, f64)>, MechError> {
        let mut out: Vec<(usize, f64)> = Vec::new();
        for term in s.split('+') {
            if term.is_empty() {
                continue;
            }
            if term.eq_ignore_ascii_case("M") {
                three_body = true;
                continue;
            }
            let pre: String = term.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let (coef, name) = if pre.is_empty() || pre.len() == term.len() {
                (1.0, term)
            } else {
                (parse_f(&pre, ln, "stoichiometric coefficient")?, &term[pre.len()..])
            };
            let Some(&k) = idx.get(&name.to_ascii_uppercase()) else {
                return merr(ln, format!("unknown species '{name}' in '{text}'"));
            };
            out.push((k, coef));
        }
        Ok(out)
    };
    let reactants = side(&lhs)?;
    let products = side(&rhs)?;
    if reactants.is_empty() || products.is_empty() {
        return merr(ln, format!("empty side in '{text}'"));
    }
    let nre: f64 = reactants.iter().map(|x| x.1).sum();
    let npr: f64 = products.iter().map(|x| x.1).sum();
    let (kind, collider) = match (&falloff, three_body) {
        (Some(_), true) => return merr(ln, "both (+M) and +M in one reaction"),
        (Some(p), false) => {
            let col = if p == "M" {
                None
            } else {
                Some(*idx.get(p).ok_or(MechError {
                    line: ln,
                    msg: format!("unknown falloff partner '{p}'"),
                })?)
            };
            (Kind::Falloff, col)
        }
        (None, true) => (Kind::ThreeBody, None),
        (None, false) => (Kind::Elementary, None),
    };
    let order = nre + if kind == Kind::ThreeBody { 1.0 } else { 0.0 };
    Ok(Reaction {
        equation: eq.clone(),
        line: ln,
        reactants,
        products,
        reversible,
        kind,
        arr: [convert_a(a, order), b, e * e_scale],
        low: None,
        troe: None,
        rev: None,
        eff: if kind == Kind::Elementary {
            Vec::new()
        } else {
            vec![1.0; ns]
        },
        collider,
        dnu: npr - nre,
    })
}

fn parse_aux(
    text: &str,
    ln: usize,
    r: &mut Reaction,
    idx: &HashMap<String, usize>,
    e_scale: f64,
) -> Result<(), MechError> {
    let up = text.to_ascii_uppercase();
    if up.trim() == "DUP" || up.trim() == "DUPLICATE" {
        return Ok(());
    }
    let pieces: Vec<&str> = text.split('/').collect();
    let mut k = 0;
    while k + 1 < pieces.len() {
        let name = pieces[k].trim();
        let vals: Vec<&str> = pieces[k + 1].split_whitespace().collect();
        k += 2;
        // a line may start with DUP followed by something; ignore a leading DUP token
        let name = name
            .strip_prefix("DUPLICATE")
            .or_else(|| name.strip_prefix("DUP"))
            .map_or(name, str::trim);
        let key = name.to_ascii_uppercase();
        let nums = || -> Result<Vec<f64>, MechError> { vals.iter().map(|v| parse_f(v, ln, "number")).collect() };
        match key.as_str() {
            "LOW" => {
                if r.kind != Kind::Falloff {
                    return merr(ln, "LOW given for a reaction without (+M)");
                }
                let v = nums()?;
                if v.len() != 3 {
                    return merr(ln, "LOW needs 3 numbers");
                }
                let order: f64 = r.reactants.iter().map(|x| x.1).sum::<f64>() + 1.0;
                r.low = Some([convert_a(v[0], order), v[1], v[2] * e_scale]);
            }
            "TROE" => {
                let v = nums()?;
                if v.len() != 3 && v.len() != 4 {
                    return merr(ln, "TROE needs 3 or 4 numbers");
                }
                r.troe = Some([v[0], v[1], v[2], if v.len() == 4 { v[3] } else { f64::INFINITY }]);
            }
            "REV" => {
                let v = nums()?;
                if v.len() != 3 {
                    return merr(ln, "REV needs 3 numbers");
                }
                if r.kind == Kind::Falloff {
                    return merr(ln, "REV with a falloff reaction is not supported");
                }
                let order: f64 =
                    r.products.iter().map(|x| x.1).sum::<f64>() + if r.kind == Kind::ThreeBody { 1.0 } else { 0.0 };
                r.rev = Some([convert_a(v[0], order), v[1], v[2] * e_scale]);
                r.reversible = true;
            }
            "SRI" | "PLOG" | "FORD" | "RORD" | "LT" | "RLT" | "HIGH" | "TDEP" | "EXCI" | "UNITS" | "JAN" | "FIT1"
            | "XSMI" => {
                return merr(ln, format!("auxiliary keyword {key} is not supported"));
            }
            _ => {
                if r.kind == Kind::Elementary {
                    return merr(
                        ln,
                        format!("efficiency '{name}' given for a reaction without a third body"),
                    );
                }
                let Some(&sp) = idx.get(&key) else {
                    return merr(ln, format!("unknown species '{name}' in efficiencies"));
                };
                let v = nums()?;
                if v.len() != 1 {
                    return merr(ln, "efficiency needs one number");
                }
                r.eff[sp] = v[0];
            }
        }
    }
    Ok(())
}
