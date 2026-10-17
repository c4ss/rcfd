//! Finite-element load deck in a Nastran-style bulk data subset, free field (comma separated), full double precision
//! (F-237, ADR 0051 D8).
//!
//! Cards: `GRID, ID, , X, Y, Z`; `CTRIA3, EID, PID, G1, G2, G3`; `CQUAD4, EID, PID, G1..G4`; `CHEXA, EID, PID, G1..G6` with a
//! continuation line `, G7, G8`; `PLOAD4, SID, EID, P1, P2, P3, P4` (blank `P2..P4` = uniform `P1`);
//! `TEMP, SID, G1, T1, G2, T2, G3, T3`. Field positions were read from public pyNastran documentation (PLOAD4, TEMP) or are
//! recalled (GRID, element cards, continuation lines, free-field syntax) and UNVERIFIED against a solver; the sign convention of
//! `PLOAD4` (here: positive = fluid pressure pressing on the structure) is unverified as well. The reader parses exactly the
//! written subset and reports `line N` in every error.

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use std::collections::HashSet;
use std::fmt::Write;

/// Element type of the supported cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElemKind {
    /// 3-node shell.
    Tria3,
    /// 4-node shell.
    Quad4,
    /// 8-node solid.
    Hexa8,
}

impl ElemKind {
    fn card(self) -> &'static str {
        match self {
            ElemKind::Tria3 => "CTRIA3",
            ElemKind::Quad4 => "CQUAD4",
            ElemKind::Hexa8 => "CHEXA",
        }
    }
    fn n_nodes(self) -> usize {
        match self {
            ElemKind::Tria3 => 3,
            ElemKind::Quad4 => 4,
            ElemKind::Hexa8 => 8,
        }
    }
}

/// One element.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Type.
    pub kind: ElemKind,
    /// Element id.
    pub id: i64,
    /// Property id.
    pub pid: i64,
    /// Grid ids.
    pub nodes: Vec<i64>,
}

/// Element pressure load: `p[0]` alone (others NaN) is a uniform pressure.
#[derive(Debug, Clone, PartialEq)]
pub struct Pload4 {
    /// Load set id.
    pub sid: i64,
    /// Element id.
    pub eid: i64,
    /// Corner pressures; NaN = blank.
    pub p: [f64; 4],
}

/// A load deck.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deck {
    /// Grid points `(id, xyz)`.
    pub grids: Vec<(i64, Vec3)>,
    /// Elements.
    pub elements: Vec<Element>,
    /// Pressure loads.
    pub pload4: Vec<Pload4>,
    /// Nodal temperatures `(set id, grid id, T)`.
    pub temps: Vec<(i64, i64, f64)>,
}

fn real(x: f64) -> String {
    format!("{x:?}")
}

impl Deck {
    /// Write the deck. Ids must be positive and unique, coordinates and loads finite (a blank corner pressure is NaN).
    pub fn to_string_checked(&self) -> Result<String> {
        self.validate()?;
        let mut o = String::from("$ rcfd load deck (free field)\nBEGIN BULK\n");
        for (id, x) in &self.grids {
            let _ = writeln!(o, "GRID,{id},,{},{},{}", real(x[0]), real(x[1]), real(x[2]));
        }
        for e in &self.elements {
            let _ = write!(o, "{},{},{}", e.kind.card(), e.id, e.pid);
            for (k, n) in e.nodes.iter().enumerate() {
                if e.kind == ElemKind::Hexa8 && k == 6 {
                    o.push('\n');
                }
                let _ = write!(o, ",{n}");
            }
            o.push('\n');
        }
        for l in &self.pload4 {
            let _ = write!(o, "PLOAD4,{},{},{}", l.sid, l.eid, real(l.p[0]));
            for v in &l.p[1..] {
                if v.is_nan() {
                    o.push(',');
                } else {
                    let _ = write!(o, ",{}", real(*v));
                }
            }
            o.push('\n');
        }
        let mut i = 0;
        while i < self.temps.len() {
            // one card per run of at most three pairs with the same set id
            let mut j = i;
            while j < self.temps.len() && j - i < 3 && self.temps[j].0 == self.temps[i].0 {
                j += 1;
            }
            let _ = write!(o, "TEMP,{}", self.temps[i].0);
            for t in &self.temps[i..j] {
                let _ = write!(o, ",{},{}", t.1, real(t.2));
            }
            o.push('\n');
            i = j;
        }
        o.push_str("ENDDATA\n");
        Ok(o)
    }

    fn validate(&self) -> Result<()> {
        let bad = |m: String| Error::Io(format!("load deck: {m}"));
        let mut ids = HashSet::new();
        for (id, x) in &self.grids {
            if *id <= 0 || !ids.insert(*id) {
                return Err(bad(format!("grid id {id} is not positive and unique")));
            }
            if x.iter().any(|v| !v.is_finite()) {
                return Err(bad(format!("grid {id} has a non-finite coordinate")));
            }
        }
        let mut eids = HashSet::new();
        for e in &self.elements {
            if e.id <= 0 || !eids.insert(e.id) {
                return Err(bad(format!("element id {} is not positive and unique", e.id)));
            }
            if e.nodes.len() != e.kind.n_nodes() {
                return Err(bad(format!(
                    "element {} has {} nodes, {} needed",
                    e.id,
                    e.nodes.len(),
                    e.kind.n_nodes()
                )));
            }
            for n in &e.nodes {
                if !ids.contains(n) {
                    return Err(bad(format!("element {} references missing grid {n}", e.id)));
                }
            }
        }
        for l in &self.pload4 {
            if !eids.contains(&l.eid) {
                return Err(bad(format!("PLOAD4 references missing element {}", l.eid)));
            }
            if !l.p[0].is_finite() || l.p[1..].iter().any(|v| !v.is_nan() && !v.is_finite()) {
                return Err(bad(format!("PLOAD4 on element {} has a non-finite pressure", l.eid)));
            }
        }
        for (s, g, t) in &self.temps {
            if !ids.contains(g) {
                return Err(bad(format!("TEMP set {s} references missing grid {g}")));
            }
            if !t.is_finite() {
                return Err(bad(format!("TEMP set {s} grid {g} has a non-finite temperature")));
            }
        }
        Ok(())
    }

