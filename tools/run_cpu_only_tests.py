"""Run numerical regression tests with CUDA unavailable in this process only."""

from __future__ import annotations

import ctypes
import os
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
MODULES = (
    "tests.test_auxiliary_generation",
    "tests.test_effect_seed_solver",
    "tests.test_effect_sequence",
    "tests.test_joint_solver",
    "tests.test_backend_freeze",
)


def main() -> int:
    os.chdir(ROOT)
    sys.path.insert(0, str(ROOT))
    from nioh3_scroll_editor import seed_accelerator

    library = seed_accelerator._load_accelerator()
    if library is None:
        raise RuntimeError("The ABI-v2 Windows accelerator is required for CPU regression")
    force_failure = library.seed_accelerator_test_force_cuda_failure
    force_failure.argtypes = (ctypes.c_int,)
    force_failure.restype = None

    class CpuOnlyResult(unittest.TextTestResult):
        def startTest(self, test):
            # A policy test may restore its own fault hook after it finishes.
            force_failure(1)
            super().startTest(test)

    force_failure(1)
    try:
        suite = unittest.defaultTestLoader.loadTestsFromNames(MODULES)
        result = unittest.TextTestRunner(verbosity=2, resultclass=CpuOnlyResult).run(suite)
        return 0 if result.wasSuccessful() else 1
    finally:
        force_failure(0)
        library.seed_accelerator_set_execution_policy(
            seed_accelerator.EXECUTION_POLICY_STRICT_GPU
        )


if __name__ == "__main__":
    raise SystemExit(main())
