"""CLI commands for pytest-difftest."""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any


def _is_remote_url(path: str) -> bool:
    """Check if a path is a remote URL (s3://, file://)."""
    return path.startswith("s3://") or path.startswith("file://")


def _resolve_inputs(inputs: list[str]) -> tuple[list[str], Path | None]:
    """Resolve inputs into local .db file paths.

    Each input can be:
    - A local .db file path (e.g. input1.db)
    - A local directory (e.g. ./results/) — collects all .db files from it
    - A remote prefix ending with / (e.g. s3://bucket/run-123/) — downloads all .db files
    - A remote single file URL (e.g. s3://bucket/specific.db) — downloads that file

    Returns:
        (local_paths, temp_dir) — temp_dir is set if any remote files were downloaded.
    """
    from pytest_difftest._storage_ops import download_remote_databases

    local_paths: list[str] = []
    temp_dir: Path | None = None

    for input_path in inputs:
        if _is_remote_url(input_path):
            if temp_dir is None:
                temp_dir = Path(tempfile.mkdtemp(prefix="pytest_difftest_merge_"))
            remote_files = download_remote_databases(input_path, temp_dir)
            print(f"Downloaded {len(remote_files)} database(s) from {input_path}")
            local_paths.extend(str(f) for f in remote_files)
        elif Path(input_path).is_dir():
            db_files = sorted(Path(input_path).glob("*.db"))
            print(f"Found {len(db_files)} database(s) in {input_path}")
            local_paths.extend(str(f) for f in db_files)
        else:
            local_paths.append(input_path)

    return local_paths, temp_dir


def merge_databases(output: str, inputs: list[str]) -> int:
    """Merge multiple pytest-difftest databases into one.

    Args:
        output: Local path or remote URL (s3://...) for the merged database.
        inputs: List of input sources. Each can be a local path, a remote prefix
            (s3://bucket/prefix/) to download all .db files, or a remote single
            file URL (s3://bucket/file.db).

    Returns:
        Exit code (0 for success, 1 for failure).
    """
    from pytest_difftest._core import PytestDiffDatabase

    if not inputs:
        print("Error: At least one input database required", file=sys.stderr)
        return 1

    # Verify local inputs exist
    for input_path in inputs:
        if not _is_remote_url(input_path) and not Path(input_path).exists():
            print(f"Error: Input not found: {input_path}", file=sys.stderr)
            return 1

    # If output is a remote URL, use a temp file locally and upload at the end
    remote_output = output if _is_remote_url(output) else None
    if remote_output:
        tmp = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
        local_output = tmp.name
        tmp.close()
    else:
        local_output = output

    # Resolve remote inputs to local files
    temp_dir: Path | None = None
    try:
        local_inputs, temp_dir = _resolve_inputs(inputs)
    except Exception as e:
        print(f"Error: Failed to download remote inputs: {e}", file=sys.stderr)
        return 1

    if not local_inputs:
        print("Error: No .db files found in the provided inputs", file=sys.stderr)
        if temp_dir:
            shutil.rmtree(temp_dir, ignore_errors=True)
        return 1

    try:
        local_path = Path(local_output)
        db = PytestDiffDatabase(str(local_path))

        # Check for commit consistency before merging
        _check_merge_commit_consistency(db, local_inputs)

        total_baselines = 0
        total_tests = 0
        for input_path in local_inputs:
            result = db.merge_baseline_from(input_path)
            print(
                f"Merged {result.baseline_count} baselines"
                f" and {result.test_execution_count} test executions from {Path(input_path).name}"
            )
            total_baselines += result.baseline_count
            total_tests += result.test_execution_count

        db.close()
        print(f"Total: {total_baselines} baselines and {total_tests} test executions")

        # Upload if output is a remote URL
        if remote_output:
            from pytest_difftest._storage_ops import upload_to_remote

            try:
                upload_to_remote(remote_output, local_path)
                print(f"Uploaded merged database to {remote_output}")
            except Exception as e:
                print(f"Error: Failed to upload to {remote_output}: {e}", file=sys.stderr)
                return 1

        return 0
    finally:
        if temp_dir:
            shutil.rmtree(temp_dir, ignore_errors=True)
        if remote_output:
            Path(local_output).unlink(missing_ok=True)


