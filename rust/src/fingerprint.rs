// Fingerprinting and change detection
//
// This module handles:
// - Calculating file fingerprints (file hash + block checksums)
// - Detecting which files have changed
// - Identifying which specific blocks changed

use anyhow::{Context, Result};
use pyo3::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

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
/// # Arguments
/// * `db_path` - Path to the .testmondata database
/// * `project_root` - Root directory of the project
///
/// # Returns
/// * ChangedFiles containing list of modified files and changed blocks
#[pyfunction]
pub fn detect_changes(_db_path: &str, _project_root: &str) -> PyResult<ChangedFiles> {
    // TODO: Implement full change detection with database integration
    // For now, return empty changes (stub implementation)

    Ok(ChangedFiles {
        modified: vec![],
        changed_blocks: HashMap::new(),
    })
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
