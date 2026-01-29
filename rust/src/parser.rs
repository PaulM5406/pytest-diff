// Python AST parser using RustPython's parser
//
// This module parses Python source code and extracts code blocks
// (functions, classes, modules) with their checksums.

use anyhow::Result;
use crc32fast::Hasher;
use pyo3::prelude::*;
use rustpython_parser::{ast, Parse};
use rustpython_parser_core::source_code::RandomLocator;

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

/// Extract module-level skeleton (excludes function/class bodies)
///
/// This creates a simplified version of the source that includes:
/// - Imports
/// - Module docstrings
/// - Module-level constants/assignments
/// - Function/class signatures (but not their bodies)
///
/// This ensures the module checksum only changes when module-level code changes,
/// not when individual function implementations change.
fn extract_module_skeleton(source: &str, parsed: &[ast::Stmt], locator: &mut RandomLocator) -> Result<String> {
    use ast::Ranged;

    let source_lines: Vec<&str> = source.lines().collect();
    let mut skeleton_parts = Vec::new();

    for stmt in parsed {
        match stmt {
            // Function definitions: include signature only
            ast::Stmt::FunctionDef(_func_def) => {
                let start = get_line_number(locator, stmt.start());
                let end = get_line_number(locator, stmt.end());

                // Extract just the def line(s) - everything up to the colon that ends the signature
                // Handle multi-line signatures like:
                //   def foo(
                //       arg1: int,
                //       arg2: str,
                //   ) -> bool:
                if start <= source_lines.len() {
                    let def_lines = extract_signature_lines(&source_lines, start, end);
                    skeleton_parts.push(def_lines.join("\n"));
                }
            }

            // Async function definitions: include signature only
            ast::Stmt::AsyncFunctionDef(_async_func_def) => {
                let start = get_line_number(locator, stmt.start());
                let end = get_line_number(locator, stmt.end());

                if start <= source_lines.len() {
                    let def_lines = extract_signature_lines(&source_lines, start, end);
                    skeleton_parts.push(def_lines.join("\n"));
                }
            }

            // Class definitions: include signature only
            ast::Stmt::ClassDef(_class_def) => {
                let start = get_line_number(locator, stmt.start());
                let end = get_line_number(locator, stmt.end());

                if start <= source_lines.len() {
                    let def_lines = extract_signature_lines(&source_lines, start, end);
                    skeleton_parts.push(def_lines.join("\n"));
                }
            }

            // All other statements: include completely
            // This includes: imports, assignments, expressions, etc.
            _ => {
                let start = get_line_number(locator, stmt.start());
                let end = get_line_number(locator, stmt.end());

                if start <= source_lines.len() {
                    let stmt_source = extract_source_lines(source, start, end)?;
                    skeleton_parts.push(stmt_source);
                }
            }
        }
    }

    Ok(skeleton_parts.join("\n"))
}

/// Strip a trailing comment from a line of Python code.
///
/// Scans the line tracking string literal state (`'`, `"`) and returns the
/// slice before the first `#` that is not inside a string literal.
fn strip_trailing_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let bytes = line.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\\' && (in_single || in_double) {
            // Skip escaped character inside a string
            i += 2;
            continue;
        }
        if ch == b'\'' && !in_double {
            in_single = !in_single;
        } else if ch == b'"' && !in_single {
            in_double = !in_double;
        } else if ch == b'#' && !in_single && !in_double {
            return line[..i].trim_end();
        }
        i += 1;
    }
    line
}

/// Extract signature lines for a function/class definition
///
/// Handles multi-line signatures by tracking parenthesis/bracket depth
/// and stopping after the line that contains the final `:` at depth 0.
fn extract_signature_lines<'a>(source_lines: &[&'a str], start: usize, end: usize) -> Vec<&'a str> {
    let mut def_lines = Vec::new();
    let range_end = end.min(source_lines.len());

    // Track nesting depth for parentheses, brackets, braces
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;

    for line in &source_lines[(start - 1)..range_end] {
        def_lines.push(*line);

        // Update depth counts
        for ch in line.chars() {
            match ch {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
        }

        // Stop after the line with the colon when at depth 0
        // This handles both simple `def foo():` and complex multi-line signatures
        // Strip trailing comments first to avoid false positives like `@deco  # TODO:`
        let trimmed = line.trim_end();
        let code_part = strip_trailing_comment(trimmed);
        if code_part.ends_with(':') && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            break;
        }
    }

    def_lines
}

/// Internal implementation that returns anyhow::Result
///
/// This must be used instead of `parse_module` for any code running inside
/// Rayon parallel iterators, because the #[pyfunction] version creates PyErr
/// objects which require the GIL — causing a deadlock when called from worker
/// threads while the main Python thread holds the GIL.
pub(crate) fn parse_module_internal(source: &str) -> Result<Vec<Block>> {
    // Parse the source code with RustPython's parser
    let parsed =
        ast::Suite::parse(source, "<string>").map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    // Build a RandomLocator once for efficient offset-to-line lookups
    let mut locator = RandomLocator::new(source);

    let mut blocks = Vec::new();

    // Add module-level block (skeleton only - excludes function/class bodies)
    // This ensures that changing a function body doesn't invalidate the module checksum
    let module_skeleton = extract_module_skeleton(source, &parsed, &mut locator)?;
    let module_checksum = calculate_checksum(&module_skeleton);
    let line_count = source.lines().count();
    blocks.push(Block {
        start_line: 1,
        end_line: line_count.max(1),
        checksum: module_checksum,
        name: "<module>".to_string(),
        block_type: "module".to_string(),
    });

    // Extract blocks from AST
    extract_blocks_from_statements(&parsed, source, &mut blocks, &mut locator)?;

    Ok(blocks)
}

