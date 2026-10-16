//! ADR 0038 check D2 (fallback when no device, simulated by RCFD_GPU_DISABLE in a CHILD process because the probe is
//! cached per process) and the default-build half of the located error. Runs with and without the feature `cuda`.

use rcfd_gpu::{probe, resolve, DeviceStatus, AUTO_MIN_UNKNOWNS, DISABLE_VAR};
use rcfd_linalg::LinearDevice;
use std::process::Command;

/// Child body: only does something when the parent set the override.
#[test]
fn d2_child_body() {
    if std::env::var(DISABLE_VAR).as_deref() != Ok("1") {
        return;
    }
    match probe() {
        DeviceStatus::Unavailable { reason } => assert!(reason.contains(DISABLE_VAR), "{reason}"),
        other => panic!("override ignored: {other:?}"),
    }
    let big = AUTO_MIN_UNKNOWNS * 10;
    let r = resolve(LinearDevice::Auto, big).unwrap();
    assert!(r.backend.is_none(), "auto must fall back to cpu");
    assert!(
        r.reason.contains(DISABLE_VAR),
        "reason must name the cause: {}",
        r.reason
    );
    let e = match resolve(LinearDevice::Cuda, big) {
        Err(e) => e,
        Ok(_) => panic!("cuda must be an error without a device"),
    };
    assert!(
        e.message.contains("linear_solver.device") && e.message.contains(DISABLE_VAR),
        "{e}"
    );
    assert!(resolve(LinearDevice::Cpu, big).unwrap().backend.is_none());
    println!("D2 CHILD OK: {}", probe().describe());
}

#[test]
fn d2_fallback_without_device_in_child_process() {
    let exe = std::env::current_exe().unwrap();
    let out = Command::new(exe)
        .args(["d2_child_body", "--exact", "--nocapture", "--test-threads=1"])
        .env(DISABLE_VAR, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("child stdout: {stdout}");
    assert!(
        out.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("D2 CHILD OK"), "the child did not run its assertions");
}

#[cfg(not(feature = "cuda"))]
#[test]
fn default_build_reports_missing_feature() {
    match probe() {
        DeviceStatus::Unavailable { reason } => assert!(
            reason.contains("feature cuda") || reason.contains(DISABLE_VAR),
            "{reason}"
        ),
        other => panic!("{other:?}"),
    }
    let e = match resolve(LinearDevice::Cuda, 10) {
        Err(e) => e,
        Ok(_) => panic!("cuda must be an error in a build without the feature"),
    };
    assert!(e.message.contains("linear_solver.device"), "{e}");
}
