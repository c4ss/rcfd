//! ADR 0758 (F-855): the body of F2 (ADR 0669) with its record, the classes of a refused probe call and the verdict.
//! Included with `#[path]` by `edit_lock_f751.rs` (F2, FS) and by `edit_lock_f855_stall.rs` (the forced controls).
#![allow(dead_code)]

use rcfd_project::atomic::{edit_claim_path, edit_lock_path, replace_if_hash, EDIT_LOCK_WAIT};
use rcfd_project::records::hash_bytes;
use rcfd_project::ProjectError;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// The most probe calls of class S in one run (ADR 0758: the most refused calls a gate has shown).
pub const STALL_LIMIT: usize = 4;
/// How late into a probe's wait the racer's hold may begin, and how much younger than the wait its lock file may be.
pub const SLACK_S: f64 = 0.1;
/// A gap between two wake-ups of the ticker that is recorded.
pub const TICK_GAP_S: f64 = 0.25;

/// One call of the racer: start and end in seconds since the run began, and whether it was accepted.
pub type RacerCall = (f64, f64, bool);

/// A probe call that ended in `LockBusy`, with the snapshot taken at once after it.
#[derive(Debug, Clone, Default)]
pub struct Busy {
    pub call: usize,
    pub p0: f64,
    pub p1: f64,
    /// A claim file existed.
    pub claim: bool,
    /// The writer named in the edit lock file.
    pub lock_who: Option<String>,
    /// The age of the edit lock file (`None`: no file).
    pub lock_age_s: Option<f64>,
    /// The age of the journal's lock (`None`: no file).
    pub journal_lock_age_s: Option<f64>,
    /// The journal's lock was modified before the edit lock file.
    pub journal_lock_before_lock: bool,
}

/// Counts over the journal at the end of the run.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    pub lines: usize,
    pub no_seq: usize,
    pub broken: usize,
    pub remove_failed: usize,
    pub stale_broken: usize,
}

/// The record of one run.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub calls: usize,
    pub conflicts: usize,
    /// Probe calls that ended in neither `Conflict` nor `LockBusy`: index and result.
    pub other: Vec<(usize, String)>,
    pub busy: Vec<Busy>,
    pub racer: Vec<RacerCall>,
    pub racer_errs: Vec<String>,
    pub journal: Journal,
    /// Files left in `.rcfd/locks`.
    pub left: Vec<String>,
    /// Gaps of the ticker: start and length in seconds.
    pub gaps: Vec<(f64, f64)>,
    pub max_probe_s: f64,
    pub total_s: f64,
}

/// What a run must satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// ADR 0669: every probe call is `Conflict`, the racer has no error.
    Adr0669,
    /// ADR 0758: as ADR 0669, but a `LockBusy` of class S is accepted, at most [`STALL_LIMIT`] times, with a clean journal.
    Adr0758,
}

/// The class of a probe call that ended in `LockBusy` (ADR 0758, the table of the amendment).
pub fn class(b: &Busy, racer: &[RacerCall]) -> char {
    let wait = b.p1 - b.p0;
    if wait < EDIT_LOCK_WAIT.as_secs_f64() {
        'E'
    } else if b.journal_lock_before_lock || (b.lock_age_s.is_none() && b.journal_lock_age_s.is_some_and(|a| a >= 1.0)) {
        'J'
    } else if b.claim {
        'C'
    } else if b.lock_who.as_deref() == Some("racer")
        && b.lock_age_s.is_some_and(|a| a >= wait - SLACK_S)
        && spanning(b, racer).is_some_and(|c| c.2)
    {
        'S'
    } else {
        'V'
    }
}

/// The racer call that began at `p0 + SLACK_S` at the latest and ended after `p1`.
pub fn spanning(b: &Busy, racer: &[RacerCall]) -> Option<RacerCall> {
    racer.iter().copied().find(|&(s, e, _)| s <= b.p0 + SLACK_S && e > b.p1)
}

