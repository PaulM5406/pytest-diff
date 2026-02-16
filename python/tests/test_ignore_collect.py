"""
Tests for pytest_ignore_collect: skipping unchanged test files early.
"""

import time


def test_unchanged_test_file_skipped(pytester):
    """After baseline with no changes, --diff skips collecting unaffected test files."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": "def add(a, b):\n    return a + b\n",
            "mylib/string_ops.py": "def upper(s):\n    return s.upper()\n",
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
            ),
            "tests/test_string.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.string_ops import upper\n"
                "\n"
                "def test_upper():\n"
                "    assert upper('hello') == 'HELLO'\n"
            ),
        }
    )

    # Baseline: run all tests
    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=2)

    # Modify only calculator.py
    time.sleep(0.01)
    calc = pytester.path / "mylib" / "calculator.py"
    calc.write_text("def add(a, b):\n    return a + b + 0  # modified\n")

    # --diff: test_string.py should be skipped via ignore_collect,
    # only test_calc.py tests should run
    result = pytester.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*test_add*"])
    # test_string tests should not appear at all (file was not collected)
    assert "test_upper" not in result.stdout.str()
    result.assert_outcomes(passed=1)


def test_new_test_file_not_skipped(pytester):
    """A new test file not in the DB should still be collected."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": "def add(a, b):\n    return a + b\n",
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
            ),
        }
    )

    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=1)

    # Add a brand-new test file
    new_test = pytester.path / "tests" / "test_new.py"
    new_test.write_text("def test_brand_new():\n    assert 1 + 1 == 2\n")

    result = pytester.runpytest_subprocess("--diff", "-v")
    # The new test file should be collected and run
    result.stdout.fnmatch_lines(["*test_brand_new*PASSED*"])


def test_affected_test_file_not_skipped(pytester):
    """A test file whose dependencies changed should still be collected."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": "def add(a, b):\n    return a + b\n",
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
            ),
        }
    )

    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=1)

    # Modify the source file that test_calc depends on
    time.sleep(0.01)
    calc = pytester.path / "mylib" / "calculator.py"
    calc.write_text("def add(a, b):\n    return a + b + 0  # modified\n")

    result = pytester.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*test_add*PASSED*"])


def test_conftest_never_skipped(pytester):
    """conftest.py should never be skipped by pytest_ignore_collect."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": "def add(a, b):\n    return a + b\n",
            "tests/__init__.py": "",
            "tests/conftest.py": (
                "import pytest\n\n@pytest.fixture\ndef magic_number():\n    return 42\n"
            ),
            "tests/test_calc.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add(magic_number):\n"
                "    assert add(1, 2) == 3\n"
                "    assert magic_number == 42\n"
            ),
        }
    )

    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=1)

    # Modify calculator to trigger a diff run
    time.sleep(0.01)
    calc = pytester.path / "mylib" / "calculator.py"
    calc.write_text("def add(a, b):\n    return a + b + 0  # modified\n")

    # conftest.py should still be loaded (fixture should work)
    result = pytester.runpytest_subprocess("--diff", "-v")
    result.stdout.fnmatch_lines(["*test_add*PASSED*"])


def test_baseline_mode_no_skipping(pytester):
    """In --diff-baseline mode, pytest_ignore_collect should not skip anything."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": "def add(a, b):\n    return a + b\n",
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                "import sys\n"
                "sys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
            ),
        }
    )

    # First baseline
    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.assert_outcomes(passed=1)

    # Second baseline with no changes: tests are deselected via incremental baseline
    # but the file itself should have been collected (not ignored by pytest_ignore_collect)
    result = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result.stdout.fnmatch_lines(["*No changes detected*"])
    # The file was collected (1 item found) then deselected — not ignored
    result.stdout.fnmatch_lines(["*1 deselected*"])


def test_is_test_file_helper():
    """Test the _is_test_file method uses pytest's python_files patterns."""
    from unittest.mock import MagicMock

    from pytest_diff.plugin import PytestDiffPlugin

    plugin = MagicMock(spec=PytestDiffPlugin)
    # Default pytest python_files patterns
    plugin._python_files = ["test_*.py", "*_test.py"]

    is_test = PytestDiffPlugin._is_test_file

    # test_ prefix
    assert is_test(plugin, "tests/test_calc.py") is True
    assert is_test(plugin, "test_something.py") is True

    # _test.py suffix
    assert is_test(plugin, "calc_test.py") is True
    assert is_test(plugin, "tests/calc_test.py") is True

    # NOT a test file: helpers.py in tests/ dir doesn't match patterns
    assert is_test(plugin, "tests/helpers.py") is False
    assert is_test(plugin, "test/helpers.py") is False

    # Not a test file
    assert is_test(plugin, "mylib/calculator.py") is False
    assert is_test(plugin, "conftest.py") is False
    assert is_test(plugin, "setup.py") is False

    # Nested paths
    assert is_test(plugin, "tests/unit/test_api.py") is True
    assert is_test(plugin, "src/tests/test_api.py") is True

    # Custom python_files pattern
    plugin._python_files = ["check_*.py"]
    assert is_test(plugin, "check_auth.py") is True
    assert is_test(plugin, "test_auth.py") is False
