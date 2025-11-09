# Contributing to pytest-diff

Thank you for your interest in contributing to pytest-diff! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- [mise](https://mise.jdx.dev/) - Manages Python and Rust versions
- [uv](https://github.com/astral-sh/uv) - Fast Python package manager
- Git

### Setup Instructions

```bash
# Clone the repository
git clone https://github.com/paulmilesi/pytest-diff.git
cd pytest-diff

# Install mise (if not already installed)
curl https://mise.run | sh

# Install Python and Rust via mise
mise install

# Install Python dependencies with uv
uv sync --all-extras --dev

# Build the Rust extension
maturin develop

# Verify setup by running tests
pytest
cargo test
```

## Development Workflow

### Making Changes

1. Create a new branch for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes in the appropriate files:
   - **Rust code**: `rust/src/`
   - **Python code**: `python/pytest_diff/`
   - **Tests**: `rust/tests/` or `python/tests/`

3. After modifying Rust code, rebuild:
   ```bash
   maturin develop
   ```

### Running Tests

```bash
# Python tests
pytest

# Rust tests
cargo test

# Run with coverage
pytest --cov=pytest_diff

# Run specific test
pytest python/tests/test_basic.py::test_import_module
```

### Code Formatting

```bash
# Format Rust code
cargo fmt

# Format Python code
ruff format python/

# Check Python formatting
ruff check python/
```

### Linting

```bash
# Rust linting
cargo clippy -- -D warnings

# Python linting
ruff check python/
```

## Code Style

### Rust

- Follow standard Rust formatting (`cargo fmt`)
- Pass all `clippy` lints
- Add documentation comments (`///`) for public APIs
- Include unit tests in the same file under `#[cfg(test)]`

### Python

- Follow [PEP 8](https://pep8.org/)
- Use type hints where appropriate
- Add docstrings for public functions and classes
- Line length: 100 characters

## Testing Guidelines

### Rust Tests

- Unit tests go in the same file as the code under `#[cfg(test)]`
- Integration tests go in `rust/tests/`
- Use `proptest` for property-based testing where appropriate

### Python Tests

- Use pytest fixtures for setup/teardown
- Test both normal and error cases
- Mock external dependencies when appropriate

## Documentation

- Update README.md for user-facing changes
- Add docstrings/comments for new APIs
- Update architecture docs if making structural changes

## Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
feat: add support for Python 3.13
fix: correct checksum calculation for async functions
docs: update installation instructions
test: add tests for nested function parsing
refactor: simplify fingerprint cache logic
perf: optimize database query batching
```

## Pull Request Process

1. Update tests to cover your changes
2. Ensure all tests pass locally
3. Update documentation as needed
4. Create a pull request with a clear description
5. Wait for CI checks to pass
6. Address any review feedback

## Release Process

(For maintainers)

1. Update version in:
   - `Cargo.toml`
   - `pyproject.toml`
   - `python/pytest_diff/__init__.py`

2. Update CHANGELOG.md

3. Create a git tag:
   ```bash
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

4. GitHub Actions will automatically build and publish to PyPI

## Questions?

- Open an issue for bugs or feature requests
- Start a discussion for questions or ideas
- Reach out to maintainers directly for sensitive issues

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
