"""
Tests for --diff: change detection and test selection.
"""

import time


def test_no_changes_skips_all(baselined_project):
    """After baseline with no changes, --diff skips all tests."""
    result = baselined_project.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*No changes detected*"])
    # No tests should have run
    result.assert_outcomes()


def test_modified_source_runs_affected_tests(baselined_project):
    """Changing a source file causes dependent tests to run."""
    # Modify the calculator module
    time.sleep(0.01)
    calc = baselined_project.path / "mylib" / "calculator.py"
    calc.write_text(
        "def add(a, b):\n"
        "    return a + b + 0  # modified\n"
        "\n"
        "def multiply(a, b):\n"
        "    return a * b\n"
    )

    result = baselined_project.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*modified*"])


def test_unmodified_module_tests_deselected(multi_module_project):
    """Only tests touching modified module run (multi-module project)."""
    # First, baseline
    result = multi_module_project.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=4)

    # Modify only math_ops
    time.sleep(0.01)
    math_ops = multi_module_project.path / "mylib" / "math_ops.py"
    math_ops.write_text(
        "def add(a, b):\n"
        "    return a + b + 0  # modified\n"
        "\n"
        "def subtract(a, b):\n"
        "    return a - b\n"
    )

    result = multi_module_project.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*modified*"])
    # string tests should be deselected
    result.stdout.fnmatch_lines(["*deselected*"])


def test_multiple_diff_runs_stable(baselined_project):
    """Running --diff 3x without changes always skips all."""
    for _ in range(3):
        result = baselined_project.runpytest_subprocess("--diff", "-v")
        result.stdout.fnmatch_lines(["*No changes detected*"])
        result.assert_outcomes()
