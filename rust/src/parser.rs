// Python AST parser using Ruff's parser
//
// This module parses Python source code and extracts code blocks
// (functions, classes, modules) with their checksums.

use anyhow::{Context, Result};
use crc32fast::Hasher;
use pyo3::prelude::*;
use ruff_python_ast::{Mod, Stmt, StmtAsyncFunctionDef, StmtClassDef, StmtFunctionDef};
use ruff_python_parser::{parse, Mode};
use ruff_text_size::TextRange;

use crate::types::Block;

/// Parse a Python module and extract all code blocks
///
/// # Arguments
/// * `source` - Python source code as a string
///
/// # Returns
/// * `PyResult<Vec<Block>>` - List of blocks found in the source
///
/// # Example
/// ```python
/// blocks = parse_module("def foo(): pass")
/// assert len(blocks) == 2  # module + function
/// ```
#[pyfunction]
pub fn parse_module(source: &str) -> PyResult<Vec<Block>> {
    let blocks = parse_module_internal(source).map_err(|e| {
        pyo3::exceptions::PySyntaxError::new_err(format!("Failed to parse Python code: {}", e))
    })?;

    Ok(blocks)
}

/// Internal implementation that returns anyhow::Result
fn parse_module_internal(source: &str) -> Result<Vec<Block>> {
    // Parse the source code with Ruff's parser
    let parsed = parse(source, Mode::Module).context("Failed to parse Python source")?;

    let mut blocks = Vec::new();

    // Add module-level block (entire file)
    let module_checksum = calculate_checksum(source);
    blocks.push(Block {
        start_line: 1,
        end_line: source.lines().count(),
        checksum: module_checksum,
        name: "<module>".to_string(),
        block_type: "module".to_string(),
    });

    // Extract blocks from AST
    match parsed {
        Mod::Module(module) => {
            extract_blocks_from_statements(&module.body, source, &mut blocks)?;
        }
        Mod::Expression(_) => {
            // Single expression, already covered by module block
        }
    }

    Ok(blocks)
}

/// Recursively extract blocks from a list of statements
fn extract_blocks_from_statements(
    statements: &[Stmt],
    source: &str,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    for stmt in statements {
        extract_block_from_statement(stmt, source, blocks)?;
    }
    Ok(())
}

/// Extract a block from a single statement
fn extract_block_from_statement(stmt: &Stmt, source: &str, blocks: &mut Vec<Block>) -> Result<()> {
    match stmt {
        Stmt::FunctionDef(func) => {
            extract_function_block(func, source, blocks, "function")?;
        }
        Stmt::AsyncFunctionDef(func) => {
            extract_async_function_block(func, source, blocks)?;
        }
        Stmt::ClassDef(class) => {
            extract_class_block(class, source, blocks)?;
        }
        // Other statement types don't create blocks but may contain nested blocks
        Stmt::If(if_stmt) => {
            extract_blocks_from_statements(&if_stmt.body, source, blocks)?;
            extract_blocks_from_statements(&if_stmt.elif_else_clauses, source, blocks)?;
        }
        Stmt::For(for_stmt) => {
            extract_blocks_from_statements(&for_stmt.body, source, blocks)?;
            extract_blocks_from_statements(&for_stmt.orelse, source, blocks)?;
        }
        Stmt::While(while_stmt) => {
            extract_blocks_from_statements(&while_stmt.body, source, blocks)?;
            extract_blocks_from_statements(&while_stmt.orelse, source, blocks)?;
        }
        Stmt::With(with_stmt) => {
            extract_blocks_from_statements(&with_stmt.body, source, blocks)?;
        }
        Stmt::Try(try_stmt) => {
            extract_blocks_from_statements(&try_stmt.body, source, blocks)?;
            for handler in &try_stmt.handlers {
                extract_blocks_from_statements(&handler.body, source, blocks)?;
            }
            extract_blocks_from_statements(&try_stmt.orelse, source, blocks)?;
            extract_blocks_from_statements(&try_stmt.finalbody, source, blocks)?;
        }
        _ => {}
    }
    Ok(())
}

