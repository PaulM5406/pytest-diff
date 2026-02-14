"""
Tests for recording skipped and xfail tests in baseline.
"""

import pytest

CALCULATOR_SRC = "def add(a, b):\n    return a + b\n\ndef multiply(a, b):\n    return a * b\n"

SYS_PATH_PREAMBLE = (
    "import sys\nsys.path.insert(0, str(__import__('pathlib').Path(__file__).parent.parent))\n"
)


@pytest.fixture
def project_with_skip_marker(pytester):
    """Project with a @pytest.mark.skip test."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": CALCULATOR_SRC,
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                SYS_PATH_PREAMBLE + "import pytest\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
                "\n"
                "@pytest.mark.skip(reason='not ready')\n"
                "def test_skipped():\n"
                "    assert False\n"
            ),
        }
    )
    return pytester


@pytest.fixture
def project_with_skipif(pytester):
    """Project with a @pytest.mark.skipif test."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": CALCULATOR_SRC,
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                SYS_PATH_PREAMBLE + "import pytest\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
                "\n"
                "@pytest.mark.skipif(True, reason='always skip')\n"
                "def test_skipif():\n"
                "    assert False\n"
            ),
        }
    )
    return pytester


@pytest.fixture
def project_with_skip_in_body(pytester):
    """Project with pytest.skip() called in test body."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": CALCULATOR_SRC,
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                SYS_PATH_PREAMBLE + "import pytest\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
                "\n"
                "def test_skip_in_body():\n"
                "    pytest.skip('skipping at runtime')\n"
            ),
        }
    )
    return pytester


@pytest.fixture
def project_with_xfail(pytester):
    """Project with a @pytest.mark.xfail test that fails as expected."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": CALCULATOR_SRC,
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                SYS_PATH_PREAMBLE + "import pytest\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
                "\n"
                "@pytest.mark.xfail(reason='known bug')\n"
                "def test_xfail():\n"
                "    assert add(1, 2) == 999\n"
            ),
        }
    )
    return pytester


def test_skip_marker_recorded_in_baseline(project_with_skip_marker):
    """@pytest.mark.skip test is recorded; second baseline run detects no changes."""
    result1 = project_with_skip_marker.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=1, skipped=1)

    result2 = project_with_skip_marker.runpytest_subprocess("--diff-baseline", "-v")
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*No changes detected*"])


def test_skipif_recorded_in_baseline(project_with_skipif):
    """@pytest.mark.skipif test is recorded similarly."""
    result1 = project_with_skipif.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=1, skipped=1)

    result2 = project_with_skipif.runpytest_subprocess("--diff-baseline", "-v")
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*No changes detected*"])


def test_skip_in_body_recorded_in_baseline(project_with_skip_in_body):
    """pytest.skip() called inside test body is recorded."""
    result1 = project_with_skip_in_body.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=1, skipped=1)

    result2 = project_with_skip_in_body.runpytest_subprocess("--diff-baseline", "-v")
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*No changes detected*"])


def test_xfail_recorded_in_baseline(project_with_xfail):
    """@pytest.mark.xfail (fails as expected) is recorded."""
    result1 = project_with_xfail.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=1, xfailed=1)

    result2 = project_with_xfail.runpytest_subprocess("--diff-baseline", "-v")
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*No changes detected*"])


def test_skip_xfail_deselected_in_diff_mode(pytester):
    """After baseline, --diff with no changes deselects skip/xfail tests."""
    pytester.makepyfile(
        **{
            "mylib/__init__.py": "",
            "mylib/calculator.py": CALCULATOR_SRC,
            "tests/__init__.py": "",
            "tests/test_calc.py": (
                SYS_PATH_PREAMBLE + "import pytest\n"
                "from mylib.calculator import add\n"
                "\n"
                "def test_add():\n"
                "    assert add(1, 2) == 3\n"
                "\n"
                "@pytest.mark.skip(reason='not ready')\n"
                "def test_skipped():\n"
                "    assert False\n"
                "\n"
                "@pytest.mark.xfail(reason='known bug')\n"
                "def test_xfail():\n"
                "    assert add(1, 2) == 999\n"
            ),
        }
    )

    # Baseline run
    result1 = pytester.runpytest_subprocess("--diff-baseline", "-v")
    result1.assert_outcomes(passed=1, skipped=1, xfailed=1)

    # Diff run with no changes — all tests should be deselected
    result2 = pytester.runpytest_subprocess("--diff", "-v")
    result2.assert_outcomes()
    result2.stdout.fnmatch_lines(["*Skipping all 3 tests*"])
