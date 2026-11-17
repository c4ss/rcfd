//! ADR 0739 phase 1 (F-848): runs on a remote site, with the fake remote of CI: a site of kind `exec` whose agent
//! (`rcfd-runner site --stdio --work <dir>`) is spawned directly instead of through `ssh`, a second folder on this
//! machine being the remote. `RCFD_TEST_EXEC_SITES=1` and `RCFD_SITES_FILE` are given to each command, never set in this
//! process.
//!
//! - S1: a generator case and a mesh-file case on the fake site and locally: `solution.vtu`, `residuals.jsonl` and the
//!   field store byte-identical, `metrics.json` identical but for the solver's wall-clock time.
//! - S2: a second run on the same mesh sends no mesh byte (the agent's `received.jsonl`).
//! - S3: an upload cut half way resumes from the length the site holds; a corrupted object is refused and a run that
//!   needs it is not submitted.
//! - S4: the agent killed mid-run: the solver runs on, `site.jsonl` says `lost` then `connected`, and the local
//!   `events.jsonl`, `residuals.jsonl` and `log.txt` equal the site's byte for byte.
//! - S5: pause, resume and cancel of a remote run reach the site; the local status follows.
//! - S6: an agent answering `rcfd-site/2` is refused with a located message; one with another `git_rev` needs the yes
//!   (`site check --confirm`), after which `site.json` records it.
//! - S7: the site's runner killed and started again during a run: the run is adopted and finishes, and the mirror
//!   copies it.
//! - S8: a local run with sites configured: no `site` in `run.json`, no `site.json`, the agent never started (the
//!   site's work root is not made).
//! - S9: an unknown site, a host `-oProxyCommand=x`, an unconfirmed site: refused before any process starts and before
//!   any run folder exists.
//! - S12 (Unix): the work root is 0700 and the objects 0600.
//! - SSH: the same over `ssh localhost` (skipped unless `ssh -o BatchMode=yes localhost` works without a prompt and
//!   `RCFD_TEST_SSH` is not `0`); R6 (Unix, `RCFD_TEST_SSH=1`): the SSH session closed during a run, the run finishes.

use rcfd_runner::site::link::AgentLink;
use rcfd_runner::site::sites::SiteEntry;
use rcfd_runner::site::{blob_of, BlobRef, Site, SiteError};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

fn case(n: usize, iters: usize, store: bool) -> String {
    format!(
        "title = \"adr0739\"\n[mesh]\ngenerator = \"cavity_2d\"\nnx = {n}\nny = {n}\n[physics]\nnu = 0.01\n\
         [boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n\
         [solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = {iters}\ntolerance_u = 1e-14\n\
         tolerance_p = 1e-14\n{}",
        if store { "[output]\nstore_interval = 5\n" } else { "" }
    )
}

/// ASCII Fluent mesh of an `nx` x `ny` quad channel (as in rcfd-cli's tests/fluent.rs).
fn channel_msh(nx: usize, ny: usize, lx: f64, ly: f64) -> String {
    let node = |i: usize, j: usize| 1 + i + (nx + 1) * j;
    let cell = |i: usize, j: usize| 1 + i + nx * j;
    let nn = (nx + 1) * (ny + 1);
    let mut s = format!("(0 \"channel\")\n(1 \"rcfd test\")\n(2 2)\n(10 (0 1 {nn:x} 0))\n(10 (1 1 {nn:x} 1 2)(\n");
    for j in 0..=ny {
        for i in 0..=nx {
            s += &format!("{} {}\n", lx * i as f64 / nx as f64, ly * j as f64 / ny as f64);
        }
    }
    s += &format!("))\n(12 (0 1 {:x} 0))\n(12 (9 1 {:x} 1 3))\n", nx * ny, nx * ny);
    let mut zones: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut interior = Vec::new();
    for j in 0..ny {
        for i in 1..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i, j + 1),
                cell(i - 1, j),
                cell(i, j)
            ));
        }
    }
    for j in 1..ny {
        for i in 0..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i + 1, j),
                cell(i, j - 1),
                cell(i, j)
            ));
        }
    }
    zones.push((3, 2, interior));
    let mut walls = Vec::new();
    for i in 0..nx {
        walls.push(format!("{:x} {:x} {:x} 0", node(i, 0), node(i + 1, 0), cell(i, 0)));
        walls.push(format!(
            "{:x} {:x} {:x} 0",
            node(i, ny),
            node(i + 1, ny),
            cell(i, ny - 1)
        ));
    }
    zones.push((4, 3, walls));
    zones.push((
        5,
        10,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(0, j), node(0, j + 1), cell(0, j)))
            .collect(),
    ));
    zones.push((
        6,
        5,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(nx, j), node(nx, j + 1), cell(nx - 1, j)))
            .collect(),
    ));
    let total: usize = zones.iter().map(|z| z.2.len()).sum();
    s += &format!("(13 (0 1 {total:x} 0))\n");
    let mut first = 1;
    for (id, bc, faces) in &zones {
        let last = first + faces.len() - 1;
        s += &format!("(13 ({id:x} {first:x} {last:x} {bc:x} 2)(\n{}\n))\n", faces.join("\n"));
        first = last + 1;
    }
    s += "(45 (3 interior interior-3)())\n(45 (4 wall walls)())\n(45 (5 velocity-inlet inlet)())\n\
          (45 (6 pressure-outlet outlet)())\n(45 (9 fluid fluid)())\n";
    s
}

