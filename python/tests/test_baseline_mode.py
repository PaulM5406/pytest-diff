"""
Tests for --diff-baseline end-to-end via pytester.
"""


def test_baseline_runs_all_tests(sample_project):
    """--diff-baseline runs all tests, no deselection."""
    result = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=2)
    # Should NOT mention deselection
    result.stdout.no_fnmatch_line("*deselected*")


def test_baseline_saves_fingerprints(sample_project):
    """Output contains 'Baseline saved for N files'."""
    result = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=2)
    result.stdout.fnmatch_lines(["*Baseline saved for * files*"])


def test_baseline_creates_database(sample_project):
    """Database file exists after baseline run."""
    sample_project.runpytest_subprocess("--diff-baseline")
    db_path = sample_project.path / ".pytest_cache" / "pytest-diff" / "pytest_diff.db"
    assert db_path.exists(), f"Database not found at {db_path}"


def test_baseline_idempotent(sample_project):
    """Running baseline twice: second run is incremental and skips all tests."""
    result1 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=2)

    result2 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    # No changes since first baseline — incremental mode skips all tests
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*No changes detected*"])
    result2.stdout.fnmatch_lines(["*Baseline saved for * files*"])


def test_baseline_incremental_runs_affected_tests(sample_project):
    """Incremental baseline only runs tests affected by changes."""
    import time

    # First baseline: run all tests
    result1 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=2)

    # Modify source file
    time.sleep(0.01)
    calc = sample_project.path / "mylib" / "calculator.py"
    calc.write_text(
        "def add(a, b):\n"
        "    return a + b + 0  # modified\n"
        "\n"
        "def multiply(a, b):\n"
        "    return a * b\n"
    )

    # Second baseline: incremental, only affected tests run
    result2 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result2.stdout.fnmatch_lines(["*Incremental baseline*"])
    result2.stdout.fnmatch_lines(["*Baseline saved for * files*"])


def test_baseline_force_runs_all_tests(sample_project):
    """--diff-force with --diff-baseline always runs all tests."""
    # First baseline
    result1 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=2)

    # Second baseline with --diff-force: runs all tests even though nothing changed
    result2 = sample_project.runpytest_subprocess("--diff-baseline", "--diff-force", "-v")
    result2.assert_outcomes(passed=2)
    result2.stdout.no_fnmatch_line("*deselected*")
    result2.stdout.fnmatch_lines(["*Baseline saved for * files*"])
