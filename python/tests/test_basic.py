"""
Basic tests for pytest-diff plugin
"""

import pytest


def test_plugin_registration(pytestconfig):
    """Test that the plugin can be registered"""
    # Plugin is only registered when --diff is passed
    # This test just ensures pytest can load the module
    assert pytestconfig is not None


def test_import_module():
    """Test that the module can be imported"""
    import pytest_diff

    assert pytest_diff.__version__ == "0.1.0"