/// Extract a function definition as a block
fn extract_function_block(
    func: &StmtFunctionDef,
    source: &str,
    blocks: &mut Vec<Block>,
    block_type: &str,
) -> Result<()> {
    let range = func.range;
    let (start_line, end_line) = range_to_lines(range, source);
    let block_source = extract_source_range(source, range)?;
    let checksum = calculate_checksum(&block_source);

    blocks.push(Block {
        start_line,
        end_line,
        checksum,
        name: func.name.to_string(),
        block_type: block_type.to_string(),
    });

    // Extract nested blocks from function body
    extract_blocks_from_statements(&func.body, source, blocks)?;

    Ok(())
}

/// Extract an async function definition as a block
fn extract_async_function_block(
    func: &StmtAsyncFunctionDef,
    source: &str,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    let range = func.range;
    let (start_line, end_line) = range_to_lines(range, source);
    let block_source = extract_source_range(source, range)?;
    let checksum = calculate_checksum(&block_source);

    blocks.push(Block {
        start_line,
        end_line,
        checksum,
        name: func.name.to_string(),
        block_type: "async_function".to_string(),
    });

    // Extract nested blocks from function body
    extract_blocks_from_statements(&func.body, source, blocks)?;

    Ok(())
}

/// Extract a class definition as a block
fn extract_class_block(
    class: &StmtClassDef,
    source: &str,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    let range = class.range;
    let (start_line, end_line) = range_to_lines(range, source);
    let block_source = extract_source_range(source, range)?;
    let checksum = calculate_checksum(&block_source);

    blocks.push(Block {
        start_line,
        end_line,
        checksum,
        name: class.name.to_string(),
        block_type: "class".to_string(),
    });

    // Extract nested blocks from class body (methods, nested classes)
    extract_blocks_from_statements(&class.body, source, blocks)?;

    Ok(())
}

/// Convert TextRange to (start_line, end_line)
fn range_to_lines(range: TextRange, source: &str) -> (usize, usize) {
    let start_offset = range.start().to_usize();
    let end_offset = range.end().to_usize();

    let start_line = source[..start_offset].lines().count();
    let end_line = source[..end_offset].lines().count();

    // Lines are 1-indexed
    (start_line.max(1), end_line.max(1))
}

/// Extract source code for a given TextRange
fn extract_source_range(source: &str, range: TextRange) -> Result<String> {
    let start = range.start().to_usize();
    let end = range.end().to_usize();

    if end > source.len() {
        anyhow::bail!("Range end {} exceeds source length {}", end, source.len());
    }

    Ok(source[start..end].to_string())
}

/// Calculate CRC32 checksum for a string
///
/// Returns a signed i32 to match pytest-testmon's format
pub fn calculate_checksum(source: &str) -> i32 {
    let mut hasher = Hasher::new();
    hasher.update(source.as_bytes());
    hasher.finalize() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = r#"
def add(a, b):
    return a + b
"#;
        let blocks = parse_module_internal(source).unwrap();

        // Should have module + function
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "<module>");
        assert_eq!(blocks[1].name, "add");
        assert_eq!(blocks[1].block_type, "function");
    }

    #[test]
    fn test_parse_class_with_methods() {
        let source = r#"
class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b
"#;
        let blocks = parse_module_internal(source).unwrap();

        // Should have: module + class + 2 methods
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].name, "<module>");
        assert_eq!(blocks[1].name, "Calculator");
        assert_eq!(blocks[1].block_type, "class");
        assert_eq!(blocks[2].name, "add");
        assert_eq!(blocks[3].name, "subtract");
    }

    #[test]
    fn test_parse_async_function() {
        let source = r#"
async def fetch_data():
    return await get_data()
"#;
        let blocks = parse_module_internal(source).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].name, "fetch_data");
        assert_eq!(blocks[1].block_type, "async_function");
    }

    #[test]
    fn test_checksum_stability() {
        let source = "def foo(): pass";
        let checksum1 = calculate_checksum(source);
        let checksum2 = calculate_checksum(source);

        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_changes_with_content() {
        let source1 = "def foo(): pass";
        let source2 = "def foo(): return 1";

        let checksum1 = calculate_checksum(source1);
        let checksum2 = calculate_checksum(source2);

        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_parse_nested_functions() {
        let source = r#"
def outer():
    def inner():
        pass
    return inner
"#;
        let blocks = parse_module_internal(source).unwrap();

        // Should have: module + outer + inner
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].name, "outer");
        assert_eq!(blocks[2].name, "inner");
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let source = "def foo(";
        let result = parse_module_internal(source);

        assert!(result.is_err());
    }
}
