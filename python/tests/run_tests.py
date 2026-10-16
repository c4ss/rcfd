"""Run the Python tests of the rcfd package. Usage: python -I python/tests/run_tests.py

The package directory is put on ``sys.path`` here (computed from this file), not through the environment, so the
run also works under ``python -I``. ``RCFD_BIN`` must point at the built ``rcfd`` binary (or one must exist in
a ``target*`` directory).
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
sys.path.insert(0, HERE)

if __name__ == "__main__":
    suite = unittest.defaultTestLoader.discover(HERE, pattern="test_*.py")
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(0 if result.wasSuccessful() else 1)
