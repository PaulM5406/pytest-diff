# pytest-difftest

**Fast test selection for pytest** - Only run tests affected by your changes, powered by Rust.

[![CI](https://github.com/PaulM5406/pytest-difftest/workflows/CI/badge.svg)](https://github.com/PaulM5406/pytest-difftest/actions)
[![PyPI](https://img.shields.io/pypi/v/pytest-difftest.svg)](https://pypi.org/project/pytest-difftest/)
[![Python Versions](https://img.shields.io/pypi/pyversions/pytest-difftest.svg)](https://pypi.org/project/pytest-difftest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

pytest-difftest tracks which tests touch which code blocks using coverage data, then uses Rust-powered AST parsing to detect changes at function/class granularity and select only the affected tests.

**Features:** block-level change detection, incremental baselines, pytest-xdist support, S3 remote storage, portable baselines (relative paths).

```bash
pip install pytest-difftest
pytest --diff-baseline  # Build baseline (first time)
pytest --diff           # Run only affected tests
```

## Installation

```bash
pip install pytest-difftest
```

For S3 remote storage support:

```bash
pip install pytest-difftest[s3]
```

## Quick Start

```bash
# 1. Build a baseline (runs all tests, records coverage)
pytest --diff-baseline

# 2. Make code changes, then run only affected tests
pytest --diff

# 3. Update baseline incrementally (only re-runs affected tests)
pytest --diff-baseline

# 4. Force a full baseline rebuild
pytest --diff-baseline --diff-force
```

## How It Works

1. **Baseline** (`--diff-baseline`) - Runs tests with coverage, builds a dependency graph mapping tests to code blocks. Stored in `.pytest_cache/pytest-difftest/pytest_difftest.db`. Subsequent runs are incremental.
2. **Change Detection** (`--diff`) - Parses modified files with Rust, computes block-level checksums, compares against stored fingerprints.
3. **Test Selection** - Skips collecting unchanged test files entirely, queries the database for tests depending on changed blocks, runs only those.

## Test Selection Behavior

| Scenario | `--diff` | `--diff-baseline` |
|----------|----------|-------------------|
| No changes | Skips all tests | Skips all tests (incremental) |
| Modified source file | Runs tests depending on changed blocks | Runs affected tests, updates baseline |
| New test/source file | Runs tests in/depending on the new file | Adds to baseline |
| Failing tests | Always re-selected | Re-run until they pass |
| Skipped / xfail tests | Deselected (recorded in baseline) | Recorded, deselected on incremental |
| First run (empty DB) | Runs all tests | Runs all tests |
| `--diff-force` | N/A | Full rebuild, re-runs all tests |

## Configuration

### Command Line Options

| Option | Description |
|--------|-------------|
| `--diff` | Run only tests affected by changes |
| `--diff-baseline` | Build/update baseline (first run: all tests; subsequent: incremental) |
| `--diff-force` | Force full baseline rebuild (with `--diff-baseline`) |
| `--diff-v` | Verbose logging |
| `--diff-batch-size N` | DB write batch size (default: 20) |
| `--diff-cache-size N` | Max fingerprints cached in memory (default: 100000) |
| `--diff-remote URL` | Remote baseline URL (e.g. `s3://bucket/baseline.db`) |
| `--diff-upload` | Upload baseline to remote after `--diff-baseline` |

### pyproject.toml

```toml
[tool.pytest.ini_options]
diff_batch_size = "50"
diff_cache_size = "200000"
diff_remote_url = "s3://my-ci-bucket/baselines/baseline.db"
```

CLI options override `pyproject.toml` values.

## Remote Baseline Storage

Share baselines between CI and developers using remote storage.

| Scheme | Backend | Requirements |
|--------|---------|-------------|
| `s3://bucket/path/file.db` | Amazon S3 | `pytest-difftest[s3]` |
| `file:///path/to/file.db` | Local filesystem | None |

**Basic workflow:**

```bash
# CI (on merge to main)
pytest --diff-baseline --diff-upload --diff-remote "s3://bucket/baseline.db"

# Developer (auto-fetches latest baseline)
pytest --diff --diff-remote "s3://bucket/baseline.db"
```

S3 uses ETag-based caching. Any S3 error aborts the run immediately to avoid silently running without a baseline.

**Parallel CI workflow:**

```bash
# Each CI job uploads its own baseline
pytest --diff-baseline --diff-upload --diff-remote "s3://bucket/run-123/job-unit.db"
pytest --diff-baseline --diff-upload --diff-remote "s3://bucket/run-123/job-integration.db"

# Final step merges and uploads
pytest-difftest merge s3://bucket/baseline.db s3://bucket/run-123/
```

### CLI: `pytest-difftest merge`

```bash
# Merge local files
pytest-difftest merge output.db input1.db input2.db

# Merge from directory (all .db files)
pytest-difftest merge output.db ./results/

# Merge from S3 prefix
pytest-difftest merge output.db s3://bucket/run-123/

# Full remote: download, merge, upload
pytest-difftest merge s3://bucket/baseline.db s3://bucket/run-123/
```

Output and inputs can be local paths, directories, or remote URLs. Directories collect all `.db` files; remote prefixes ending with `/` download all `.db` files.

## Development

### Prerequisites

- [mise](https://mise.jdx.dev/) (manages Python + Rust versions)
- [uv](https://github.com/astral-sh/uv) (Python package manager)

### Setup

```bash
git clone https://github.com/PaulM5406/pytest-difftest.git
cd pytest-difftest
mise install
uv sync --all-extras --dev
maturin develop
```

### Commands

```bash
maturin develop          # Rebuild Rust extension
pytest                   # Python tests
cargo test --lib         # Rust tests
cargo fmt && cargo clippy --lib -- -D warnings  # Rust lint
ruff check python/ && ruff format python/       # Python lint
ty check python/                                # Type check
```

## Credits

Inspired by [pytest-testmon](https://github.com/tarpas/pytest-testmon). Built with [Ruff's Python parser](https://github.com/astral-sh/ruff), [PyO3](https://github.com/PyO3/pyo3), and [Maturin](https://github.com/PyO3/maturin).

## License

[MIT](LICENSE)