def _check_merge_commit_consistency(db: Any, inputs: list[str]) -> None:
    """Check that all input databases have the same baseline_commit."""
    commits: dict[str, list[str]] = {}  # commit -> list of filenames

    for input_path in inputs:
        try:
            commit = db.get_external_metadata(input_path, "baseline_commit")
            if commit:
                commits.setdefault(commit, []).append(Path(input_path).name)
        except Exception:
            pass  # Silently skip if we can't read metadata

    if len(commits) > 1:
        details = ", ".join(f"{sha[:8]}({len(files)} files)" for sha, files in commits.items())
        print(
            f"Warning: Merging baselines from different commits: {details}. "
            "This may cause inconsistent test selection.",
            file=sys.stderr,
        )


def inspect_database(db_path: str, test: str | None, file: str | None) -> int:
    """Inspect a pytest-difftest database for diagnostic purposes.

    Args:
        db_path: Path to the pytest-difftest database.
        test: If set, show files this test depends on.
        file: If set, show tests that depend on this file.

    Returns:
        Exit code (0 for success, 1 for failure).
    """
    from pytest_difftest._core import PytestDiffDatabase

    if not Path(db_path).exists():
        print(f"Error: Database not found: {db_path}", file=sys.stderr)
        return 1

    db = PytestDiffDatabase(db_path)

    if test:
        files = db.get_test_dependencies(test)
        print(f"Test: {test}")
        print(f"Depends on {len(files)} file(s):")
        for f in files:
            print(f"  {f}")
    elif file:
        tests = db.get_file_dependents(file)
        print(f"File: {file}")
        print(f"Depended on by {len(tests)} test(s):")
        for t in tests:
            print(f"  {t}")
    else:
        # Summary mode
        stats = db.get_stats()
        print(f"Database: {db_path}")
        print(f"  Tests:        {stats.get('test_count', 0)}")
        print(f"  Files:        {stats.get('file_count', 0)}")
        print(f"  Fingerprints: {stats.get('fingerprint_count', 0)}")
        print(f"  Baselines:    {stats.get('baseline_count', 0)}")
        commit = db.get_metadata("baseline_commit")
        if commit:
            print(f"  Commit:       {commit[:10]}")
        scope = db.get_metadata("baseline_scope")
        if scope:
            print(f"  Scope:        {scope}")

    db.close()
    return 0


def main() -> int:
    """Main entry point for pytest-difftest CLI."""
    parser = argparse.ArgumentParser(
        prog="pytest-difftest",
        description="pytest-difftest command line tools",
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # merge command
    merge_parser = subparsers.add_parser(
        "merge",
        help="Merge multiple pytest-difftest databases into one",
        description="Merge multiple pytest-difftest databases into one. "
        "Output can be a local path or a remote URL (s3://...). "
        "Inputs can be local files, local directories, or remote URLs "
        "(prefix ending with / downloads all .db files). "
        "Usage: pytest-difftest merge output.db input1.db input2.db "
        "or: pytest-difftest merge s3://bucket/baseline.db s3://bucket/run-123/",
    )
    merge_parser.add_argument(
        "output",
        help="Output destination: local path or remote URL (e.g. s3://bucket/baseline.db)",
    )
    merge_parser.add_argument(
        "inputs",
        nargs="+",
        help="Input sources: local files, local directories (collects all .db files), "
        "or remote URLs (prefix ending with / downloads all .db files)",
    )

    # inspect command
    inspect_parser = subparsers.add_parser(
        "inspect",
        help="Inspect baseline database contents",
        description="Inspect a pytest-difftest database. "
        "Shows summary statistics, test dependencies, or file dependents.",
    )
    inspect_parser.add_argument("db_path", help="Path to the pytest-difftest database")
    inspect_parser.add_argument("--test", help="Show files this test depends on")
    inspect_parser.add_argument("--file", help="Show tests that depend on this file")

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        return 0

    if args.command == "merge":
        return merge_databases(args.output, args.inputs)

    if args.command == "inspect":
        return inspect_database(args.db_path, args.test, args.file)

    return 0


if __name__ == "__main__":
    sys.exit(main())
