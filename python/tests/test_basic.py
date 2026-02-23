"""
Basic tests for pytest-difftest plugin
"""


def test_plugin_registration(pytestconfig):
    """Test that the plugin can be registered"""
    # Plugin is only registered when --diff is passed
    # This test just ensures pytest can load the module
    assert pytestconfig is not None


def test_import_module():
    """Test that the module can be imported"""
    import pytest_difftest

    assert pytest_difftest.__version__  # Non-empty version string
