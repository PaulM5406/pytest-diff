# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [v0.3.0] - 2026-02-23

### Added

- `pytest-difftest inspect` CLI command: inspect database contents, query test dependencies and file dependents
- `get_test_dependencies()` and `get_file_dependents()` methods on `PytestDiffDatabase`

### Changed

- `__version__` now derived from `importlib.metadata` instead of hardcoded string
- Coverage is now treated as a required dependency (removed dead fallback code path)
- Hoisted `import json` and `import time` to module level in plugin.py

## [v0.2.0] - 2026-02-21

### Added

- Python 3.14 support (upgraded PyO3 from 0.23 to 0.25)
- Python 3.14 added to CI test matrix and release wheel builds

## [v0.1.2] - 2026-02-19

### Changed

- Reduced log verbosity: progress messages now only appear in verbose mode (`--diff-v`)
- Unified log prefix to `pytest-difftest:` in Rust output

### Fixed

- Include LICENSE file in sdist (fixes PyPI upload)
- Release workflow now verifies CI passed before publishing
