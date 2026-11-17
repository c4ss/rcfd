//! The user's sites file (ADR 0739 P3, Q8 (a)): `%APPDATA%\rcfd\sites.toml` on Windows, `$XDG_CONFIG_HOME/rcfd/sites.toml`
//! or `~/.config/rcfd/sites.toml` elsewhere; `RCFD_SITES_FILE` names another file. Projects never hold hosts or
//! credentials: a project or a case can name a site only by its name, which must be in this file. rcfd stores no
//! secret here or anywhere: an `ssh` site uses OpenSSH's keys, agent, `known_hosts` and `~/.ssh/config`.
//!
//! ```toml
//! [site.lab]
//! kind = "ssh"                       # or "exec" (tests only)
//! host = "me@hpc.example.org"        # one ssh argument
//! engine = "/opt/rcfd/bin"           # the folder of the site's rcfd and rcfd-runner
//! work = "/scratch/me/rcfd"          # the site's work root, created 0700
//! max_runs = 4                       # the site runner's budgets (ADR 0733)
//! cpu_threads = 64
//! mem_budget_mb = 250000
//! assistant = false                  # Q6 (a): may MCP clients name this site
//! confirmed = "SHA256:..."           # written by `rcfd site check lab --confirm`
//! accept_engines = ["rcfd 0.9.4 (abc)"]   # Q10 (a): engines other than this machine's the user said yes to
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Names another sites file.
pub const ENV_FILE: &str = "RCFD_SITES_FILE";
/// Allows sites of kind `exec` (the fake remote of the tests: the agent spawned directly, no `ssh`).
pub const ENV_TEST_EXEC: &str = "RCFD_TEST_EXEC_SITES";

/// The file as it parses.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SitesFile {
    #[serde(default)]
    site: BTreeMap<String, SiteEntry>,
}

/// One `[site.<name>]` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteEntry {
    /// `ssh` or `exec`.
    pub kind: String,
    /// The `ssh` destination (`ssh` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The folder of the site's `rcfd` and `rcfd-runner`.
    pub engine: String,
    /// The site's work root.
    pub work: String,
    /// Site runner budget `max_runs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs: Option<u64>,
    /// Site runner budget `cpu_threads`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_threads: Option<u64>,
    /// Site runner budget `mem_budget_mb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_budget_mb: Option<u64>,
    /// Whether MCP clients may name this site.
    #[serde(default)]
    pub assistant: bool,
    /// What the user confirmed once: the host key fingerprint (`ssh`), `exec` for an exec site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed: Option<String>,
    /// Engines other than this machine's the user said yes to (`<version> (<git_rev>)`).
    #[serde(default)]
    pub accept_engines: Vec<String>,
}

/// The sites file of this user: `RCFD_SITES_FILE`, else the platform's config folder; `None` without a home.
pub fn sites_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(ENV_FILE).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("rcfd").join("sites.toml"))
}

/// Every site of the file at `path` (none when it does not exist); an error names the file.
pub fn load_from(path: &Path) -> Result<BTreeMap<String, SiteEntry>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let f: SitesFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(f.site)
}

/// Every site of this user's file.
pub fn load() -> Result<(PathBuf, BTreeMap<String, SiteEntry>), String> {
    let path = sites_path().ok_or("no sites file: neither RCFD_SITES_FILE nor a user config folder is set")?;
    let sites = load_from(&path)?;
    Ok((path, sites))
}

/// Whether sites of kind `exec` and the agent's test answers are allowed (`RCFD_TEST_EXEC_SITES=1`).
pub fn exec_allowed() -> bool {
    matches!(std::env::var(ENV_TEST_EXEC).as_deref(), Ok("1"))
}

/// A site name: letters, digits, `_` and `-`, at most 64.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A value given to a program as one argument or put in the agent's command line: not empty, not starting with `-`, no
/// control character and no quote.
fn plain_arg(v: &str) -> bool {
    !v.is_empty() && !v.starts_with('-') && !v.chars().any(|c| c.is_control() || c == '\'' || c == '"')
}

/// The named site, checked (ADR 0739 P11): known, of a known kind, `host` an ssh destination that cannot be read as an
/// option, `engine` and `work` plain; `exec` only where tests allow it. Errors name the file and the table. Nothing is
/// started.
pub fn site(name: &str) -> Result<(PathBuf, SiteEntry), String> {
    if !valid_name(name) {
        return Err(format!("'{name}' is not a site name (letters, digits, _ and -)"));
    }
    let (path, sites) = load()?;
    let Some(e) = sites.get(name) else {
        return Err(format!(
            "site '{name}' is not configured in {} (rcfd site list shows the sites)",
            path.display()
        ));
    };
    e.check(name)
        .map_err(|m| format!("{} [site.{name}]: {m}", path.display()))?;
    Ok((path, e.clone()))
}

impl SiteEntry {
    /// The checks of [`site`] on this entry.
    pub fn check(&self, name: &str) -> Result<(), String> {
        let _ = name;
        match self.kind.as_str() {
            "ssh" => {
                let host = self.host.as_deref().ok_or("an ssh site needs host")?;
                if !plain_arg(host) || host.chars().any(char::is_whitespace) {
                    return Err(format!(
                        "host '{host}' is refused: an ssh destination cannot be empty, start with '-' or hold spaces, \
                         quotes or control characters"
                    ));
                }
            }
            "exec" => {
                if !exec_allowed() {
                    return Err(format!(
                        "kind \"exec\" is for tests only (it needs {ENV_TEST_EXEC}=1); use kind = \"ssh\""
                    ));
                }
            }
            other => return Err(format!("kind '{other}' is not ssh (slurm, pbs and batch come later)")),
        }
        for (k, v) in [("engine", &self.engine), ("work", &self.work)] {
            if !plain_arg(v) {
                return Err(format!(
                    "{k} '{v}' is refused: it cannot be empty, start with '-' or hold quotes or control characters"
                ));
            }
        }
        Ok(())
    }

