//! ADR 0152 (job 48R) DV6 (a): the record of the start plans of the 22 unedited cases and of the saved control, made on
//! 6867e40 before any code change of job 48R (`tests/fixtures/plans_6867e40.txt`). The recorder is ignored and writes
//! only to a new file named by `RCFD_DV6_RECORD`; it was run once, on 6867e40 with only these test files added. Never
//! record it again: the comparison is DV6 in `saved_store_r.rs`.

mod common;
mod dv;
mod g4;

#[test]
#[ignore = "ADR 0152 DV6 (a): the recorder, run once on 6867e40; never record again"]
fn dv6_record_on_6867e40() {
    let path = std::env::var_os("RCFD_DV6_RECORD").expect("RCFD_DV6_RECORD names the new file to write");
    let path = std::path::PathBuf::from(path);
    assert!(!path.exists(), "{} exists: the record is written once", path.display());
    let mut entries = dv::unedited();
    entries.push(dv::saved_control());
    let text = dv::record(&entries);
    std::fs::write(&path, &text).unwrap();
    eprintln!("DV6 (a): {} plans recorded to {}", entries.len(), path.display());
    eprintln!("{text}");
}
