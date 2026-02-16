"""Configuration helpers for pytest-difftest.

Extracted from plugin.py to keep the main module focused on pytest hooks.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import pytest

logger = logging.getLogger("pytest_difftest")


def get_rootdir(config: pytest.Config) -> Path:
    """Get pytest rootdir from config.

    pytest adds `rootdir` dynamically at runtime, so we use getattr
    to avoid type checker complaints about the missing attribute.
    """
    return Path(getattr(config, "rootdir"))


def get_workerinput(config: pytest.Config) -> dict[str, Any]:
    """Get xdist workerinput dict from config.

    pytest-xdist adds `workerinput` dynamically to worker configs.
    Returns empty dict if not running as xdist worker.
    """
    return getattr(config, "workerinput", {})


def get_config_value(config: pytest.Config, cli_name: str, ini_name: str, default: int) -> int:
    """Get config value from CLI option or ini file, with fallback to default.

    CLI options take precedence over ini options. Supports configuration via:
    - Command line: --diff-{cli_name}
    - pyproject.toml: [tool.pytest.ini_options] diff_{ini_name} = value
    """
    cli_value = config.getoption(f"--diff-{cli_name}", None)
    if cli_value is not None:
        return cli_value

    ini_value = config.getini(f"diff_{ini_name}")
    if ini_value:
        try:
            return int(ini_value)
        except (ValueError, TypeError):
            pass

    return default


def get_scope_paths(config: pytest.Config) -> list[str]:
    """Get the absolute paths that define the pytest invocation scope.

    If user runs 'pytest tests/unit/', we should only track files under tests/unit/.
    If no args provided, track the entire rootdir.
    """
    if not config.args:
        return [str(get_rootdir(config).resolve())]

    scope_paths: list[str] = []
    for arg in config.args:
        file_path = arg.split("::")[0]
        path = Path(file_path)
        if not path.is_absolute():
            path = get_rootdir(config) / path

        try:
            resolved = path.resolve(strict=False)
            if resolved.is_dir():
                scope_paths.append(str(resolved))
            elif resolved.is_file() or file_path.endswith(".py"):
                scope_paths.append(str(resolved.parent))
        except (OSError, RuntimeError):
            pass

    return scope_paths if scope_paths else [str(get_rootdir(config).resolve())]


def is_subpath(child: Path, parent: Path) -> bool:
    """Check if *child* is equal to or a subdirectory of *parent*."""
    try:
        child.relative_to(parent)
        return True
    except ValueError:
        return False


def relative_scope_paths(scope_paths: list[str], rootdir: str) -> list[str]:
    """Convert absolute scope paths to relative paths from *rootdir*.

    Paths equal to rootdir become ``'.'``.
    """
    result: list[str] = []
    for p in scope_paths:
        try:
            result.append(str(Path(p).relative_to(rootdir)))
        except ValueError:
            result.append(p)
    return result


def check_scope_mismatch(
    db: Any,
    config: pytest.Config,
    scope_paths: list[str],
    is_baseline: bool,
) -> bool:
    """Check if the current diff scope differs from the baseline scope.

    Returns True if there is a mismatch.
    In --diff-baseline mode the caller should run all tests to rebuild properly.
    In --diff mode this is informational only.
    """
    if db is None:
        return False

    raw = db.get_metadata("baseline_scope")
    if raw is None:
        return False
    try:
        baseline_scope: list[str] = json.loads(raw)
    except (json.JSONDecodeError, TypeError):
        return False

    rootdir = str(get_rootdir(config))
    current_scope = relative_scope_paths(scope_paths, rootdir)

    if sorted(baseline_scope) == sorted(current_scope):
        return False

    baseline_paths = [Path(p) for p in baseline_scope]
    current_paths = [Path(p) for p in current_scope]
    is_subscope = all(any(is_subpath(cp, bp) for bp in baseline_paths) for cp in current_paths)
    if is_subscope:
        return False

    baseline_display = ", ".join(baseline_scope) or "."
    current_display = ", ".join(current_scope) or "."
    if is_baseline:
        logger.warning(
            "⚠ pytest-difftest: Scope mismatch — baseline was built with [%s] "
            "but current run uses [%s]. Running all tests to rebuild baseline.",
            baseline_display,
            current_display,
        )
    else:
        logger.warning(
            "⚠ pytest-difftest: Scope mismatch — baseline was built with [%s] "
            "but current run uses [%s]. "
            "Some tests may not be selected. "
            "Consider re-running: pytest --diff-baseline %s",
            baseline_display,
            current_display,
            current_display,
        )
    return True
