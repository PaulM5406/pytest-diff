"""
Tests for plugin registration, CLI/ini options, and precedence.
"""


def test_plugin_not_registered_without_flags(pytester):
    """No pytest-diff output when neither --diff nor --diff-baseline passed."""
    pytester.makepyfile("def test_noop(): pass")
    result = pytester.runpytest_subprocess("-v")
    result.assert_outcomes(passed=1)
    result.stdout.no_fnmatch_line("*pytest-diff: Using database*")


def test_plugin_registered_with_diff_flag(pytester):
    """Plugin activates with --diff."""
    pytester.makepyfile("def test_noop(): pass")
    result = pytester.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*pytest-diff*"])


def test_plugin_registered_with_baseline_flag(pytester):
    """Plugin activates with --diff-baseline."""
    pytester.makepyfile("def test_noop(): pass")
    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.stdout.fnmatch_lines(["*pytest-diff*Baseline saved*"])


def test_help_shows_all_options(pytester):
    """All diff options appear in --help output."""
    result = pytester.runpytest_subprocess("--help")
    result.stdout.fnmatch_lines(
        [
            "*--diff *",
            "*--diff-baseline*",
            "*--diff-v*",
            "*--diff-batch-size*",
            "*--diff-cache-size*",
        ]
    )


def test_verbose_flag_produces_timing_output(sample_project):
    """--diff-v outputs timing messages."""
    result = sample_project.runpytest_subprocess("--diff-baseline", "--diff-v", "-v")
    result.assert_outcomes(passed=2)
    result.stdout.fnmatch_lines(["*pytest-diff:*"])


def test_batch_size_cli_override(sample_project):
    """--diff-batch-size=1 causes per-test flush."""
    result = sample_project.runpytest_subprocess(
        "--diff-baseline", "--diff-v", "--diff-batch-size=1", "-v"
    )
    result.assert_outcomes(passed=2)
    # With batch_size=1 and verbose, each test triggers a flush
    result.stdout.fnmatch_lines(["*Flushed 1 test executions*"])


def test_ini_option_respected(pytester):
    """diff_batch_size in ini config works."""
    pytester.makepyfile("def test_noop(): pass")
    pytester.makeini(
        """
[pytest]
diff_batch_size = 5
"""
    )
    # Just verify it doesn't crash — ini is parsed at startup
    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=1)


def test_cli_overrides_ini(pytester):
    """CLI --diff-batch-size takes precedence over ini."""
    pytester.makepyfile("def test_noop(): pass")
    pytester.makeini(
        """
[pytest]
diff_batch_size = 999
"""
    )
    # CLI value should win; with verbose we can observe flush behavior
    result = pytester.runpytest_subprocess(
        "--diff-baseline", "--diff-v", "--diff-batch-size=1", "-v"
    )
    result.assert_outcomes(passed=1)
    result.stdout.fnmatch_lines(["*Flushed 1 test executions*"])
