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
    """Running baseline twice doesn't fail or corrupt data."""
    result1 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=2)

    result2 = sample_project.runpytest_subprocess("--diff-baseline", "-v")
    result2.assert_outcomes(passed=2)
    result2.stdout.fnmatch_lines(["*Baseline saved for * files*"])
