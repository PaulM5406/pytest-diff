// Python AST parser using RustPython's parser
//
// This module parses Python source code and extracts code blocks
// (functions, classes, modules) with their checksums.

use anyhow::Result;
use crc32fast::Hasher;
use pyo3::prelude::*;
use rustpython_parser::{ast, Parse};
use rustpython_parser_core::source_code::LinearLocator;

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
    // Parse the source code with RustPython's parser
    let parsed = ast::Suite::parse(source, "<string>")
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    let mut blocks = Vec::new();

    // Add module-level block (entire file)
    let module_checksum = calculate_checksum(source);
    let line_count = source.lines().count();
    blocks.push(Block {
        start_line: 1,
        end_line: line_count.max(1),
        checksum: module_checksum,
        name: "<module>".to_string(),
        block_type: "module".to_string(),
    });

    // Create a locator to convert TextRange to line numbers
    let mut locator = LinearLocator::new(source);

    // Extract blocks from AST
    extract_blocks_from_statements(&parsed, source, &mut locator, &mut blocks)?;

    Ok(blocks)
}

/// Recursively extract blocks from a list of statements
fn extract_blocks_from_statements(
    statements: &[ast::Stmt],
    source: &str,
    locator: &mut LinearLocator,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    for stmt in statements {
        extract_block_from_statement(stmt, source, locator, blocks)?;
    }
    Ok(())
}

/// Extract a block from a single statement
fn extract_block_from_statement(
    stmt: &ast::Stmt,
    source: &str,
    locator: &mut LinearLocator,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    use ast::Ranged; // Import trait to use range() method

    match stmt {
        ast::Stmt::FunctionDef(func_def) => {
            let start = get_line_number(locator, stmt.start());
            let end = get_line_number(locator, stmt.end());

            // Extract the source for this function
            let block_source = extract_source_lines(source, start, end)?;
            let checksum = calculate_checksum(&block_source);

            blocks.push(Block {
                start_line: start,
                end_line: end,
                checksum,
                name: func_def.name.to_string(),
                block_type: "function".to_string(),
            });

            // Extract nested blocks
            extract_blocks_from_statements(&func_def.body, source, locator, blocks)?;
        }
        ast::Stmt::AsyncFunctionDef(async_func_def) => {
            let start = get_line_number(locator, stmt.start());
            let end = get_line_number(locator, stmt.end());

            let block_source = extract_source_lines(source, start, end)?;
            let checksum = calculate_checksum(&block_source);

            blocks.push(Block {
                start_line: start,
                end_line: end,
                checksum,
                name: async_func_def.name.to_string(),
                block_type: "async_function".to_string(),
            });

            extract_blocks_from_statements(&async_func_def.body, source, locator, blocks)?;
        }
        ast::Stmt::ClassDef(class_def) => {
            let start = get_line_number(locator, stmt.start());
            let end = get_line_number(locator, stmt.end());

            let block_source = extract_source_lines(source, start, end)?;
            let checksum = calculate_checksum(&block_source);

            blocks.push(Block {
                start_line: start,
                end_line: end,
                checksum,
                name: class_def.name.to_string(),
                block_type: "class".to_string(),
            });

            extract_blocks_from_statements(&class_def.body, source, locator, blocks)?;
        }
        // Handle other statement types that may contain nested blocks
        ast::Stmt::If(if_stmt) => {
            extract_blocks_from_statements(&if_stmt.body, source, locator, blocks)?;
            extract_blocks_from_statements(&if_stmt.orelse, source, locator, blocks)?;
        }
        ast::Stmt::For(for_stmt) => {
            extract_blocks_from_statements(&for_stmt.body, source, locator, blocks)?;
            extract_blocks_from_statements(&for_stmt.orelse, source, locator, blocks)?;
        }
        ast::Stmt::While(while_stmt) => {
            extract_blocks_from_statements(&while_stmt.body, source, locator, blocks)?;
            extract_blocks_from_statements(&while_stmt.orelse, source, locator, blocks)?;
        }
        ast::Stmt::With(with_stmt) => {
            extract_blocks_from_statements(&with_stmt.body, source, locator, blocks)?;
        }
        ast::Stmt::Try(try_stmt) => {
            extract_blocks_from_statements(&try_stmt.body, source, locator, blocks)?;
            for handler in &try_stmt.handlers {
                match handler {
                    ast::ExceptHandler::ExceptHandler(h) => {
                        extract_blocks_from_statements(&h.body, source, locator, blocks)?;
                    }
                }
            }
            extract_blocks_from_statements(&try_stmt.orelse, source, locator, blocks)?;
            extract_blocks_from_statements(&try_stmt.finalbody, source, locator, blocks)?;
        }
        _ => {}
    }
    Ok(())
}

/// Convert TextSize to 1-indexed line number
fn get_line_number(locator: &mut LinearLocator, offset: rustpython_parser_core::text_size::TextSize) -> usize {
    let location = locator.locate(offset);
    location.row.get() as usize  // Convert OneIndexed u32 to usize
}

/// Extract source lines from start to end (inclusive, 1-indexed)
fn extract_source_lines(source: &str, start: usize, end: usize) -> Result<String> {
    let lines: Vec<&str> = source.lines().collect();

    if start < 1 || start > lines.len() {
        anyhow::bail!("Start line {} out of range (1-{})", start, lines.len());
    }

    let end = end.min(lines.len());

    Ok(lines[(start - 1)..end].join("\n"))
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
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].name, "<module>");

        // Find the function block
        let func_block = blocks.iter().find(|b| b.name == "add").unwrap();
        assert_eq!(func_block.block_type, "function");
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
        assert!(blocks.len() >= 4);
        assert!(blocks.iter().any(|b| b.name == "Calculator" && b.block_type == "class"));
        assert!(blocks.iter().any(|b| b.name == "add" && b.block_type == "function"));
        assert!(blocks.iter().any(|b| b.name == "subtract" && b.block_type == "function"));
    }

    #[test]
    fn test_parse_async_function() {
        let source = r#"
async def fetch_data():
    return await get_data()
"#;
        let blocks = parse_module_internal(source).unwrap();

        assert!(blocks.len() >= 2);
        let async_func = blocks.iter().find(|b| b.name == "fetch_data").unwrap();
        assert_eq!(async_func.block_type, "async_function");
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
        assert!(blocks.len() >= 3);
        assert!(blocks.iter().any(|b| b.name == "outer"));
        assert!(blocks.iter().any(|b| b.name == "inner"));
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let source = "def foo(";
        let result = parse_module_internal(source);

        assert!(result.is_err());
    }
}
