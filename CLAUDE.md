# Project Rules

## Toolchain

- **mise** manages Python, Rust and uv versions (see `.mise.toml`). Run `mise install` to bootstrap.
- **uv** is the Python package manager. Use `uv sync --all-extras --dev` to install dependencies.
- **maturin** builds the Rust extension into the Python package. Run `maturin develop` after any Rust change.
- **ruff** is the Python linter and formatter (`ruff check python/`, `ruff format python/`).
- **ty** is the Python type checker (`ty check python/`).

## Verification

Always run these checks before considering work done:

```bash
# Rebuild Rust extension
maturin develop

# Python tests (37 tests)
pytest

# Rust tests
cargo test --lib

# Python lint + format + type check
ruff check python/
ruff format --check python/
ty check python/
```