    /// The budgets the site's runner gets (only those given).
    pub fn budgets(&self) -> Value {
        let mut v = json!({});
        for (k, x) in [
            ("max_runs", self.max_runs),
            ("cpu_threads", self.cpu_threads),
            ("mem_budget_mb", self.mem_budget_mb),
        ] {
            if let Some(n) = x {
                v[k] = json!(n);
            }
        }
        v
    }

    /// The entry as `rcfd site list` and MCP `list_sites` show it.
    pub fn listing(&self, name: &str) -> Value {
        json!({"name": name, "kind": self.kind, "host": self.host, "engine": self.engine, "work": self.work,
               "budgets": self.budgets(), "assistant": self.assistant, "confirmed": self.confirmed.is_some(),
               "accept_engines": self.accept_engines})
    }
}

/// Record the user's confirmation of site `name` in the file at `path`: `confirmed = "<what>"` and, when the site's
/// engine differs from this machine's, `engine` added to `accept_engines`. The other lines of the file, comments
/// included, stay as they are; the file is replaced atomically.
pub fn record_confirmation(path: &Path, name: &str, confirmed: &str, engine: Option<&str>) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let sites = load_from(path)?;
    let entry = sites
        .get(name)
        .ok_or_else(|| format!("{}: no [site.{name}]", path.display()))?;
    let mut accept = entry.accept_engines.clone();
    if let Some(e) = engine {
        if !accept.iter().any(|a| a == e) {
            accept.push(e.to_string());
        }
    }
    let header = format!("[site.{name}]");
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.trim() == header).ok_or_else(|| {
        format!(
            "{}: write [site.{name}] as a table header of its own to confirm it",
            path.display()
        )
    })?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| start + 1 + i);
    let is_key = |l: &str, k: &str| {
        l.trim_start()
            .strip_prefix(k)
            .is_some_and(|r| r.trim_start().starts_with('='))
    };
    let mut out: Vec<String> = lines[..=start].iter().map(|s| s.to_string()).collect();
    out.push(format!("confirmed = {}", toml_str(confirmed)));
    let list: Vec<String> = accept.iter().map(|a| toml_str(a)).collect();
    out.push(format!("accept_engines = [{}]", list.join(", ")));
    out.extend(
        lines[start + 1..end]
            .iter()
            .filter(|l| !is_key(l, "confirmed") && !is_key(l, "accept_engines"))
            .map(|s| s.to_string()),
    );
    out.extend(lines[end..].iter().map(|s| s.to_string()));
    let mut new = out.join("\n");
    new.push('\n');
    // the result must parse to the same sites with the two keys changed
    let check: SitesFile =
        toml::from_str(&new).map_err(|e| format!("{}: the edit would not parse: {e}", path.display()))?;
    let got = check.site.get(name).ok_or("the edit lost the site")?;
    if got.confirmed.as_deref() != Some(confirmed) || got.accept_engines != accept {
        return Err(format!("{}: the edit of [site.{name}] did not take", path.display()));
    }
    crate::records::atomic_write(path, new.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// A TOML basic string.
fn toml_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() => o.push_str(&format!("\\u{:04X}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_checked_and_a_confirmation_keeps_the_rest_of_the_file() {
        let d = std::env::temp_dir().join(format!("rcfd_sites_unit_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("sites.toml");
        let text = "# my sites\n[site.lab]\nkind = \"ssh\"\nhost = \"me@hpc\"   # the login node\nengine = \"/opt/rcfd/bin\"\n\
                    work = \"/scratch/me\"\nconfirmed = \"old\"\n\n[site.bad]\nkind = \"ssh\"\nhost = \"-oProxyCommand=x\"\n\
                    engine = \"/e\"\nwork = \"/w\"\n";
        std::fs::write(&f, text).unwrap();
        let sites = load_from(&f).unwrap();
        assert!(sites["lab"].check("lab").is_ok());
        let e = sites["bad"].check("bad").unwrap_err();
        assert!(e.contains("start with '-'"), "{e}");
        assert!(!sites["lab"].assistant);
        record_confirmation(&f, "lab", "SHA256:abc", Some("rcfd 1 (x)")).unwrap();
        let after = std::fs::read_to_string(&f).unwrap();
        assert!(
            after.starts_with("# my sites\n[site.lab]\nconfirmed = \"SHA256:abc\"\n"),
            "{after}"
        );
        assert!(after.contains("host = \"me@hpc\"   # the login node"), "{after}");
        assert_eq!(after.matches("confirmed").count(), 1, "{after}");
        let s = load_from(&f).unwrap();
        assert_eq!(s["lab"].accept_engines, vec!["rcfd 1 (x)".to_string()]);
        assert_eq!(s["bad"].host.as_deref(), Some("-oProxyCommand=x"));
        assert!(load_from(&d.join("none.toml")).unwrap().is_empty());
        std::fs::write(
            &f,
            "[site.x]\nkind = \"ssh\"\nhost = \"h\"\nengine = \"/e\"\nwork = \"/w\"\npassword = \"no\"\n",
        )
        .unwrap();
        assert!(load_from(&f).unwrap_err().contains("password"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
