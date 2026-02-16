"""
pytest-difftest: Blazingly fast test selection for pytest

Only run tests affected by your changes, powered by Rust.
"""

__version__ = "0.1.0"
__author__ = "Paul Milesi"

# Re-export main plugin
from .plugin import pytest_addoption, pytest_configure

__all__ = ["pytest_addoption", "pytest_configure", "__version__"]
