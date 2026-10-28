"""Self-test of scripts/studio.ps1 and scripts/studio.sh, argument handling only (ADR 0141): cargo is never run (dry
runs, or a fake cargo named by CARGO) and no Studio is started. A shell that is not installed is skipped with a SKIP
line. python -m unittest scripts/test_studio_scripts.py"""
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TAIL = ["--release", "-p", "rcfd-studio", "-p", "rcfd-runner", "-p", "rcfd-cli", "-p", "rcfd-mcp"]
# a folder name with a space and a single quote (a double quote is not a valid Windows file name character)
ODD = "my proj" + os.sep + "it's here"


def lines(out, tag):
    return [line[len(tag) + 2:] for line in out.splitlines() if line.startswith(tag + "> ")]


def slashes(p):
    return p.replace("\\", "/")


def fake_cargo(folder, code, windows_cmd):
    """A cargo that writes its arguments to $FAKE_LOG and exits with `code`."""
    if windows_cmd:
        p = os.path.join(folder, "cargo.cmd")
        with open(p, "w", newline="\r\n") as f:
            f.write('@echo off\necho %* > "%FAKE_LOG%"\nexit /b {}\n'.format(code))
    else:
        p = os.path.join(folder, "cargo")
        with open(p, "w", newline="\n") as f:
            f.write('#!/bin/sh\nprintf "%s\\n" "$@" > "$FAKE_LOG"\nexit {}\n'.format(code))
        os.chmod(p, os.stat(p).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return p


class Base:
    """The checks of one script under one shell; subclasses set CMD, DRY, NOBUILD, EXE and WINDOWS_CMD."""

    CMD = None

    def setUp(self):
        if self.CMD is None:
            print("SKIP {}: shell not installed".format(type(self).__name__), file=sys.stderr)
            self.skipTest("shell not installed")

    def run_script(self, args, env=None):
        return subprocess.run(self.CMD + args, capture_output=True, text=True, env=env, timeout=120)

    def test_dry_run_prints_the_build_and_the_start_without_a_project(self):
        r = self.run_script([self.DRY])
        self.assertEqual(r.returncode, 0, r.stderr)
        b, s = lines(r.stdout, "build"), lines(r.stdout, "start")
        self.assertEqual(b[0], "cargo")
        self.assertEqual(b[1:3], ["build", "--manifest-path"])
        self.assertTrue(slashes(b[3]).endswith("/Cargo.toml"), b[3])
        if self.NATIVE_PATHS:
            # the repository root, from the script's own location
            self.assertTrue(os.path.samefile(os.path.dirname(b[3]), os.path.dirname(HERE)), b[3])
        self.assertEqual(b[4:], TAIL)
        self.assertEqual(len(s), 1, s)
        self.assertTrue(slashes(s[0]).endswith("target/release/rcfd-studio" + self.EXE), s[0])

    def test_a_project_with_a_space_and_a_quote_is_one_argument(self):
        r = self.run_script([self.DRY, ODD])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(lines(r.stdout, "start")[1:], [ODD])

    def test_no_build_skips_the_build(self):
        r = self.run_script([self.NOBUILD, self.DRY, "proj"])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(lines(r.stdout, "build"), [])
        self.assertEqual(lines(r.stdout, "start")[1:], ["proj"])

    def test_two_project_folders_exit_2(self):
        r = self.run_script([self.DRY, "a", "b"])
        self.assertEqual(r.returncode, 2)
        self.assertIn("at most one project folder", r.stderr)
        self.assertEqual(lines(r.stdout, "start"), [])

    def test_an_unknown_option_exits_2(self):
        r = self.run_script(["--bogus"])
        self.assertEqual(r.returncode, 2)
        self.assertIn("unknown option", r.stderr)

    def test_help(self):
        r = self.run_script(["-h"])
        self.assertEqual(r.returncode, 0)
        self.assertIn("usage:", r.stdout)

    def test_a_failed_build_starts_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, CARGO=fake_cargo(d, 3, self.WINDOWS_CMD), FAKE_LOG=os.path.join(d, "args.txt"),
                       CARGO_TARGET_DIR=os.path.join(d, "target"))
            r = self.run_script(["proj"], env)
            self.assertEqual(r.returncode, 3, r.stdout + r.stderr)
            self.assertIn("the build failed (exit 3)", r.stderr)
            self.assertNotIn("not found", r.stderr)
            with open(os.path.join(d, "args.txt")) as f:
                got = f.read().split()
            self.assertEqual(got[0], "build")
            self.assertEqual(got[-len(TAIL):], TAIL)

    def test_a_missing_studio_after_the_build_is_named(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, CARGO=fake_cargo(d, 0, self.WINDOWS_CMD), FAKE_LOG=os.path.join(d, "args.txt"),
                       CARGO_TARGET_DIR=os.path.join(d, "target"))
            r = self.run_script(["proj"], env)
            self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
            self.assertIn("rcfd-studio" + self.EXE, r.stderr)
            self.assertIn("not found", r.stderr)


def ps_class(host):
    exe = shutil.which(host)
    windows = os.name == "nt"
    cmd = None if exe is None else [exe, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                                    os.path.join(HERE, "studio.ps1")]
    return type("Ps_" + host, (Base, unittest.TestCase), {
        "CMD": cmd, "DRY": "-DryRun", "NOBUILD": "-NoBuild", "EXE": ".exe" if windows else "",
        "WINDOWS_CMD": windows, "NATIVE_PATHS": True,
    })


Powershell = ps_class("powershell")
Pwsh = ps_class("pwsh")

_SH = shutil.which("sh") or shutil.which("bash")


class Sh(Base, unittest.TestCase):
    CMD = None if _SH is None else [_SH, os.path.join(HERE, "studio.sh")]
    DRY = "--dry-run"
    NOBUILD = "--no-build"
    # under Windows the POSIX shell is MSYS (Git Bash): the Studio is an .exe and paths are written /c/...
    EXE = ".exe" if os.name == "nt" else ""
    WINDOWS_CMD = False
    NATIVE_PATHS = os.name != "nt"


if __name__ == "__main__":
    unittest.main()
