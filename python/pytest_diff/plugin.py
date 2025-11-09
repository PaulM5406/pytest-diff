"""
Main pytest plugin for pytest-diff

This module integrates with pytest to provide intelligent test selection
based on code changes.
"""

import sys
from pathlib import Path
from typing import List, Optional

import pytest

# Rust core will be imported when built with maturin
try:
    from pytest_diff import _core  # type: ignore
except ImportError:
    _core = None  # Allow import before building


class TestmonPlugin:
    """Main plugin class for pytest-diff"""

    def __init__(self, config):
        self.config = config
        self.enabled = config.getoption("--diff", False)
        self.noselect = config.getoption("--diff-noselect", False)
        self.nocollect = config.getoption("--diff-nocollect", False)

        if not self.enabled:
            return

        if _core is None:
            raise ImportError(
                "pytest-diff Rust core not found. "
                "Please install with: pip install pytest-diff"
            )

        # Initialize components
        self.db_path = Path(config.rootdir) / ".testmondata"
        self.db = None
        self.collector = None
        self.deselected_items = []

    def pytest_configure(self, config):
        """Initialize database and coverage collector"""
        if not self.enabled:
            return

        # TODO: Initialize Rust components when implemented
        # self.db = _core.TestmonDatabase(str(self.db_path))
        # self.collector = _core.CoverageCollector()

    def pytest_collection_modifyitems(self, config, items):
        """Select tests based on code changes"""
        if not self.enabled or self.noselect:
            return

        # TODO: Implement test selection
        # changed = _core.detect_changes(str(self.db_path), str(config.rootdir))
        #
        # if changed.has_changes():
        #     affected_tests = self.db.get_affected_tests(changed.changed_blocks)
        #     selected = [item for item in items if item.nodeid in affected_tests]
        #     self.deselected_items = [item for item in items if item not in selected]
        #     items[:] = selected
        #     config.hook.pytest_deselected(items=self.deselected_items)

    def pytest_runtest_protocol(self, item, nextitem):
        """Start coverage collection for a test"""
        if not self.enabled or self.nocollect:
            return

        # TODO: Start coverage collection
        # self.collector.start_test(item.nodeid)
        # setup_coverage(self.collector)

    def pytest_runtest_logfinish(self, nodeid, location):
        """Finish coverage collection and save to database"""
        if not self.enabled or self.nocollect:
            return

        # TODO: Save coverage data
        # teardown_coverage()
        # coverage_data = self.collector.finish_test()
        # fingerprints = [...]
        # self.db.save_test_execution(nodeid, fingerprints, 0.0, False)

    def pytest_terminal_summary(self, terminalreporter):
        """Show summary of deselected tests"""
        if not self.enabled:
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
        "--diff-noselect",
        action="store_true",
        help="Collect coverage but don't deselect tests",
    )

    group.addoption(
        "--diff-nocollect",
        action="store_true",
        help="Select tests but don't collect coverage",
    )

    parser.addini(
        "diff_ignore_patterns",
        type="linelist",
        help="List of file patterns to ignore",
        default=[],
    )


def pytest_configure(config):
    """Register the plugin"""
    if config.getoption("--diff"):
        plugin = TestmonPlugin(config)
        config.pluginmanager.register(plugin, "pytest_diff")
