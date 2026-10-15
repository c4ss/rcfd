//! JSON interchange of a [`Mechanism`] (ADR 0027 D6b, roadmap F-224): the project's own schema, a lossless image of the
//! in-memory mechanism, so a mechanism read from CHEMKIN text can be stored and re-read without the text parser.
//!
//! Layout: `{"format": "rcfd-mechanism", "version": 1, "elements": [..], "species": [{"name", "atoms", "w", "t_low",
//! "t_mid", "t_high", "nasa7_low", "nasa7_high", "nasa9": null | [{"t_low","t_high","a","b"}]}],
//! "reactions": [{"equation", "line", "reactants": [[index, nu]], "products": [[index, nu]], "reversible",
//! "type": "elementary" | "three_body" | "falloff", "arrhenius": [A, b, Ta], "low": null | [..], "troe": null | [alpha, T3, T1, T2],
//! "rev": null | [..], "efficiencies": [..], "collider": null | index, "dnu"}]}`. Units are SI (`A` in the
//! units of the reaction order, `Ta` in K). An absent Troe `T2` is stored as `null`.

use crate::mech::{Kind, MechError, Mechanism, Nasa9Range, Reaction, Species};
use serde_json::{json, Value};

fn err<T>(msg: impl Into<String>) -> Result<T, MechError> {
    Err(MechError {
        line: 0,
        msg: msg.into(),
    })
}

fn num_or_null(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else {
        Value::Null
    }
}

fn pairs(v: &[(usize, f64)]) -> Value {
    Value::Array(v.iter().map(|(k, nu)| json!([k, nu])).collect())
}

/// Serialise a mechanism.
pub fn to_json(m: &Mechanism) -> Value {
    let species: Vec<Value> = m
        .species
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "atoms": s.atoms,
                "w": s.w,
                "t_low": s.t_low,
                "t_mid": s.t_mid,
                "t_high": s.t_high,
                "nasa7_low": s.low,
                "nasa7_high": s.high,
                "nasa9": s.nasa9.as_ref().map(|r| r.iter().map(|x| json!({
                    "t_low": x.t_low, "t_high": x.t_high, "a": x.a, "b": x.b,
                })).collect::<Vec<_>>()),
            })
        })
        .collect();
    let reactions: Vec<Value> = m
        .reactions
        .iter()
        .map(|r| {
            json!({
                "equation": r.equation,
                "line": r.line,
                "reactants": pairs(&r.reactants),
                "products": pairs(&r.products),
                "reversible": r.reversible,
                "type": match r.kind {
                    Kind::Elementary => "elementary",
                    Kind::ThreeBody => "three_body",
                    Kind::Falloff => "falloff",
                },
                "arrhenius": r.arr,
                "low": r.low,
                "troe": r.troe.map(|t| t.iter().map(|v| num_or_null(*v)).collect::<Vec<_>>()),
                "rev": r.rev,
                "efficiencies": r.eff,
                "collider": r.collider,
                "dnu": r.dnu,
            })
        })
        .collect();
    json!({
        "format": "rcfd-mechanism",
        "version": 1,
        "elements": m.elements,
        "species": species,
        "reactions": reactions,
    })
}

fn f64_of(v: &Value, what: &str) -> Result<f64, MechError> {
    v.as_f64().map_or_else(|| err(format!("{what} is not a number")), Ok)
}

fn arr_n<const N: usize>(v: &Value, what: &str) -> Result<[f64; N], MechError> {
    let a = v.as_array().filter(|a| a.len() == N);
    let Some(a) = a else {
        return err(format!("{what} must be an array of {N} numbers"));
    };
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = f64_of(x, what)?;
    }
    Ok(out)
}

fn vec_f(v: &Value, what: &str) -> Result<Vec<f64>, MechError> {
    let Some(a) = v.as_array() else {
        return err(format!("{what} must be an array"));
    };
    a.iter().map(|x| f64_of(x, what)).collect()
}

fn get<'a>(v: &'a Value, key: &str) -> Result<&'a Value, MechError> {
    v.get(key).map_or_else(|| err(format!("missing key '{key}'")), Ok)
}

fn opt_arr3(v: &Value, what: &str) -> Result<Option<[f64; 3]>, MechError> {
    if v.is_null() {
        Ok(None)
    } else {
        arr_n::<3>(v, what).map(Some)
    }
}

fn read_pairs(v: &Value, ns: usize, what: &str) -> Result<Vec<(usize, f64)>, MechError> {
    let Some(a) = v.as_array() else {
        return err(format!("{what} must be an array"));
    };
    a.iter()
        .map(|p| {
            let k = p.get(0).and_then(Value::as_u64).map(|k| k as usize);
            let nu = p.get(1).and_then(Value::as_f64);
            match (k, nu) {
                (Some(k), Some(nu)) if k < ns => Ok((k, nu)),
                _ => err(format!(
                    "{what}: entries are [species index, coefficient] with a valid index"
                )),
            }
        })
        .collect()
}

