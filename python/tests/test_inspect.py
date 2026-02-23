"""Tests for the 'pytest-difftest inspect' CLI command."""

from pytest_difftest import _core
from pytest_difftest.cli import inspect_database


def test_inspect_summary_empty_db(tmp_path, capsys):
    """Summary on an empty database shows zero counts."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)
    db.close()

    rc = inspect_database(db_path, test=None, file=None)
    assert rc == 0

    out = capsys.readouterr().out
    assert "Tests:        0" in out
    assert "Files:        0" in out
    assert "Baselines:    0" in out


def test_inspect_summary_with_metadata(tmp_path, capsys):
    """Summary shows commit and scope when metadata is present."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)
    db.set_metadata("baseline_commit", "abc123def456")
    db.set_metadata("baseline_scope", '["tests/"]')
    db.close()

    rc = inspect_database(db_path, test=None, file=None)
    assert rc == 0

    out = capsys.readouterr().out
    assert "abc123def4" in out
    assert '["tests/"]' in out


def test_inspect_test_dependencies(tmp_path, capsys):
    """--test shows files the test depends on."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)

    fp = _core.calculate_fingerprint(_create_py_file(tmp_path, "module_a.py", "x = 1\n"))
    db.save_test_execution("tests/test_foo.py::test_bar", [fp], 0.1, False)
    db.close()

    rc = inspect_database(db_path, test="tests/test_foo.py::test_bar", file=None)
    assert rc == 0

    out = capsys.readouterr().out
    assert "tests/test_foo.py::test_bar" in out
    assert "Depends on 1 file(s)" in out


def test_inspect_file_dependents(tmp_path, capsys):
    """--file shows tests that depend on a file."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)

    py_file = _create_py_file(tmp_path, "src/models.py", "class Model:\n    pass\n")
    fp = _core.calculate_fingerprint(py_file)
    db.save_test_execution("tests/test_models.py::test_create", [fp], 0.2, False)
    db.save_test_execution("tests/test_models.py::test_delete", [fp], 0.3, False)
    db.close()

    rc = inspect_database(db_path, test=None, file=fp.filename)
    assert rc == 0

    out = capsys.readouterr().out
    assert "Depended on by 2 test(s)" in out
    assert "test_create" in out
    assert "test_delete" in out


def test_inspect_nonexistent_test(tmp_path, capsys):
    """--test with unknown test name shows 0 results, no error."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)
    db.close()

    rc = inspect_database(db_path, test="nonexistent::test", file=None)
    assert rc == 0

    out = capsys.readouterr().out
    assert "Depends on 0 file(s)" in out


def test_inspect_nonexistent_file(tmp_path, capsys):
    """--file with unknown filename shows 0 results, no error."""
    db_path = str(tmp_path / "test.db")
    db = _core.PytestDiffDatabase(db_path)
    db.close()

    rc = inspect_database(db_path, test=None, file="nonexistent.py")
    assert rc == 0

    out = capsys.readouterr().out
    assert "Depended on by 0 test(s)" in out


def test_inspect_nonexistent_db(tmp_path, capsys):
    """Nonexistent database path returns error code 1."""
    rc = inspect_database(str(tmp_path / "missing.db"), test=None, file=None)
    assert rc == 1

    err = capsys.readouterr().err
    assert "not found" in err


def _create_py_file(tmp_path, name, content):
    """Helper to create a .py file and return its path as string."""
    p = tmp_path / name
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content)
    return str(p)
