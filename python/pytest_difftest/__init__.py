"""
pytest-difftest: Blazingly fast test selection for pytest

Only run tests affected by your changes, powered by Rust.
"""

from importlib.metadata import PackageNotFoundError, version

try:
    __version__ = version("pytest-difftest")
except PackageNotFoundError:
    __version__ = "0.0.0"  # Not installed (dev/editable mode fallback)
__author__ = "Paul Milesi"

# Re-export main plugin
from .plugin import pytest_addoption, pytest_configure

__all__ = ["pytest_addoption", "pytest_configure", "__version__"]