/// Read a mechanism written by [`to_json`].
pub fn from_json(v: &Value) -> Result<Mechanism, MechError> {
    if get(v, "format")?.as_str() != Some("rcfd-mechanism") {
        return err("not an rcfd-mechanism document (key 'format')");
    }
    if get(v, "version")?.as_u64() != Some(1) {
        return err("unsupported mechanism version (only 1 is known)");
    }
    let elements: Vec<String> = get(v, "elements")?
        .as_array()
        .map_or_else(|| err("'elements' must be an array"), Ok)?
        .iter()
        .map(|e| {
            e.as_str()
                .map(str::to_string)
                .map_or_else(|| err("element names are strings"), Ok)
        })
        .collect::<Result<_, _>>()?;
    let mut species = Vec::new();
    for s in get(v, "species")?
        .as_array()
        .map_or_else(|| err("'species' must be an array"), Ok)?
    {
        let nasa9 = match get(s, "nasa9")? {
            Value::Null => None,
            Value::Array(rs) => Some(
                rs.iter()
                    .map(|r| {
                        Ok(Nasa9Range {
                            t_low: f64_of(get(r, "t_low")?, "t_low")?,
                            t_high: f64_of(get(r, "t_high")?, "t_high")?,
                            a: arr_n::<7>(get(r, "a")?, "nasa9 a")?,
                            b: arr_n::<2>(get(r, "b")?, "nasa9 b")?,
                        })
                    })
                    .collect::<Result<Vec<_>, MechError>>()?,
            ),
            _ => return err("'nasa9' must be null or an array of ranges"),
        };
        let atoms = vec_f(get(s, "atoms")?, "atoms")?;
        if atoms.len() != elements.len() {
            return err("species 'atoms' must have one entry per element");
        }
        species.push(Species {
            name: get(s, "name")?
                .as_str()
                .map_or_else(|| err("species name must be a string"), |n| Ok(n.to_string()))?,
            atoms,
            w: f64_of(get(s, "w")?, "w")?,
            t_low: f64_of(get(s, "t_low")?, "t_low")?,
            t_mid: f64_of(get(s, "t_mid")?, "t_mid")?,
            t_high: f64_of(get(s, "t_high")?, "t_high")?,
            low: arr_n::<7>(get(s, "nasa7_low")?, "nasa7_low")?,
            high: arr_n::<7>(get(s, "nasa7_high")?, "nasa7_high")?,
            nasa9,
        });
    }
    let ns = species.len();
    let mut reactions = Vec::new();
    for r in get(v, "reactions")?
        .as_array()
        .map_or_else(|| err("'reactions' must be an array"), Ok)?
    {
        let kind = match get(r, "type")?.as_str() {
            Some("elementary") => Kind::Elementary,
            Some("three_body") => Kind::ThreeBody,
            Some("falloff") => Kind::Falloff,
            _ => return err("reaction 'type' must be elementary, three_body or falloff"),
        };
        let troe = match get(r, "troe")? {
            Value::Null => None,
            Value::Array(t) if t.len() == 4 => {
                let mut v = [0.0; 4];
                for (i, x) in t.iter().enumerate() {
                    v[i] = match x {
                        // only the fourth parameter T2 may be absent (infinity)
                        Value::Null if i == 3 => f64::INFINITY,
                        _ => f64_of(x, "troe")?,
                    };
                }
                Some(v)
            }
            _ => return err("'troe' must be null or four entries"),
        };
        let eff = vec_f(get(r, "efficiencies")?, "efficiencies")?;
        if !eff.is_empty() && eff.len() != ns {
            return err("'efficiencies' must be empty or have one entry per species");
        }
        let collider = match get(r, "collider")? {
            Value::Null => None,
            c => match c.as_u64() {
                Some(k) if (k as usize) < ns => Some(k as usize),
                _ => return err("'collider' must be null or a valid species index"),
            },
        };
        reactions.push(Reaction {
            equation: get(r, "equation")?
                .as_str()
                .map_or_else(|| err("equation must be a string"), |n| Ok(n.to_string()))?,
            line: get(r, "line")?
                .as_u64()
                .map_or_else(|| err("'line' must be a non-negative integer"), Ok)? as usize,
            reactants: read_pairs(get(r, "reactants")?, ns, "reactants")?,
            products: read_pairs(get(r, "products")?, ns, "products")?,
            reversible: get(r, "reversible")?
                .as_bool()
                .map_or_else(|| err("'reversible' must be a boolean"), Ok)?,
            kind,
            arr: arr_n::<3>(get(r, "arrhenius")?, "arrhenius")?,
            low: opt_arr3(get(r, "low")?, "low")?,
            troe,
            rev: opt_arr3(get(r, "rev")?, "rev")?,
            eff,
            collider,
            dnu: f64_of(get(r, "dnu")?, "dnu")?,
        });
    }
    Ok(Mechanism {
        elements,
        species,
        reactions,
    })
}

/// Read a mechanism from JSON text.
pub fn from_json_str(text: &str) -> Result<Mechanism, MechError> {
    let v: Value = serde_json::from_str(text).map_or_else(
        |e| {
            Err(MechError {
                line: e.line(),
                msg: format!("invalid JSON: {e}"),
            })
        },
        Ok,
    )?;
    from_json(&v)
}