    /// Parse a deck written by [`Deck::to_string_checked`].
    pub fn parse(text: &str) -> Result<Deck> {
        let mut deck = Deck::default();
        // join continuation lines (start with ',' or '+') to the previous card
        let mut cards: Vec<(usize, String)> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('$') {
                continue;
            }
            if line.starts_with(',') || line.starts_with('+') {
                match cards.last_mut() {
                    Some((_, s)) => {
                        s.push_str(line.trim_start_matches('+'));
                    }
                    None => return Err(err(i + 1, "continuation line without a card")),
                }
            } else {
                cards.push((i + 1, line.to_string()));
            }
        }
        let mut seen_end = false;
        let mut gids = HashSet::new();
        let mut eids = HashSet::new();
        for (ln, card) in &cards {
            let ln = *ln;
            if seen_end {
                return Err(err(ln, "data after ENDDATA"));
            }
            let f: Vec<&str> = card.split(',').map(|s| s.trim()).collect();
            let int = |k: usize, what: &str| -> Result<i64> {
                f.get(k)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| err(ln, &format!("{} card: missing {what}", f[0])))?
                    .parse::<i64>()
                    .map_err(|_| err(ln, &format!("{} card: {what} '{}' is not an integer", f[0], f[k])))
            };
            let flt = |k: usize, what: &str| -> Result<f64> {
                let v = f
                    .get(k)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| err(ln, &format!("{} card: missing {what}", f[0])))?
                    .parse::<f64>()
                    .map_err(|_| err(ln, &format!("{} card: {what} '{}' is not a number", f[0], f[k])))?;
                if !v.is_finite() {
                    return Err(err(ln, &format!("{} card: {what} is not finite", f[0])));
                }
                Ok(v)
            };
            match f[0].to_ascii_uppercase().as_str() {
                "BEGIN BULK" => {}
                "ENDDATA" => seen_end = true,
                "GRID" => {
                    let id = int(1, "ID")?;
                    if id <= 0 || !gids.insert(id) {
                        return Err(err(ln, &format!("GRID id {id} is not positive and unique")));
                    }
                    deck.grids.push((id, [flt(3, "X1")?, flt(4, "X2")?, flt(5, "X3")?]));
                }
                c @ ("CTRIA3" | "CQUAD4" | "CHEXA") => {
                    let kind = match c {
                        "CTRIA3" => ElemKind::Tria3,
                        "CQUAD4" => ElemKind::Quad4,
                        _ => ElemKind::Hexa8,
                    };
                    let id = int(1, "EID")?;
                    if id <= 0 || !eids.insert(id) {
                        return Err(err(ln, &format!("{c} element id {id} is not positive and unique")));
                    }
                    let pid = int(2, "PID")?;
                    let mut nodes = Vec::new();
                    for k in 0..kind.n_nodes() {
                        let n = int(3 + k, &format!("G{}", k + 1))?;
                        if !gids.contains(&n) {
                            return Err(err(
                                ln,
                                &format!("{c} {id}: grid {n} is not defined before the element"),
                            ));
                        }
                        nodes.push(n);
                    }
                    if f.len() > 3 + kind.n_nodes() {
                        return Err(err(ln, &format!("{c} {id}: too many fields")));
                    }
                    deck.elements.push(Element { kind, id, pid, nodes });
                }
                "PLOAD4" => {
                    let sid = int(1, "SID")?;
                    let eid = int(2, "EID")?;
                    if !eids.contains(&eid) {
                        return Err(err(ln, &format!("PLOAD4 references undefined element {eid}")));
                    }
                    let mut p = [f64::NAN; 4];
                    p[0] = flt(3, "P1")?;
                    for (k, pk) in p.iter_mut().enumerate().skip(1) {
                        if f.get(3 + k).is_some_and(|s| !s.is_empty()) {
                            *pk = flt(3 + k, &format!("P{}", k + 1))?;
                        }
                    }
                    deck.pload4.push(Pload4 { sid, eid, p });
                }
                "TEMP" => {
                    let sid = int(1, "SID")?;
                    let mut k = 2;
                    let mut n = 0;
                    while k < f.len() && !f[k].is_empty() {
                        let g = int(k, "G")?;
                        if !gids.contains(&g) {
                            return Err(err(ln, &format!("TEMP references undefined grid {g}")));
                        }
                        deck.temps.push((sid, g, flt(k + 1, "T")?));
                        k += 2;
                        n += 1;
                    }
                    if n == 0 || n > 3 {
                        return Err(err(ln, "TEMP card needs one to three (G, T) pairs"));
                    }
                }
                other => return Err(err(ln, &format!("unknown card '{other}'"))),
            }
        }
        if !seen_end {
            return Err(Error::Io("load deck: missing ENDDATA".into()));
        }
        Ok(deck)
    }
}

fn err(line: usize, msg: &str) -> Error {
    Error::Io(format!("load deck: line {line}: {msg}"))
}
