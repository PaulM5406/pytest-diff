# Next Claude Session: Commands & Context

## Quick Start Command

```bash
cd /Users/paulmilesi/Repos/Perso/pytest-diff
```

Then say to Claude:
**"Continue with Phase 3: Fix the parser to work with rustpython-parser, then complete change detection"**

---

## Current Status

### ✅ Completed (Phase 1 & 2)
- Project structure and build system
- Core Rust types with PyO3 bindings
- **Full database layer** (520 LOC, 6 tests passing)
- SQLite schema (pytest-testmon compatible)
- In-memory caching with DashMap
- WAL mode + performance optimizations

### 🔄 In Progress (Phase 3)
- **Parser needs rustpython-parser API fix** (main blocker)
- Change detection implementation
- Fingerprint module integration

---

## Priority Tasks for Next Session

### 1. Fix Parser (HIGH - ~1-2 hours)

**File**: `rust/src/parser.rs`

**Problem**: Code was written for Ruff's parser API (not on crates.io). Now using rustpython-parser which has different AST structure.

**What needs fixing**:

```rust
// CURRENT (broken):
match &stmt.node {
    ast::StmtKind::FunctionDef { name, body, .. } => {
        let start = stmt.location.row();
    }
}

// NEED TO CHANGE TO (check rustpython docs):
match stmt {
    ast::Stmt::FunctionDef(func_def) => {
        let name = &func_def.name;
        let body = &func_def.body;
        let start = func_def.location.row();  // or whatever the API is
    }
}
```

**Steps**:
1. Check rustpython-parser API: https://docs.rs/rustpython-parser/
2. Look at `rustpython_parser::ast::Stmt` enum
3. Update pattern matching in `extract_block_from_statement()`
4. Update location access
5. Test: `cargo test --lib parser::tests`

**Success Criteria**: All parser tests passing

---

### 2. Complete Change Detection (HIGH)

**File**: `rust/src/fingerprint.rs`

**Implement**: `detect_changes()` function

```rust
pub fn detect_changes(
    db_path: &str,
    project_root: &str,
) -> PyResult<ChangedFiles> {
    // 1. Scan project for Python files
    // 2. For each file:
    //    a. Check mtime (fast)
    //    b. If changed, check file hash
    //    c. If hash changed, parse and compare checksums
    // 3. Build ChangedFiles result
}
```

**Use**:
- `walkdir` or `ignore` crate for file scanning
- `rayon` for parallel processing
- Database methods: `db.get_fingerprint()`

---

### 3. Integration Testing (MEDIUM)

Once parser + change detection work:

```bash
# Build Rust extension
maturin develop

# Run Python tests
pytest python/tests/

# Run Rust tests
cargo test

# Test manually
python -c "
from pytest_diff import _core
blocks = _core.parse_module('def foo(): pass')
print(blocks)
"
```

---

## Helpful Commands

### Development Workflow

```bash
# Check what changed since last commit
git status

# Run Rust tests
cargo test --lib

# Run specific test
cargo test --lib parser::tests::test_parse_simple_function

# Check for compilation errors
cargo check

# Format code
cargo fmt

# Build Python extension
maturin develop

# Run Python tests
pytest python/tests/

# See test output
cargo test -- --nocapture
```

### Debugging

```bash
# Check rustpython-parser API
cargo doc --open

# Search for specific symbol
cargo search rustpython

# Check dependency tree
cargo tree | grep rustpython
```

---

## Context for Claude

### What We're Building
pytest-diff: A Rust-powered pytest plugin for intelligent test selection (10-30x faster than pytest-testmon)

### Architecture
```
Python pytest plugin → PyO3 bindings → Rust core
  ├── Parser (rustpython-parser) - NEEDS FIX
  ├── Database (SQLite + cache) - ✅ DONE
  ├── Fingerprinting - Partially done
  └── Change detection - TODO
```

### Why It's Fast
1. Rust AST parsing (10-50x faster than Python)
2. Optimized SQLite (WAL, mmap, caching)
3. Parallel file processing (rayon)
4. In-memory caching (DashMap)

---

## Files to Focus On

### Primary
1. `rust/src/parser.rs` - Fix rustpython API
2. `rust/src/fingerprint.rs` - Complete detect_changes()
3. `rust/src/database.rs` - Already done, may need integration tweaks

### Secondary
4. `rust/src/lib.rs` - PyO3 module exports
5. `python/pytest_diff/plugin.py` - Python integration (later)

---

## Expected Timeline

| Task | Estimated Time |
|------|----------------|
| Fix parser | 1-2 hours |
| Complete change detection | 2-3 hours |
| Integration testing | 1 hour |
| **Total** | **4-6 hours** |

---

## Success Metrics

### Phase 3 Complete When:
- [ ] All Rust tests passing (`cargo test`)
- [ ] Parser works with Python 3.8-3.13 syntax
- [ ] `detect_changes()` correctly identifies changed files
- [ ] Change detection uses 3 levels (mtime → hash → checksum)
- [ ] Python extension builds (`maturin develop`)
- [ ] Basic integration test works

---

## Git Commits

We're on commit: `2388d2a` (Phase 2: Complete database layer)

Next commit should be:
```
Phase 3: Fix parser and implement change detection

- Updated parser for rustpython-parser API
- Implemented three-level change detection
- All tests passing
```

---

## Resources

- **rustpython-parser docs**: https://docs.rs/rustpython-parser/
- **rusqlite docs**: https://docs.rs/rusqlite/
- **PyO3 guide**: https://pyo3.rs/
- **Project README**: `/Users/paulmilesi/Repos/Perso/pytest-diff/README.md`
- **Implementation plan**: `docs/IMPLEMENTATION_PLAN.md`

---

## Quick Reference

```bash
# Project root
cd /Users/paulmilesi/Repos/Perso/pytest-diff

# Build tools (via mise)
mise install

# Python env
uv sync --all-extras --dev

# Build & test cycle
cargo test && maturin develop && pytest
```

---

**Generated**: 2025-01-09
**For**: Next Claude Code session
**Phase**: 3 (Change Detection)
**Priority**: Fix parser, then detect_changes()
