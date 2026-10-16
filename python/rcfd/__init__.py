"""rcfd: Python access to the rcfd CFD solver through its command line (docs/python.md).

Standard library only; numpy is used lazily for field arrays when it is installed.
"""

import json as _json

from . import _binary
from .case import Case, RunResult
from .errors import RcfdError, SettingsError, StudyError
from .study import Linspace, List, Logspace, StudyResult, study

__version__ = "0.1.0"
find_binary = _binary.find_binary

__all__ = [
    "Case", "RunResult", "RcfdError", "SettingsError", "StudyError", "List", "Linspace", "Logspace",
    "StudyResult", "study", "find_binary", "schema", "__version__",
]


def schema(binary=None):
    """The JSON schema of the case settings (``rcfd schema``) as a dictionary."""
    exe = _binary.find_binary(binary)
    code, out, err = _binary.call(exe, ["schema"])
    if code != 0:
        raise RcfdError(err.strip() or "rcfd schema failed", code, err)
    return _json.loads(out)
