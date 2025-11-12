"""
Main pytest plugin for pytest-diff

This module integrates with pytest to provide intelligent test selection
based on code changes.
"""

from __future__ import annotations

from pathlib import Path


# Coverage module will be imported when needed (not at module level)
# to avoid caching None if not installed during initial import

# Rust core will be imported when built with maturin
try:
    from pytest_diff import _core  # type: ignore
except ImportError:
    _core = None  # Allow import before building


class TestmonPlugin:
    """Main plugin class for pytest-diff"""

    def __init__(self, config):
        self.config = config
        self.baseline = config.getoption("--diff-baseline", False)
        self.enabled = config.getoption("--diff", False) or self.baseline

        if not self.enabled:
            return

        if _core is None:
            raise ImportError(
                "pytest-diff Rust core not found. " "Please install with: pip install pytest-diff"
            )

        # Initialize components
        self.db_path = Path(config.rootdir) / ".testmondata"
        self.db = None
        self.cov = None
        self.deselected_items = []
        self.current_test = None
        self.test_start_time = None
        self.test_files_executed = []

    def pytest_configure(self, config):
        """Initialize database and coverage collector"""
        if config.option.verbose >= 2:
            print(f"\n[DEBUG] TestmonPlugin.pytest_configure called, enabled={self.enabled}")

        if not self.enabled:
            return

        # Initialize Rust components
        try:
            self.db = _core.TestmonDatabase(str(self.db_path))
            print(f"✓ pytest-diff: Using database at {self.db_path}")
        except Exception as e:
            print(f"⚠ pytest-diff: Could not open database: {e}")
            print(f"  Creating new database at {self.db_path}")
            self.db = _core.TestmonDatabase(str(self.db_path))

        # Initialize coverage if available
        coverage_module = None
        try:
            import coverage as coverage_module
        except ImportError:
            pass

        if config.option.verbose >= 2:
            print(f"[DEBUG] Coverage module available: {coverage_module is not None}")

        if coverage_module:
            self.cov = coverage_module.Coverage(
                data_file=None,  # Don't save coverage data
                branch=False,
                config_file=False,
                source=[str(config.rootdir)],
            )
            if config.option.verbose >= 2:
                print("[DEBUG] Coverage initialized successfully")

    def pytest_collection_modifyitems(self, config, items):
        """Select tests based on code changes"""
        if not self.enabled or self.baseline:
            # Skip selection when:
            # - Not enabled
            # - Running in baseline mode (need to run all tests to set baseline)
            return

        try:
            # Detect changes
            changed = _core.detect_changes(str(self.db_path), str(config.rootdir))

            if changed.has_changes():
                print(f"\n✓ pytest-diff: Detected {len(changed.modified)} modified files")
                print(f"  Changed blocks in {len(changed.changed_blocks)} files")

                # Get affected tests from database
                affected_tests = self.db.get_affected_tests(changed.changed_blocks)

                if affected_tests:
                    # Select only affected tests
                    selected = [item for item in items if item.nodeid in affected_tests]
                    self.deselected_items = [item for item in items if item not in selected]
                    items[:] = selected

                    print(f"  Running {len(selected)} affected tests")
                    print(f"  Skipping {len(self.deselected_items)} unaffected tests")

                    if self.deselected_items:
                        config.hook.pytest_deselected(items=self.deselected_items)
                else:
                    print("  No tests affected by changes (database may be empty)")
                    print(f"  Running all {len(items)} tests to build database")
            else:
                print("\n✓ pytest-diff: No changes detected")
                print(f"  Skipping all {len(items)} tests")
                self.deselected_items = items
                items[:] = []
                config.hook.pytest_deselected(items=self.deselected_items)
        except Exception as e:
            print(f"\n⚠ pytest-diff: Error during change detection: {e}")
            print("  Running all tests")
            import traceback

            traceback.print_exc()

    def pytest_runtest_protocol(self, item, nextitem):
        """Start coverage collection for a test"""
        if not self.enabled:
            return

        import time

        self.current_test = item.nodeid
        self.test_start_time = time.time()
        self.test_files_executed = []

        # Start coverage collection
        if self.cov:
            if self.config.option.verbose >= 2:
                print(f"\n[DEBUG] Starting coverage for {item.nodeid}")
            self.cov.start()
        elif self.config.option.verbose >= 2:
            print(f"\n[DEBUG] Coverage not available for {item.nodeid}")

    def pytest_runtest_makereport(self, item, call):
        """Capture test result and save to database"""
        if not self.enabled:
            return

        # Only save after test execution (not setup/teardown)
        if call.when != "call":
            return

        import time

        duration = time.time() - (self.test_start_time or time.time())
        failed = call.excinfo is not None

        try:
            # Stop coverage and get executed files
            fingerprints = []
            seen_files = set()

            if self.cov:
                self.cov.stop()
                data = self.cov.get_data()

                # Debug: log how many files coverage found
                measured = list(data.measured_files())
                if self.config.option.verbose >= 2:
                    print(f"\n[DEBUG] Coverage measured {len(measured)} files")
                    for f in measured[:5]:
                        print(f"  - {f}")

                # Get all .py files that were executed
                for filename in measured:
                    filepath = Path(filename)
                    # Include project files (source and tests)
                    if filepath.suffix == ".py" and str(filepath).startswith(
                        str(self.config.rootdir)
                    ):
                        abs_path = str(filepath.resolve())
                        if abs_path not in seen_files:
                            seen_files.add(abs_path)
                            try:
                                fp = _core.calculate_fingerprint(abs_path)
                                fingerprints.append(fp)
                            except Exception as e:
                                # Skip files that can't be fingerprinted, but log in verbose mode
                                if self.config.option.verbose:
                                    print(f"\n⚠ pytest-diff: Could not fingerprint {abs_path}: {e}")

                self.cov.erase()  # Clear coverage data for next test

            # Always include the test file itself
            test_file = Path(item.fspath).resolve()
            test_file_str = str(test_file)
            if test_file_str not in seen_files and test_file.exists() and test_file.suffix == ".py":
                try:
                    fingerprints.append(_core.calculate_fingerprint(test_file_str))
                except Exception:
                    pass

            # Save test execution
            if fingerprints:
                self.db.save_test_execution(item.nodeid, fingerprints, duration, failed)
        except Exception as e:
            # Don't fail the test run if we can't save to database
            if self.config.option.verbose:
                print(f"\n⚠ pytest-diff: Could not save test execution: {e}")

    def pytest_terminal_summary(self, terminalreporter):
        """Show summary of deselected tests"""
        if not self.enabled:
            return

        # If baseline mode, save baseline fingerprints
        if self.baseline:
            try:
                count = _core.save_baseline(str(self.db_path), str(self.config.rootdir))
                terminalreporter.write_sep(
                    "=",
                    f"pytest-diff: Baseline saved for {count} files",
                    green=True,
                )
            except Exception as e:
                terminalreporter.write_sep(
                    "=",
                    f"pytest-diff: Failed to save baseline: {e}",
                    red=True,
                )
            return

        if self.deselected_items:
            terminalreporter.write_sep(
                "=",
                f"pytest-diff: {len(self.deselected_items)} tests deselected",
                green=True,
            )


def pytest_addoption(parser):
    """Add command-line options for pytest-diff"""
    group = parser.getgroup("diff", "pytest-diff test selection")

    group.addoption(
        "--diff",
        action="store_true",
        help="Enable pytest-diff (select tests based on changes)",
    )

    group.addoption(
        "--diff-baseline",
        action="store_true",
        help="Run all tests and save current state as baseline for change detection",
    )

    parser.addini(
        "diff_ignore_patterns",
        type="linelist",
        help="List of file patterns to ignore",
        default=[],
    )


def pytest_configure(config):
    """Register the plugin"""
    if config.getoption("--diff") or config.getoption("--diff-baseline"):
        plugin = TestmonPlugin(config)
        config.pluginmanager.register(plugin, "pytest_diff")