/// Recursively extract blocks from a list of statements
fn extract_blocks_from_statements(
    statements: &[ast::Stmt],
    source: &str,
    blocks: &mut Vec<Block>,
    locator: &mut RandomLocator,
) -> Result<()> {
    for stmt in statements {
        extract_block_from_statement(stmt, source, blocks, locator)?;
    }
    Ok(())
}

/// Extract a block from a single statement
fn extract_block_from_statement(
    stmt: &ast::Stmt,
    source: &str,
    blocks: &mut Vec<Block>,
    locator: &mut RandomLocator,
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
            extract_blocks_from_statements(&func_def.body, source, blocks, locator)?;
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

            extract_blocks_from_statements(&async_func_def.body, source, blocks, locator)?;
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

            extract_blocks_from_statements(&class_def.body, source, blocks, locator)?;
        }
        // Handle other statement types that may contain nested blocks
        ast::Stmt::If(if_stmt) => {
            extract_blocks_from_statements(&if_stmt.body, source, blocks, locator)?;
            extract_blocks_from_statements(&if_stmt.orelse, source, blocks, locator)?;
        }
        ast::Stmt::For(for_stmt) => {
            extract_blocks_from_statements(&for_stmt.body, source, blocks, locator)?;
            extract_blocks_from_statements(&for_stmt.orelse, source, blocks, locator)?;
        }
        ast::Stmt::While(while_stmt) => {
            extract_blocks_from_statements(&while_stmt.body, source, blocks, locator)?;
            extract_blocks_from_statements(&while_stmt.orelse, source, blocks, locator)?;
        }
        ast::Stmt::With(with_stmt) => {
            extract_blocks_from_statements(&with_stmt.body, source, blocks, locator)?;
        }
        ast::Stmt::Try(try_stmt) => {
            extract_blocks_from_statements(&try_stmt.body, source, blocks, locator)?;
            for handler in &try_stmt.handlers {
                match handler {
                    ast::ExceptHandler::ExceptHandler(h) => {
                        extract_blocks_from_statements(&h.body, source, blocks, locator)?;
                    }
                }
            }
            extract_blocks_from_statements(&try_stmt.orelse, source, blocks, locator)?;
            extract_blocks_from_statements(&try_stmt.finalbody, source, blocks, locator)?;
        }
        _ => {}
    }
    Ok(())
}

/// Convert TextSize to 1-indexed line number
fn get_line_number(
    locator: &mut RandomLocator,
    offset: rustpython_parser_core::text_size::TextSize,
) -> usize {
    let location = locator.locate(offset);
    location.row.get() as usize // Convert OneIndexed u32 to usize
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
        assert!(blocks
            .iter()
            .any(|b| b.name == "Calculator" && b.block_type == "class"));
        assert!(blocks
            .iter()
            .any(|b| b.name == "add" && b.block_type == "function"));
        assert!(blocks
            .iter()
            .any(|b| b.name == "subtract" && b.block_type == "function"));
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
    fn test_multiline_signature_with_comment_colon() {
        // A multi-line signature where an intermediate line has a trailing
        // comment ending in `:` should NOT cause premature termination.
        let source = r#"
def foo(
    a,  # see also dict:
    b,
):
    pass
"#;
        let blocks = parse_module_internal(source).unwrap();

        let func = blocks.iter().find(|b| b.name == "foo").unwrap();
        assert_eq!(func.block_type, "function");
        assert_eq!(func.start_line, 2);
        assert_eq!(func.end_line, 6);
    }

    #[test]
    fn test_extract_signature_with_comment_colon() {
        // Directly test that extract_signature_lines doesn't stop at a comment colon
        let lines = vec![
            "def foo(",
            "    a,  # note:",
            "    b,",
            "):",
            "    pass",
        ];
        let sig = extract_signature_lines(&lines, 1, 5);
        // Should include lines up to `):`
        assert_eq!(sig.len(), 4);
        assert_eq!(sig[3], "):");
    }

    #[test]
    fn test_strip_trailing_comment() {
        assert_eq!(strip_trailing_comment("code  # comment"), "code");
        assert_eq!(strip_trailing_comment("no comment"), "no comment");
        assert_eq!(strip_trailing_comment("'#' not a comment"), "'#' not a comment");
        assert_eq!(strip_trailing_comment("\"#\" not a comment"), "\"#\" not a comment");
        assert_eq!(strip_trailing_comment("x = 1  # TODO:"), "x = 1");
        assert_eq!(strip_trailing_comment("@deco  # note:"), "@deco");
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let source = "def foo(";
        let result = parse_module_internal(source);

        assert!(result.is_err());
    }
}
