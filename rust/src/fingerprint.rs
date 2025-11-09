// Fingerprinting and change detection
//
// This module handles:
// - Calculating file fingerprints (file hash + block checksums)
// - Detecting which files have changed
// - Identifying which specific blocks changed

use anyhow::{Context, Result};
use pyo3::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

use crate::database::TestmonDatabase;
use crate::parser::parse_module;
use crate::types::{ChangedFiles, Fingerprint};

/// Calculate fingerprint for a single Python file
///
/// # Arguments
/// * `path` - Path to the Python file
///
/// # Returns
/// * Fingerprint containing blocks, checksums, hash, and mtime
#[pyfunction]
pub fn calculate_fingerprint(path: &str) -> PyResult<Fingerprint> {
    let fingerprint = calculate_fingerprint_internal(path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to calculate fingerprint: {}", e))
    })?;

    Ok(fingerprint)
}

fn calculate_fingerprint_internal(path: &str) -> Result<Fingerprint> {
    let path = Path::new(path);

    // Read file content
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    // Calculate file-level hash using Blake3 (fast!)
    let file_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    // Parse and extract blocks
    let blocks = parse_module(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse Python file: {}", e))?;

    // Extract checksums
    let checksums: Vec<i32> = blocks.iter().map(|b| b.checksum).collect();

    // Get modification time
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;
    let mtime = metadata
        .modified()
        .with_context(|| "Failed to get modification time")?
        .duration_since(UNIX_EPOCH)
        .with_context(|| "Invalid modification time")?
        .as_secs_f64();

    Ok(Fingerprint {
        filename: path.to_string_lossy().to_string(),
        checksums,
        file_hash,
        mtime,
        blocks: Some(blocks),
    })
}

/// Detect changes between current filesystem state and database
///
/// Uses three-level change detection for optimal performance:
/// 1. mtime check (fastest - file modification time)
/// 2. file hash check (fast - blake3 hash of entire file)
/// 3. block checksum comparison (precise - per-function/class checksums)
///
/// # Arguments
/// * `db_path` - Path to the .testmondata database
/// * `project_root` - Root directory of the project
///
/// # Returns
/// * ChangedFiles containing list of modified files and changed blocks
#[pyfunction]
pub fn detect_changes(db_path: &str, project_root: &str) -> PyResult<ChangedFiles> {
    let changes = detect_changes_internal(db_path, project_root).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to detect changes: {}", e))
    })?;

    Ok(changes)
}

fn detect_changes_internal(db_path: &str, project_root: &str) -> Result<ChangedFiles> {
    // Open database
    let db = TestmonDatabase::open(db_path)?;

    // Find all Python files in the project
    let python_files = find_python_files(project_root)?;

    // Process files sequentially
    // TODO: Can optimize with parallel processing later by using multiple database connections
    let changed_entries: Vec<_> = python_files
        .iter()
        .filter_map(|path| {
            match check_file_changed(&db, path) {
                Ok(Some(change)) => Some(change),
                Ok(None) => None,
                Err(e) => {
                    eprintln!("Warning: Failed to check {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    // Separate modified files from changed blocks
    let mut modified = Vec::new();
    let mut changed_blocks = HashMap::new();

    for (file, blocks) in changed_entries {
        modified.push(file.clone());
        if !blocks.is_empty() {
            changed_blocks.insert(file, blocks);
        }
    }

    Ok(ChangedFiles {
        modified,
        changed_blocks,
    })
}

/// Find all Python files in a directory
fn find_python_files(root: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories and common non-source directories
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "__pycache__" && name != "node_modules"
        })
    {
        let entry = entry?;
        let path = entry.path();

        // Only include .py files
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("py") {
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

/// Check if a file has changed using three-level detection
///
/// Returns Some((filename, changed_checksums)) if changed, None if unchanged
fn check_file_changed(
    db: &TestmonDatabase,
    path: &Path,
) -> Result<Option<(String, Vec<i32>)>> {
    let filename = path.to_string_lossy().to_string();

    // Get stored fingerprint from database
    let stored_fp = match db.get_fingerprint_rust(&filename)? {
        Some(fp) => fp,
        None => {
            // File not in database - it's new
            return Ok(Some((filename, vec![])));
        }
    };

    // Level 1: mtime check (fastest)
    let metadata = std::fs::metadata(path)?;
    let current_mtime = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs_f64();

    if (current_mtime - stored_fp.mtime).abs() < 0.001 {
        // mtime unchanged - file definitely not modified
        return Ok(None);
    }

    // Level 2: file hash check (fast)
    let content = std::fs::read_to_string(path)?;
    let current_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    if current_hash == stored_fp.file_hash {
        // Hash unchanged - content is identical (mtime changed but not content)
        return Ok(None);
    }

    // Level 3: block checksum comparison (precise)
    let current_blocks = parse_module(&content)
        .map_err(|e| anyhow::anyhow!("Parse error in {}: {}", filename, e))?;

    let current_checksums: Vec<i32> = current_blocks.iter().map(|b| b.checksum).collect();

    if current_checksums == stored_fp.checksums {
        // Checksums unchanged - semantically equivalent (e.g., only whitespace/comments changed)
        return Ok(None);
    }

    // Find which specific blocks changed
    let changed_checksums = find_changed_checksums(&stored_fp.checksums, &current_checksums);

    Ok(Some((filename, changed_checksums)))
}

/// Find which checksums changed by comparing old vs new
fn find_changed_checksums(old_checksums: &[i32], new_checksums: &[i32]) -> Vec<i32> {
    // Find checksums that are in new but not in old, or vice versa
    let old_set: std::collections::HashSet<i32> = old_checksums.iter().copied().collect();
    let new_set: std::collections::HashSet<i32> = new_checksums.iter().copied().collect();

    // Return checksums that are different (added or removed)
    new_set
        .symmetric_difference(&old_set)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_calculate_fingerprint() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "def foo(): pass").unwrap();
        file.flush().unwrap();

        let path = file.path().to_str().unwrap();
        let fingerprint = calculate_fingerprint_internal(path).unwrap();

        assert_eq!(fingerprint.filename, path);
        assert_eq!(fingerprint.checksums.len(), 2); // module + function
        assert!(!fingerprint.file_hash.is_empty());
        assert!(fingerprint.mtime > 0.0);
    }

    #[test]
    fn test_fingerprint_hash_stability() {
        let mut file = NamedTempFile::new().unwrap();
        let source = "def add(a, b):\n    return a + b\n";
        writeln!(file, "{}", source).unwrap();
        file.flush().unwrap();

        let path = file.path().to_str().unwrap();

        let fp1 = calculate_fingerprint_internal(path).unwrap();
        let fp2 = calculate_fingerprint_internal(path).unwrap();

        assert_eq!(fp1.file_hash, fp2.file_hash);
        assert_eq!(fp1.checksums, fp2.checksums);
    }
}
