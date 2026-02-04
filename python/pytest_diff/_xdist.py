"""pytest-xdist coordination utilities."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pytest


def is_xdist_worker(config: pytest.Config) -> bool:
    """Return True if running as an xdist worker process."""
    return hasattr(config, "workerinput")


def is_xdist_controller(config: pytest.Config) -> bool:
    """Return True if running as the xdist controller."""
    return hasattr(config, "workercount") and not hasattr(config, "workerinput")


def get_worker_id(config: pytest.Config) -> str | None:
    """Get worker ID (e.g., 'gw0') or None if not a worker."""
    if hasattr(config, "workerinput"):
        return config.workerinput.get("workerid")
    return None