/// The verdict of a run by `rule`: the number of accepted calls of class S, or what is wrong.
pub fn verdict(rec: &Record, rule: Rule) -> Result<usize, Vec<String>> {
    let mut bad = Vec::new();
    if !rec.racer_errs.is_empty() {
        bad.push(format!(
            "the racer has {} errors (first {:?})",
            rec.racer_errs.len(),
            rec.racer_errs.first()
        ));
    }
    if rule == Rule::Adr0669 {
        if rec.conflicts != rec.calls {
            bad.push(format!("{} of {} probe calls are conflicts", rec.conflicts, rec.calls));
        }
        return if bad.is_empty() { Ok(0) } else { Err(bad) };
    }
    if !rec.other.is_empty() {
        bad.push(format!(
            "{} probe calls ended in neither Conflict nor LockBusy (first {:?})",
            rec.other.len(),
            rec.other.first()
        ));
    }
    let classes: String = rec.busy.iter().map(|b| class(b, &rec.racer)).collect();
    let stalled = classes.chars().filter(|&c| c == 'S').count();
    if stalled != classes.len() {
        bad.push(format!("LockBusy calls of a failing class: {classes}"));
    }
    if stalled > STALL_LIMIT {
        bad.push(format!("{stalled} calls of class S, more than the limit {STALL_LIMIT}"));
    }
    let j = &rec.journal;
    if stalled > 0 && j.no_seq + j.broken + j.remove_failed + j.stale_broken > 0 {
        bad.push(format!("a call of class S with a journal that is not clean: {j:?}"));
    }
    if rec.conflicts + rec.busy.len() + rec.other.len() != rec.calls {
        bad.push("the record does not add up to the number of calls".to_string());
    }
    if bad.is_empty() {
        Ok(stalled)
    } else {
        Err(bad)
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn age_s(t: Option<SystemTime>) -> Option<f64> {
    t.map(|t| t.elapsed().map(|a| a.as_secs_f64()).unwrap_or(0.0))
}

fn journal_counts(root: &Path) -> Journal {
    let mut j = Journal::default();
    let text = std::fs::read_to_string(root.join(".rcfd/edits.jsonl")).unwrap_or_default();
    for v in text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
    {
        j.lines += 1;
        j.no_seq += usize::from(v.get("seq").is_none());
        j.broken += usize::from(v.get("journal_lock_broken_s").is_some());
        j.remove_failed += usize::from(v["kind"] == "edit_lock_remove_failed");
        j.stale_broken += usize::from(v["kind"] == "stale_lock_broken");
    }
    j
}

/// F2 of ADR 0669 in the project at `root`: a cooperating racer replaces `cases/c.txt` continuously while the calling
/// thread makes `calls` probe calls with a wrong base. `racer_init` and `probe_init` run once on the racer's and on the
/// probe's thread before their loops (the forced controls set their thread-local hooks there).
pub fn run(root: &Path, calls: usize, racer_init: impl FnOnce() + Send + 'static, probe_init: impl FnOnce()) -> Record {
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let lock = edit_lock_path(root, "cases/c.txt");
    let claim = edit_claim_path(root, "cases/c.txt");
    let journal_lock = root.join(rcfd_project::journal::LOCK);
    let t0 = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let racer = {
        let (root, stop) = (root.to_path_buf(), stop.clone());
        std::thread::spawn(move || {
            racer_init();
            let (mut ok, mut errs, mut made) = (0usize, Vec::new(), Vec::<RacerCall>::new());
            while !stop.load(Ordering::SeqCst) {
                let cur = std::fs::read(root.join("cases/c.txt")).unwrap();
                let mut next = cur.clone();
                next.extend_from_slice(format!("r {ok}\n").as_bytes());
                let s = t0.elapsed().as_secs_f64();
                let r = replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(&cur)), &next, "racer");
                made.push((s, t0.elapsed().as_secs_f64(), r.is_ok()));
                match r {
                    Ok(_) => ok += 1,
                    Err(e) => errs.push(e.to_string()),
                }
            }
            (errs, made)
        })
    };
    let ticker = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut gaps = Vec::new();
            let mut last = t0.elapsed().as_secs_f64();
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
                let now = t0.elapsed().as_secs_f64();
                if now - last >= TICK_GAP_S {
                    gaps.push((last, now - last));
                }
                last = now;
            }
            gaps
        })
    };
    probe_init();
    let mut rec = Record {
        calls,
        ..Record::default()
    };
    for call in 0..calls {
        let p0 = t0.elapsed().as_secs_f64();
        let r = replace_if_hash(root, "cases/c.txt", Some("fnv64:0000000000000000"), b"p", "probe");
        let p1 = t0.elapsed().as_secs_f64();
        rec.max_probe_s = rec.max_probe_s.max(p1 - p0);
        match r {
            Err(ProjectError::Conflict { .. }) => rec.conflicts += 1,
            Err(ProjectError::LockBusy { .. }) => {
                // the order is registered: the claim, the edit lock, then the journal's lock
                let claim = claim.exists();
                let lock_who = std::fs::read_to_string(&lock)
                    .ok()
                    .and_then(|t| t.split_whitespace().nth(1).map(str::to_string));
                let lock_m = modified(&lock);
                let journal_m = modified(&journal_lock);
                rec.busy.push(Busy {
                    call,
                    p0,
                    p1,
                    claim,
                    lock_who,
                    lock_age_s: age_s(lock_m),
                    journal_lock_age_s: age_s(journal_m),
                    journal_lock_before_lock: matches!((journal_m, lock_m), (Some(j), Some(l)) if j < l),
                });
            }
            r => rec.other.push((call, format!("{r:?}"))),
        }
    }
    stop.store(true, Ordering::SeqCst);
    let (errs, made) = racer.join().unwrap();
    rec.racer_errs = errs;
    rec.racer = made;
    rec.gaps = ticker.join().unwrap();
    rec.journal = journal_counts(root);
    rec.left = std::fs::read_dir(root.join(".rcfd/locks"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    rec.left.sort();
    rec.total_s = t0.elapsed().as_secs_f64();
    rec
}

/// The printed lines of a record: one for every `LockBusy` call and one for the run.
pub fn lines(rec: &Record) -> Vec<String> {
    let mut out: Vec<String> = rec
        .busy
        .iter()
        .map(|b| {
            format!(
                "F2 busy call={} class={} wait_s={:.3} p0={:.3} claim={} lock_who={:?} lock_age_s={:.3?} \
                 journal_lock_age_s={:.3?} journal_lock_before_lock={} racer_calls_ended_in_wait={} spanning_racer_call={:.3?}",
                b.call,
                class(b, &rec.racer),
                b.p1 - b.p0,
                b.p0,
                b.claim,
                b.lock_who,
                b.lock_age_s,
                b.journal_lock_age_s,
                b.journal_lock_before_lock,
                rec.racer.iter().filter(|c| c.1 > b.p0 && c.1 <= b.p1).count(),
                spanning(b, &rec.racer)
            )
        })
        .collect();
    let mut longest: Vec<(f64, f64)> = rec.racer.iter().map(|c| (c.1 - c.0, c.0)).collect();
    longest.sort_by(|a, b| b.0.total_cmp(&a.0));
    longest.truncate(3);
    let j = &rec.journal;
    out.push(format!(
        "F2 run calls={} conflicts={} busy_classes={:?} other={} first_other={:?} racer_ok={} racer_errs={} \
         first_err={:?} lines={} no_seq={} journal_lock_broken={} remove_failed={} stale_broken={} left={:?} \
         max_probe_s={:.3} longest_racer_calls_s_at={:.3?} ticker_gaps_at_s={:.2?} total_s={:.1}",
        rec.calls,
        rec.conflicts,
        rec.busy.iter().map(|b| class(b, &rec.racer)).collect::<String>(),
        rec.other.len(),
        rec.other.first(),
        rec.racer.iter().filter(|c| c.2).count(),
        rec.racer_errs.len(),
        rec.racer_errs.first(),
        j.lines,
        j.no_seq,
        j.broken,
        j.remove_failed,
        j.stale_broken,
        rec.left,
        rec.max_probe_s,
        longest,
        rec.gaps.iter().take(12).collect::<Vec<_>>(),
        rec.total_s
    ));
    out
}
