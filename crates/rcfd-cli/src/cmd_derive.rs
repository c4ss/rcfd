//! `rcfd post derive <store dir> --kind ... --out <file.glb>` (ADR 0738 P4): a derived picture of a stored field without a
//! project, by `rcfd_workspace::derive` (the library of the project tool `derive_picture`, so both make the same file).
//! The flags become that function's arguments; the reply is printed as one JSON line.

use crate::{flag, opt, positionals, usage_error, FAIL, OK};
use serde_json::{json, Map, Value};
use std::path::Path;

const VALUE_OPTS: [&str; 18] = [
    "--kind",
    "--field",
    "--level",
    "--colour-by",
    "--seeds",
    "--direction",
    "--max-steps",
    "--max-length",
    "--min-speed",
    "--step-fraction",
    "--axis",
    "--at",
    "--keep",
    "--snapshot",
    "--step",
    "--time",
    "--out",
    "--rake",
];

fn number(args: &[String], name: &str) -> Result<Option<Value>, String> {
    match opt(args, name) {
        None => Ok(None),
        Some(t) => t
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .map(|x| Some(json!(x)))
            .ok_or_else(|| format!("{name} needs a finite number, not '{t}'")),
    }
}

fn integer(args: &[String], name: &str) -> Result<Option<Value>, String> {
    match opt(args, name) {
        None => Ok(None),
        Some(t) => t
            .parse::<u64>()
            .map(|x| Some(json!(x)))
            .map_err(|_| format!("{name} needs a non-negative integer, not '{t}'")),
    }
}

/// The arguments of `rcfd_workspace::derive::derive` from the flags.
fn arguments(args: &[String]) -> Result<Value, String> {
    let mut a = Map::new();
    for (flag_name, key) in [
        ("--kind", "kind"),
        ("--field", "field"),
        ("--colour-by", "colour_by"),
        ("--direction", "direction"),
        ("--axis", "axis"),
        ("--keep", "keep"),
    ] {
        if let Some(v) = opt(args, flag_name) {
            a.insert(key.into(), json!(v));
        }
    }
    for (flag_name, key) in [
        ("--level", "level"),
        ("--max-length", "max_length"),
        ("--min-speed", "min_speed"),
        ("--step-fraction", "step_fraction"),
        ("--at", "at"),
        ("--time", "time"),
    ] {
        if let Some(v) = number(args, flag_name)? {
            a.insert(key.into(), v);
        }
    }
    for (flag_name, key) in [
        ("--max-steps", "max_steps"),
        ("--snapshot", "snapshot"),
        ("--step", "step"),
    ] {
        if let Some(v) = integer(args, flag_name)? {
            a.insert(key.into(), v);
        }
    }
    if let Some(s) = opt(args, "--seeds") {
        let v: Value = serde_json::from_str(s).map_err(|e| format!("--seeds needs JSON ({e})"))?;
        a.insert("seeds".into(), v);
    }
    if let Some(r) = opt(args, "--rake") {
        let x: Vec<f64> = r
            .split(',')
            .map(|t| t.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("--rake needs x0,y0,z0,x1,y1,z1,n, not '{r}'"))?;
        if x.len() != 7 || x[6] < 1.0 || x[6].fract() != 0.0 {
            return Err(format!("--rake needs x0,y0,z0,x1,y1,z1,n, not '{r}'"));
        }
        a.insert(
            "seeds".into(),
            json!({"rake": {"p0": [x[0], x[1], x[2]], "p1": [x[3], x[4], x[5]], "n": x[6] as u64}}),
        );
    }
    if flag(args, "--vectors") {
        a.insert("vectors".into(), json!(true));
    }
    Ok(Value::Object(a))
}

/// `rcfd post derive ...`: exit 0 with the reply on stdout, 1 when the picture cannot be made (the error as JSON on
/// stderr), 2 on a usage error.
pub(crate) fn cmd_derive(args: &[String]) -> u8 {
    let pos = positionals(args, &VALUE_OPTS);
    let [dir] = pos[..] else {
        return usage_error("post derive takes exactly one field store directory (runs/<id>/fields)");
    };
    let Some(out) = opt(args, "--out") else {
        return usage_error("post derive needs --out <file.glb>");
    };
    if opt(args, "--kind").is_none() {
        return usage_error("post derive needs --kind iso|streamlines|slice|cut_surface");
    }
    let a = match arguments(args) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let fail = |code: i64, message: String| {
        eprintln!("{}", json!({"error": {"code": code, "message": message}}));
        FAIL
    };
    let d = match rcfd_workspace::derive::derive_dir(Path::new(dir), &a) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{}",
                json!({"error": {"code": e.code, "message": e.message, "data": e.data}})
            );
            return FAIL;
        }
    };
    if let Err(e) = std::fs::write(out, &d.glb) {
        return fail(-32000, format!("{out}: {e}"));
    }
    let mut v = d.summary;
    v["path"] = json!(out);
    v["bytes"] = json!(d.glb.len());
    println!("{v}");
    OK
}