const MESH_CASE: &str = "title = \"adr0739 channel\"\n[mesh]\nsource = \"file\"\nfile = \"../meshes/chan.msh\"\n\
    [physics]\nnu = 0.1\n[boundaries.inlet]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n\
    [boundaries.outlet]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n[boundaries.walls]\nvelocity = \"fixed\"\n\
    [solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = 40\ntolerance_u = 1e-14\n\
    tolerance_p = 1e-14\n";

struct Env {
    root: PathBuf,
    project: PathBuf,
    work: PathBuf,
    sites: PathBuf,
    extra_env: Vec<(String, String)>,
}

fn engine_dir() -> PathBuf {
    Path::new(RUNNER).parent().unwrap().to_path_buf()
}

impl Env {
    fn new(name: &str) -> Env {
        let rcfd = engine_dir().join(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
        assert!(rcfd.is_file(), "{} missing (cargo build -p rcfd-cli)", rcfd.display());
        let root = std::env::temp_dir().join(format!("rcfd_sites_adr0739_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("proj");
        for d in ["cases", "meshes", ".rcfd"] {
            std::fs::create_dir_all(project.join(d)).unwrap();
        }
        let config = json!({"max_runs": 2, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 100,
            "adopt_probe_ms": 500, "cancel_grace_ms": 10_000});
        std::fs::write(project.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        std::fs::write(
            project.join("rcfd-project.json"),
            json!({"name": "adr0739 test", "created": "now"}).to_string(),
        )
        .unwrap();
        let work = root.join("site");
        let sites = root.join("sites.toml");
        let toml_path = |p: &Path| p.display().to_string().replace('\\', "\\\\");
        let text = format!(
            "# the fake remote of the tests\n[site.fake]\nkind = \"exec\"\nengine = \"{e}\"\nwork = \"{w}\"\n\
             max_runs = 2\ncpu_threads = 8\nmem_budget_mb = 100000\nconfirmed = \"exec\"\n\n\
             [site.unconfirmed]\nkind = \"exec\"\nengine = \"{e}\"\nwork = \"{w}\"\n\n\
             [site.evil]\nkind = \"ssh\"\nhost = \"-oProxyCommand=x\"\nengine = \"/e\"\nwork = \"/w\"\nconfirmed = \"x\"\n",
            e = toml_path(&engine_dir()),
            w = toml_path(&work)
        );
        std::fs::write(&sites, text).unwrap();
        Env {
            root,
            project,
            work,
            sites,
            extra_env: Vec::new(),
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(RUNNER);
        c.env("RCFD_SITES_FILE", &self.sites).env("RCFD_TEST_EXEC_SITES", "1");
        for (k, v) in &self.extra_env {
            c.env(k, v);
        }
        c
    }

    fn runner(&self, args: &[&str]) -> Output {
        let mut c = self.cmd();
        c.args(args).arg("--project").arg(&self.project).arg("--quiet");
        out_by_file(c, &self.root)
    }

    fn write_case(&self, name: &str, text: &str) -> String {
        std::fs::write(self.project.join("cases").join(name), text).unwrap();
        self.project.join("cases").join(name).display().to_string()
    }

    fn start(&self, case: &str, extra: &[&str]) -> Result<String, String> {
        let mut args = vec!["start", case];
        args.extend_from_slice(extra);
        let o = self.runner(&args);
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).to_string())
        }
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.project.join("runs").join(id)
    }

    fn status(&self, id: &str) -> Value {
        std::fs::read(self.dir(id).join("status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null)
    }

    fn wait_state(&self, id: &str, states: &[&str], secs: u64) -> Value {
        let t0 = Instant::now();
        loop {
            let st = self.status(id);
            if states.iter().any(|s| st["state"] == *s) {
                return st;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(secs),
                "run {id} not {states:?} after {secs} s: {st}\nsite.jsonl: {}\nrunner.log: {}",
                std::fs::read_to_string(self.dir(id).join("site.jsonl")).unwrap_or_default(),
                std::fs::read_to_string(self.project.join(".rcfd/runner.log")).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn wait_end(&self, id: &str, secs: u64) -> Value {
        self.wait_state(id, &["finished", "failed", "cancelled"], secs)
    }

    /// The site's mirror of the project (the one folder below `projects/`).
    fn mirror(&self) -> PathBuf {
        let mut v: Vec<PathBuf> = std::fs::read_dir(self.work.join("projects"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(v.len(), 1, "{v:?}");
        v.pop().unwrap()
    }

    fn site_run(&self, id: &str) -> PathBuf {
        self.mirror().join("runs").join(id)
    }

    fn site_events(&self, id: &str) -> Vec<Value> {
        std::fs::read_to_string(self.dir(id).join("site.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    fn entry(&self) -> SiteEntry {
        SiteEntry {
            kind: "exec".into(),
            host: None,
            engine: engine_dir().display().to_string(),
            work: self.work.display().to_string(),
            max_runs: None,
            cpu_threads: None,
            mem_budget_mb: None,
            assistant: false,
            confirmed: Some("exec".into()),
            accept_engines: vec![],
        }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.runner(&["shutdown"]);
        if let Ok(rd) = std::fs::read_dir(self.work.join("projects")) {
            for m in rd.flatten() {
                let mut c = self.cmd();
                c.args(["shutdown", "--project"]).arg(m.path());
                let _ = out_by_file(c, &self.root);
            }
        }
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

/// The output of `c` through files, not pipes: on Windows a runner the command starts inherits its handles (ADR 0146),
/// so a pipe would stay open until that runner exits.
fn out_by_file(mut c: Command, dir: &Path) -> Output {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    std::fs::create_dir_all(dir).unwrap();
    let (o, e) = (dir.join(format!("out-{n}.txt")), dir.join(format!("err-{n}.txt")));
    let status = c
        .stdin(std::process::Stdio::null())
        .stdout(std::fs::File::create(&o).unwrap())
        .stderr(std::fs::File::create(&e).unwrap())
        .status()
        .unwrap();
    Output {
        status,
        stdout: std::fs::read(&o).unwrap_or_default(),
        stderr: std::fs::read(&e).unwrap_or_default(),
    }
}

fn bytes(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `metrics.json` without the solver's wall-clock members.
fn metrics(p: &Path) -> Value {
    let mut v: Value = serde_json::from_slice(&bytes(p)).unwrap();
    if let Some(f) = v["solver_finish"].as_object_mut() {
        f.retain(|k, _| !(k.contains("elapsed") || k.contains("wall") || k == "t"));
    }
    v
}

/// S1 and S2: a generator case and a mesh-file case on the fake site and here; a second run on the same mesh.
#[test]
fn s1_s2_results_on_the_site_equal_local_ones_and_a_mesh_goes_once() {
    let env = Env::new("s1");
    let cavity = env.write_case("cavity.toml", &case(10, 30, true));
    std::fs::write(env.project.join("meshes/chan.msh"), channel_msh(12, 6, 2.0, 1.0)).unwrap();
    let chan = env.write_case("chan.toml", MESH_CASE);
    for (label, c) in [("generator", &cavity), ("mesh file", &chan)] {
        let local = env.start(c, &[]).unwrap();
        let remote = env.start(c, &["--site", "fake"]).unwrap();
        let (a, b) = (env.wait_end(&local, 120), env.wait_end(&remote, 120));
        assert_eq!(a["state"], "finished", "{label}: {a}");
        assert_eq!(b["state"], "finished", "{label}: {b}");
        let (l, r) = (env.dir(&local), env.dir(&remote));
        let rec: Value = serde_json::from_slice(&bytes(&r.join("run.json"))).unwrap();
        assert_eq!(rec["site"], "fake", "{rec}");
        for f in ["output/solution.vtu", "residuals.jsonl"] {
            assert!(bytes(&l.join(f)) == bytes(&r.join(f)), "{label}: {f} differs");
        }
        assert_eq!(
            metrics(&l.join("metrics.json")),
            metrics(&r.join("metrics.json")),
            "{label}"
        );
        if label == "generator" {
            let store = |d: &Path| bytes(&d.join("fields/store.rts"));
            assert!(store(&l) == store(&r), "{label}: fields/store.rts differs");
        }
        // the mirror's own copies equal the downloads
        let site = env.site_run(&remote);
        assert!(bytes(&site.join("output/solution.vtu")) == bytes(&r.join("output/solution.vtu")));
        assert!(r.join("launch.json").is_file(), "the site's launch record is copied");
        let sj: Value = serde_json::from_slice(&bytes(&r.join("site.json"))).unwrap();
        assert_eq!(sj["done"], true, "{sj}");
        assert_eq!(sj["protocol"], "rcfd-site/1");
        let kinds: Vec<String> = env
            .site_events(&remote)
            .iter()
            .map(|e| e["kind"].as_str().unwrap_or("").to_string())
            .collect();
        for k in ["uploaded", "submitted", "downloaded"] {
            assert!(kinds.iter().any(|x| x == k), "{label}: {kinds:?}");
        }
        eprintln!("S1 {label}: local {local} and remote {remote} equal; site.jsonl {kinds:?}");
    }
    // S2: the mesh again, from another case on it
    let mesh = blob_of(&bytes(&env.project.join("meshes/chan.msh")));
    let received = || -> Vec<Value> {
        std::fs::read_to_string(env.work.join("received.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    };
    let before: Vec<Value> = received().into_iter().filter(|v| v["object"] == mesh.name()).collect();
    assert!(!before.is_empty(), "the first run sent the mesh");
    let chan2 = env.write_case(
        "chan2.toml",
        &MESH_CASE.replace("max_iterations = 40", "max_iterations = 20"),
    );
    let id = env.start(&chan2, &["--site", "fake"]).unwrap();
    assert_eq!(env.wait_end(&id, 120)["state"], "finished");
    let after: Vec<Value> = received().into_iter().filter(|v| v["object"] == mesh.name()).collect();
    assert_eq!(after.len(), before.len(), "no mesh byte the second time");
    let up = env
        .site_events(&id)
        .into_iter()
        .find(|e| e["kind"] == "uploaded")
        .unwrap();
    let snap = std::fs::metadata(env.dir(&id).join("case.snapshot")).unwrap().len();
    assert_eq!(up["bytes"], json!(snap), "only the case was sent: {up}");
    eprintln!("S2: second run sent {up}");
}

/// S3: a cut upload resumes; a corrupted object is refused and nothing is submitted.
#[test]
fn s3_an_upload_resumes_from_the_sites_length_and_a_corrupt_object_is_refused() {
    let env = Env::new("s3");
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 31 % 253) as u8).collect();
    let blob = blob_of(&data);
    let half = data.len() / 2;
    {
        let mut a = AgentLink::open("fake", &env.entry()).unwrap();
        assert_eq!(a.missing(std::slice::from_ref(&blob)).unwrap()[0].from, 0);
        let mut at = 0u64;
        while (at as usize) < half {
            let end = (at as usize + (1 << 20)).min(half);
            at = a.put(&blob, at, &data[at as usize..end]).unwrap();
        }
        // the agent killed mid-upload
        let pid = a.pid();
        assert!(rcfd_runner::sys::kill(pid), "kill agent {pid}");
    }
    let mut b = AgentLink::open("fake", &env.entry()).unwrap();
    let m = b.missing(std::slice::from_ref(&blob)).unwrap();
    assert_eq!(m[0].from, half as u64, "resumes from the length the site holds");
    let mut at = m[0].from;
    while at < blob.bytes {
        let end = (at as usize + (1 << 20)).min(data.len());
        at = b.put(&blob, at, &data[at as usize..end]).unwrap();
    }
    assert!(b.missing(std::slice::from_ref(&blob)).unwrap().is_empty());
    let obj = env.work.join("objects").join(blob.name());
    assert_eq!(blob_of(&bytes(&obj)), blob);
    // a corrupted piece: the last piece's bytes do not hash to the name
    let good = b"the real bytes".to_vec();
    let claimed = blob_of(&good);
    let r = b.put(&claimed, 0, b"the fake bytes");
    match r {
        Err(SiteError::Refused(m)) => assert!(m.contains("dropped"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(!env.work.join("objects").join(claimed.name()).exists());
    assert!(!env
        .work
        .join("objects")
        .join(format!("tmp-{}", claimed.name()))
        .exists());
    let run = rcfd_runner::site::RemoteRun {
        project: rcfd_runner::site::ProjectRef {
            key: "p-0".into(),
            name: "p".into(),
            manifest: None,
        },
        id: "r-20260101T000000-abcd".into(),
        record: json!({}),
        status: String::new(),
        events: String::new(),
        snapshot: claimed.clone(),
        files: vec![],
        up: 0,
        parent: None,
        budgets: json!({}),
    };
    match b.submit(&run) {
        Err(SiteError::Refused(m)) => assert!(m.contains("not on the site"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(!env.work.join("projects/p-0/runs/r-20260101T000000-abcd").exists());
    eprintln!(
        "S3: resumed at {half} of {}; corrupt object refused, nothing submitted",
        blob.bytes
    );
    let _ = BlobRef::of_filehash("fnv64:0000000000000000", 0);
}

/// S4: the agent killed during a run.
#[test]
fn s4_a_lost_agent_is_reconnected_and_the_copies_are_exact() {
    let env = Env::new("s4");
    let c = env.write_case("long.toml", &case(40, 6000, false));
    let id = env.start(&c, &["--site", "fake"]).unwrap();
    env.wait_state(&id, &["running"], 60);
    // wait for some residual lines here, then kill the agent the runner holds
    let t0 = Instant::now();
    while std::fs::metadata(env.dir(&id).join("residuals.jsonl")).map_or(0, |m| m.len()) < 200 {
        assert!(t0.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(50));
    }
    let pid = env
        .site_events(&id)
        .iter()
        .rev()
        .find(|e| e["kind"] == "connected")
        .and_then(|e| e["pid"].as_u64())
        .map(|p| p as u32);
    let pid = match pid {
        Some(p) => p,
        None => panic!("no connected line: {:?}", env.site_events(&id)),
    };
    assert!(rcfd_runner::sys::kill(pid), "kill agent {pid}");
    let st = env.wait_end(&id, 300);
    assert_eq!(st["state"], "finished", "{st}");
    let kinds: Vec<String> = env
        .site_events(&id)
        .iter()
        .map(|e| e["kind"].as_str().unwrap_or("").to_string())
        .collect();
    let lost = kinds.iter().position(|k| k == "lost").expect("lost");
    assert!(kinds[lost..].iter().any(|k| k == "connected"), "{kinds:?}");
    let site = env.site_run(&id);
    for f in ["events.jsonl", "residuals.jsonl", "log.txt"] {
        let (a, b) = (bytes(&env.dir(&id).join(f)), bytes(&site.join(f)));
        assert!(a == b, "{f}: local {} bytes, site {} bytes", a.len(), b.len());
    }
    let res = String::from_utf8(bytes(&env.dir(&id).join("residuals.jsonl"))).unwrap();
    let steps: Vec<u64> = res
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["step"].as_u64().unwrap())
        .collect();
    assert!(steps.windows(2).all(|w| w[1] == w[0] + 1), "no line twice");
    eprintln!(
        "S4: agent {pid} killed; {kinds:?}; {} residual lines equal",
        steps.len()
    );
}

/// S5: pause, resume and cancel across the link.
#[test]
fn s5_pause_resume_and_cancel_reach_the_site() {
    let env = Env::new("s5");
    let c = env.write_case("long.toml", &case(40, 30_000, false));
    let id = env.start(&c, &["--site", "fake"]).unwrap();
    env.wait_state(&id, &["running"], 60);
    let site_state = |id: &str| -> Value {
        std::fs::read(env.site_run(id).join("status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null)
    };
    let timed = |what: &str, want: &str| {
        let o = env.runner(&[what, &id]);
        assert!(o.status.success(), "{what}: {}", String::from_utf8_lossy(&o.stderr));
        let t0 = Instant::now();
        while site_state(&id)["state"] != want {
            assert!(
                t0.elapsed() < Duration::from_secs(30),
                "{what}: site {}",
                site_state(&id)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let at_site = t0.elapsed();
        env.wait_state(&id, &[want], 30);
        eprintln!(
            "S5: {what}: the site's run {want} after {at_site:?}, the local status after {:?}",
            t0.elapsed()
        );
    };
    timed("pause", "paused");
    assert!(env.dir(&id).join("pause.ack").exists(), "pause.ack mirrored");
    timed("resume", "running");
    timed("cancel", "cancelled");
    let kinds: Vec<Value> = env
        .site_events(&id)
        .into_iter()
        .filter(|e| e["kind"] == "request_sent")
        .collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    assert!(!env.dir(&id).join("cancel").exists() && !env.dir(&id).join("pause").exists());
}

/// S6: version negotiation (Q10 (a)).
#[test]
fn s6_another_protocol_is_refused_and_another_engine_needs_the_yes() {
    let mut env = Env::new("s6");
    let c = env.write_case("cavity.toml", &case(8, 10, false));
    env.extra_env = vec![("RCFD_SITE_TEST_PROTOCOL".into(), "rcfd-site/2".into())];
    let e = env.start(&c, &["--site", "fake"]).unwrap_err();
    assert!(e.contains("speaks rcfd-site/2") && e.contains("[site.fake]"), "{e}");
    assert!(!env.project.join("runs").exists(), "nothing written");
    env.extra_env = vec![("RCFD_SITE_TEST_GIT_REV".into(), "other-rev".into())];
    let e = env.start(&c, &["--site", "fake"]).unwrap_err();
    assert!(e.contains("other-rev") && e.contains("--confirm"), "{e}");
    let o = out_by_file(
        {
            let mut c = env.cmd();
            c.args(["site", "check", "fake"]);
            c
        },
        &env.root,
    );
    assert_eq!(
        o.status.code(),
        Some(1),
        "check without the yes: {}",
        String::from_utf8_lossy(&o.stdout)
    );
    let o = out_by_file(
        {
            let mut c = env.cmd();
            c.args(["site", "check", "fake", "--confirm"]);
            c
        },
        &env.root,
    );
    assert!(
        o.status.success(),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    let text = std::fs::read_to_string(&env.sites).unwrap();
    assert!(
        text.contains("accept_engines = [\"") && text.contains("(other-rev)"),
        "{text}"
    );
    assert!(
        text.starts_with("# the fake remote of the tests\n"),
        "the file's comments stay"
    );
    let id = env.start(&c, &["--site", "fake"]).unwrap();
    assert_eq!(env.wait_end(&id, 120)["state"], "finished");
    let sj: Value = serde_json::from_slice(&bytes(&env.dir(&id).join("site.json"))).unwrap();
    assert_eq!(sj["engine_accepted"], true, "{sj}");
    assert_eq!(sj["engine"]["git_rev"], "other-rev", "{sj}");
    eprintln!(
        "S6: refused rcfd-site/2; other-rev accepted after the yes: {}",
        sj["engine"]
    );
}

/// S7: the site's runner killed and started again during a run.
#[test]
fn s7_the_sites_runner_restarted_adopts_the_run() {
    let env = Env::new("s7");
    let c = env.write_case("long.toml", &case(40, 8000, false));
    let id = env.start(&c, &["--site", "fake"]).unwrap();
    env.wait_state(&id, &["running"], 60);
    let mirror = env.mirror();
    let info: Value = serde_json::from_slice(&bytes(&mirror.join(".rcfd/runner.json"))).unwrap();
    let pid = info["pid"].as_u64().unwrap() as u32;
    assert!(rcfd_runner::sys::kill(pid), "kill the site's runner {pid}");
    let t0 = Instant::now();
    while rcfd_runner::sys::lock_held(&mirror.join(".rcfd/runner.lock")) {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    let o = out_by_file(
        {
            let mut c = env.cmd();
            c.args(["ensure", "--quiet", "--project"]).arg(&mirror);
            c
        },
        &env.root,
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let st = env.wait_end(&id, 300);
    assert_eq!(st["state"], "finished", "{st}");
    let ev = String::from_utf8(bytes(&env.dir(&id).join("events.jsonl"))).unwrap();
    assert!(ev.contains("\"adopted\""), "the restarted site runner adopted the run");
    assert!(env.dir(&id).join("output/solution.vtu").is_file());
    eprintln!("S7: site runner {pid} killed and started again; the run was adopted and finished");
}

/// S8 and S9: a local run with sites configured; the refusals before anything starts.
#[test]
fn s8_s9_local_runs_are_untouched_and_bad_sites_are_refused_before_any_process() {
    let env = Env::new("s8");
    let c = env.write_case("cavity.toml", &case(8, 10, false));
    let id = env.start(&c, &[]).unwrap();
    assert_eq!(env.wait_end(&id, 120)["state"], "finished");
    let rec: Value = serde_json::from_slice(&bytes(&env.dir(&id).join("run.json"))).unwrap();
    let keys: Vec<&str> = rec.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "case",
            "case_dir",
            "case_snapshot_hash",
            "case_source",
            "created",
            "id",
            "settings",
            "solver_version"
        ]
    );
    for f in ["site.json", "site.jsonl"] {
        assert!(!env.dir(&id).join(f).exists(), "{f}");
    }
    assert!(!env.work.exists(), "the agent never started: no work root");
    let runs_before = std::fs::read_dir(env.project.join("runs")).unwrap().count();
    for (site, want) in [
        ("nowhere", "not configured"),
        ("evil", "start with '-'"),
        ("unconfirmed", "not confirmed"),
        ("../x", "not a site name"),
    ] {
        let e = env.start(&c, &["--site", site]).unwrap_err();
        assert!(e.contains(want), "{site}: {e}");
    }
    assert_eq!(
        std::fs::read_dir(env.project.join("runs")).unwrap().count(),
        runs_before
    );
    assert!(!env.work.exists(), "no agent started for a refused site");
    // exec is for tests only
    let o = Command::new(RUNNER)
        .env("RCFD_SITES_FILE", &env.sites)
        .env_remove("RCFD_TEST_EXEC_SITES")
        .args(["start", &c, "--site", "fake", "--no-runner", "--project"])
        .arg(&env.project)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("for tests only"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    eprintln!("S8: local run {id} has keys {keys:?}; S9: four refusals, no process, no folder");
}

/// S12: the modes of the work root and the objects (Unix).
#[cfg(unix)]
#[test]
fn s12_the_work_root_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new("s12");
    let mut a = AgentLink::open("fake", &env.entry()).unwrap();
    let b = blob_of(b"abc");
    a.put(&b, 0, b"abc").unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&env.work), 0o700);
    assert_eq!(mode(&env.work.join("objects").join(b.name())), 0o600);
}

/// Whether `ssh localhost` answers without a prompt (BatchMode), unless RCFD_TEST_SSH=0.
fn ssh_localhost() -> bool {
    if std::env::var("RCFD_TEST_SSH").as_deref() == Ok("0") {
        return false;
    }
    Command::new("ssh")
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "localhost", "exit"])
        .stdin(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The same run over `ssh localhost` (skipped when ssh localhost is not set up).
#[test]
fn ssh_localhost_runs_a_case_when_set_up() {
    if !ssh_localhost() {
        eprintln!("SSH: skipped (ssh -o BatchMode=yes localhost does not answer without a prompt)");
        return;
    }
    let env = Env::new("ssh");
    let text = std::fs::read_to_string(&env.sites).unwrap();
    let toml_path = |p: &Path| p.display().to_string().replace('\\', "\\\\");
    std::fs::write(
        &env.sites,
        format!(
            "{text}\n[site.local]\nkind = \"ssh\"\nhost = \"localhost\"\nengine = \"{}\"\nwork = \"{}\"\nconfirmed = \"t\"\n",
            toml_path(&engine_dir()),
            toml_path(&env.work)
        ),
    )
    .unwrap();
    let c = env.write_case("long.toml", &case(40, 8000, false));
    let id = env.start(&c, &["--site", "local"]).unwrap();
    env.wait_state(&id, &["running"], 120);
    // R6: the SSH session closed during the run (its ssh process ended); the site's runner and solver go on
    let pid = env
        .site_events(&id)
        .iter()
        .rev()
        .find(|e| e["kind"] == "connected")
        .and_then(|e| e["pid"].as_u64())
        .expect("connected") as u32;
    assert!(rcfd_runner::sys::kill(pid), "kill ssh {pid}");
    assert_eq!(env.wait_end(&id, 600)["state"], "finished");
    assert!(env.dir(&id).join("output/solution.vtu").is_file());
    eprintln!("SSH/R6: run {id} finished over ssh localhost after its session {pid} was closed");
}
