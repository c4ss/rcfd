"""Tests of scripts/gate_test.py (ADR 0129). Run: python -m unittest scripts/test_gate_test.py -v
The end-to-end time-limit tests (T1, T2) build a tiny cargo crate in a temp folder with `cargo test --release`; they run only with
GATE_TEST_E2E=1 (take a gate slot first)."""
import os
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gate_test import limit_s  # noqa: E402

GATE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "gate_test.py")


class Clamp(unittest.TestCase):
    def test_limit(self):
        cases = [  # (recorded seconds, factor, min, max) -> limit
            ((None, 5, 600, 3600), 3600),  # no record: max
            ((0.0, 5, 600, 3600), 600),  # zero time: min
            ((0.07, 5, 600, 3600), 600),
            ((120, 5, 600, 3600), 600),  # 5 x 120 = 600 exactly
            ((121, 5, 600, 3600), 605),
            ((300, 5, 600, 3600), 1500),
            ((720, 5, 600, 3600), 3600),  # 5 x 720 = max exactly
            ((820, 5, 600, 3600), 3600),
            ((1e12, 5, 600, 3600), 3600),  # huge time: max
            ((10, 3, 10, 10), 10),  # min = max
            ((None, 5, 10, 10), 10),
            ((100, 2, 0, 1000), 200),  # no minimum
            ((100, 5, 600, 0), None),  # max 0: off
            ((None, 5, 600, 0), None),
            ((100, 5, 600, -1), None),
        ]
        for args, want in cases:
            self.assertEqual(limit_s(*args), want, args)


LIB = r"""
#[test]
fn quick() {}

#[test]
fn hang() {
    let secs: u64 = std::env::var("HANG_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(3600);
    let mut child = if cfg!(windows) {
        std::process::Command::new("ping").args(["-n", &(secs + 1).to_string(), "127.0.0.1"]).spawn()
    } else {
        std::process::Command::new("sleep").arg(secs.to_string()).spawn()
    }
    .unwrap();
    std::fs::write(std::env::var("PID_FILE").unwrap(), format!("{} {}", std::process::id(), child.id())).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(secs));
    let _ = child.kill();
    let _ = child.wait();
}
"""


def alive(pid):
    if os.name == "nt":
        out = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"], stdout=subprocess.PIPE).stdout
        return f'"{pid}"'.encode() in out
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


@unittest.skipUnless(os.environ.get("GATE_TEST_E2E") == "1", "set GATE_TEST_E2E=1 (builds a crate with cargo --release)")
class TimeLimit(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="gate_test_e2e_")
        os.makedirs(os.path.join(cls.dir, "src"))
        with open(os.path.join(cls.dir, "Cargo.toml"), "w") as f:
            f.write('[package]\nname = "hang"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n')
        with open(os.path.join(cls.dir, "src", "lib.rs"), "w") as f:
            f.write(LIB)

    def gate(self, hang_secs, *flags):
        pid_file = os.path.join(self.dir, "pids.txt")
        tsv = os.path.join(self.dir, "times.tsv")
        for p in (pid_file, tsv):
            if os.path.exists(p):
                os.remove(p)
        env = dict(os.environ, HANG_SECS=str(hang_secs), PID_FILE=pid_file, CARGO_TARGET_DIR=os.path.join(self.dir, "target"))
        t = time.time()
        r = subprocess.run([sys.executable, GATE, "--tsv", tsv, *flags], cwd=self.dir, env=env, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT)
        out = r.stdout.decode("utf-8", "replace")
        print(f"\n--- hang {hang_secs} s, flags {flags}: exit {r.returncode}, {time.time() - t:.1f} s\n{out}")
        rows = {ln.split("\t")[1]: ln.split("\t") for ln in open(tsv).read().splitlines()[1:]}
        pids = [int(x) for x in open(pid_file).read().split()]
        return r.returncode, out, rows["hang"], pids

    def test_t1_hang_is_killed_with_its_child(self):
        code, out, row, pids = self.gate(3600, "--timeout-min", "10", "--timeout-max", "10")
        self.assertNotEqual(code, 0)
        self.assertEqual(row[2], "TIMEOUT")
        self.assertLessEqual(float(row[3]), 15.0)
        self.assertIn("test hang ... FAILED (timeout after 10 s)", out)
        self.assertIn("test result: FAILED. 1 passed; 1 failed;", out)
        print(f"test pid {pids[0]}, child pid {pids[1]}: alive {alive(pids[0])}, {alive(pids[1])}")
        self.assertFalse(alive(pids[0]) or alive(pids[1]), pids)

    def test_t2_control_short_sleep_runs_to_the_end_without_a_limit(self):
        code, out, row, pids = self.gate(15, "--timeout-max", "0")
        self.assertEqual(code, 0)
        self.assertEqual(row[2], "ok")
        self.assertGreaterEqual(float(row[3]), 15.0)

    def test_t2_same_short_sleep_is_killed_with_the_limit(self):
        code, out, row, pids = self.gate(15, "--timeout-min", "10", "--timeout-max", "10")
        self.assertNotEqual(code, 0)
        self.assertEqual(row[2], "TIMEOUT")
        self.assertFalse(alive(pids[0]) or alive(pids[1]), pids)


if __name__ == "__main__":
    unittest.main()
