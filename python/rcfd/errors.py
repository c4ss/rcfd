"""Errors raised by the rcfd Python package."""

import re

_PATH_RE = re.compile(r"^[A-Za-z0-9_.\-]+$")


class RcfdError(Exception):
    """A failed call of the rcfd binary.

    Attributes: ``exit_code`` (the run-control exit code of the binary), ``stderr`` (its text) and
    ``diagnostics`` (a list of ``(path, message)`` tuples parsed from it; the path is the dotted
    settings path exactly as the command line prints it, ``""`` when there is none).
    """

    def __init__(self, message, exit_code=None, stderr="", diagnostics=None):
        super().__init__(message)
        self.exit_code = exit_code
        self.stderr = stderr
        self.diagnostics = list(diagnostics or [])


class SettingsError(RcfdError):
    """An invalid settings path or value; ``path`` and ``message`` are those of the first diagnostic."""

    def __init__(self, path, message, exit_code=None, stderr="", diagnostics=None):
        super().__init__(f"{path}: {message}" if path else message, exit_code, stderr, diagnostics)
        self.path = path
        self.message = message


class StudyError(RcfdError):
    """An invalid study definition (static sweep errors, exit code 3)."""


def parse_diagnostics(stderr, label=None):
    """Extract ``(path, message)`` pairs from the text the binary prints on stderr.

    Lines look like ``<label>: --set <path>: <message>``, ``<label>: line 12: <path>: <message>`` or
    ``<label>: <path>: <message>``; ``error: --set ...`` lines (usage errors) are kept with an empty
    path unless a path can be read.
    """
    out = []
    for line in stderr.splitlines():
        line = line.rstrip()
        if not line:
            continue
        rest = line
        if label and rest.startswith(label + ": "):
            rest = rest[len(label) + 2:]
        elif rest.startswith("error: "):
            rest = rest[len("error: "):]
        else:
            continue
        if rest.startswith("--set "):
            rest = rest[len("--set "):]
        m = re.match(r"^line \d+: (.*)$", rest)
        if m:
            rest = m.group(1)
        head, sep, tail = rest.partition(": ")
        head = head.strip("'")
        if sep and _PATH_RE.match(head):
            out.append((head, tail))
        else:
            out.append(("", rest))
    return out
