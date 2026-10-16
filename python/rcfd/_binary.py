"""Locating and calling the rcfd binary (always with an argument list, never through a shell)."""

import os
import subprocess

from .errors import RcfdError

_here = os.path.dirname(os.path.abspath(__file__))


def find_binary(path=None):
    """Return the path of the ``rcfd`` executable.

    Order: the ``path`` argument, the ``RCFD_BIN`` environment variable, ``target*/release`` and
    ``target*/debug`` directories next to the repository root of this package. Raises ``RcfdError``
    with the places that were tried when nothing is found.
    """
    tried = []
    candidates = []
    if path:
        candidates.append(path)
    env = os.environ.get("RCFD_BIN")
    if env:
        candidates.append(env)
    root = os.path.dirname(os.path.dirname(_here))
    for base in (root, os.path.dirname(root)):
        try:
            names = sorted(os.listdir(base))
        except OSError:
            continue
        for n in names:
            if n == "target" or n.startswith("target-"):
                for prof in ("release", "debug"):
                    for exe in ("rcfd.exe", "rcfd"):
                        candidates.append(os.path.join(base, n, prof, exe))
    for c in candidates:
        tried.append(c)
        if os.path.isfile(c):
            return os.path.abspath(c)
    raise RcfdError("rcfd binary not found; set RCFD_BIN or pass binary=...; tried: " + ", ".join(tried))


def call(binary, args, cwd=None):
    """Run ``rcfd <args>``; return ``(exit_code, stdout, stderr)`` as text (UTF-8)."""
    proc = subprocess.run(
        [binary] + [str(a) for a in args],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        stdin=subprocess.DEVNULL,
        check=False,
    )
    return (
        proc.returncode,
        proc.stdout.decode("utf-8", errors="replace"),
        proc.stderr.decode("utf-8", errors="replace"),
    )
