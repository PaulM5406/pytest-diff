// Fingerprinting and change detection
//
// This module handles:
// - Calculating file fingerprints (file hash + block checksums)
// - Detecting which files have changed
// - Identifying which specific blocks changed
// - Processing coverage data with concurrent block filtering

use anyhow::{Context, Result};
use pyo3::prelude::*;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

use crate::database::TestmonDatabase;
use crate::parser::parse_module;
use crate::types::{Block, ChangedFiles, Fingerprint};

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

pub(crate) fn calculate_fingerprint_internal(path: &str) -> Result<Fingerprint> {
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

/// Save baseline fingerprints for all Python files in a project
///
/// This establishes the "known good" state that change detection compares against.
/// Should be called after tests pass to set the baseline.
///
/// # Arguments
/// * `db_path` - Path to the .testmondata database
/// * `project_root` - Root directory of the project
/// * `verbose` - Whether to print debug information
/// * `scope_paths` - List of directory paths to limit the scope (e.g., ["tests/unit/"])
///
/// # Returns
/// * Number of files added to baseline
#[pyfunction]
pub fn save_baseline(db_path: &str, project_root: &str, verbose: bool, scope_paths: Vec<String>) -> PyResult<usize> {
    let count = save_baseline_internal(db_path, project_root, verbose, scope_paths).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to save baseline: {}", e))
    })?;

    Ok(count)
}

fn save_baseline_internal(db_path: &str, project_root: &str, verbose: bool, scope_paths: Vec<String>) -> Result<usize> {
    use std::time::Instant;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let start = Instant::now();
    let mut db = TestmonDatabase::open(db_path)?;
    if verbose {
        eprintln!("[rust] Database opened in {:.3}s", start.elapsed().as_secs_f64());
    }

    let find_start = Instant::now();
    let python_files = find_python_files(project_root, &scope_paths)?;
    if verbose {
        eprintln!("[rust] Found {} Python files in {:.3}s", python_files.len(), find_start.elapsed().as_secs_f64());
    }

    let processing_start = Instant::now();
    let total_files = python_files.len();

    // Progress counter for parallel processing
    let progress_counter = Arc::new(AtomicUsize::new(0));

    if verbose {
        eprintln!("[rust] Calculating fingerprints in parallel...");
    }

    // PARALLEL: Calculate fingerprints for all files concurrently
    let fp_calc_start = Instant::now();
    let fingerprints: Vec<(String, Result<Fingerprint>)> = python_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();

            // Update progress counter
            if verbose {
                let count = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if count % 50 == 0 {
                    eprintln!("[rust] Fingerprinted {}/{} files ({:.1}%)",
                             count, total_files, count as f64 / total_files as f64 * 100.0);
                }
            }

            let fp_start = Instant::now();
            let result = calculate_fingerprint_internal(&path_str);

            // Log slow files
            if verbose && fp_start.elapsed().as_millis() > 100 {
                eprintln!("[rust]   Fingerprint for {} took {:.3}s",
                         path.file_name().unwrap_or_default().to_string_lossy(),
                         fp_start.elapsed().as_secs_f64());
            }

            (path_str, result)
        })
        .collect();

    if verbose {
        eprintln!("[rust] Fingerprinted {} files in {:.3}s (parallel)",
                 total_files, fp_calc_start.elapsed().as_secs_f64());
    }

    // SEQUENTIAL: Save to database using batch insert (much faster!)
    let db_save_start = Instant::now();
    let mut valid_fingerprints = Vec::new();
    let mut skipped = 0;

    for (path_str, result) in fingerprints {
        match result {
            Ok(fp) => {
                valid_fingerprints.push(fp);
            }
            Err(e) => {
                if verbose {
                    eprintln!("[rust]   Skipping {}: {}", path_str, e);
                }
                skipped += 1;
            }
        }
    }

    let count = db.save_baseline_fingerprints_batch(valid_fingerprints)?;

    if verbose {
        eprintln!("[rust] Saved {} fingerprints to DB in {:.3}s (batch insert)", count, db_save_start.elapsed().as_secs_f64());
        eprintln!("[rust] Total processing time: {:.3}s", processing_start.elapsed().as_secs_f64());
        eprintln!("[rust] Successfully saved: {}, Skipped: {}", count, skipped);
    }

    Ok(count)
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
/// * `scope_paths` - List of directory paths to limit the scope (e.g., ["tests/unit/"])
///
/// # Returns
/// * ChangedFiles containing list of modified files and changed blocks
#[pyfunction]
pub fn detect_changes(db_path: &str, project_root: &str, scope_paths: Vec<String>) -> PyResult<ChangedFiles> {
    let changes = detect_changes_internal(db_path, project_root, scope_paths).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to detect changes: {}", e))
    })?;

    Ok(changes)
}

fn detect_changes_internal(db_path: &str, project_root: &str, scope_paths: Vec<String>) -> Result<ChangedFiles> {
    // Open database
    let db = TestmonDatabase::open(db_path)?;

    // Find all Python files in the project
    let python_files = find_python_files(project_root, &scope_paths)?;
    // eprintln!("DEBUG: Found {} Python files to check", python_files.len());

    // Process files sequentially
    // TODO: Can optimize with parallel processing later by using multiple database connections
    let changed_entries: Vec<_> = python_files
        .iter()
        .filter_map(|path| {
            // eprintln!("DEBUG: Checking file: {}", path.display());
            match check_file_changed(&db, path) {
                Ok(Some(change)) => {
                    // eprintln!("DEBUG: File changed: {} with {} changed blocks", path.display(), change.1.len());
                    Some(change)
                }
                Ok(None) => {
                    // eprintln!("DEBUG: File unchanged: {}", path.display());
                    None
                }
                Err(_) => {
                    // Only show warnings for actual errors
                    // eprintln!("Warning: Failed to check {}: {}", path.display(), e);
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
fn find_python_files(root: &str, scope_paths: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // Convert root to absolute path
    let root_path = std::fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));

    // Convert scope paths to absolute PathBufs for comparison
    let scope_paths_abs: Vec<PathBuf> = scope_paths
        .iter()
        .map(|p| {
            let path = PathBuf::from(p);
            std::fs::canonicalize(&path).unwrap_or(path)
        })
        .collect();

    for entry in WalkDir::new(&root_path)
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
            // Store absolute path
            let abs_path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
            };

            // Check if file is within any of the scope paths
            let in_scope = scope_paths_abs.iter().any(|scope| abs_path.starts_with(scope));
            if in_scope {
                files.push(abs_path);
            }
        }
    }

    Ok(files)
}

/// Check if a file has changed using three-level detection
///
/// Returns Some((filename, changed_checksums)) if changed, None if unchanged
fn check_file_changed(db: &TestmonDatabase, path: &Path) -> Result<Option<(String, Vec<i32>)>> {
    let filename = path.to_string_lossy().to_string();

    // Get baseline fingerprint (NOT last-run fingerprint!)
    // This compares against the "known good" state set via --diff-baseline
    let stored_fp = match db.get_baseline_fingerprint_rust(&filename)? {
        Some(fp) => fp,
        None => {
            // No baseline for this file - it's not tracked yet, so no change to detect
            // Baselines are set when running: pytest --diff-baseline
            return Ok(None);
        }
    };

    // Level 1: mtime check (fastest)
    let metadata = std::fs::metadata(path)?;
    let current_mtime = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs_f64();

    // eprintln!("DEBUG: {} - current_mtime: {}, stored_mtime: {}, diff: {}",
    //           filename, current_mtime, stored_fp.mtime, (current_mtime - stored_fp.mtime).abs());

    if (current_mtime - stored_fp.mtime).abs() < 0.001 {
        // mtime unchanged - file definitely not modified
        // eprintln!("DEBUG: {} - mtime unchanged, skipping", filename);
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

/// Find which OLD checksums were removed/modified (these indicate blocks that changed)
///
/// Returns the OLD checksums that are no longer present in the new version.
/// These are the checksums that tests may have used, so any test that used
/// these blocks should be re-run to verify the changes.
fn find_changed_checksums(old_checksums: &[i32], new_checksums: &[i32]) -> Vec<i32> {
    let new_set: std::collections::HashSet<i32> = new_checksums.iter().copied().collect();

    // Return OLD checksums that are no longer in the new version
    // These represent blocks that were removed or modified
    old_checksums
        .iter()
        .copied()
        .filter(|checksum| !new_set.contains(checksum))
        .collect()
}

/// Process coverage data and return filtered fingerprints
///
/// This function does the heavy lifting of coverage processing in Rust with
/// parallel processing for better performance. It:
/// 1. Filters files (only .py files in project, excludes other test files)
/// 2. Calculates fingerprints for each file (using cache if provided)
/// 3. Filters blocks to only those that were executed (block-level granularity)
/// 4. Returns fingerprints with only executed blocks
///
/// # Arguments
/// * `coverage_data` - Map of filename -> list of executed line numbers
/// * `project_root` - Root directory of the project
/// * `test_file` - Path to the current test file (to filter out other test files)
/// * `verbose` - Whether to print debug information
/// * `scope_paths` - List of directory paths to limit the scope (e.g., ["tests/unit/"])
/// * `cache` - Optional FingerprintCache to avoid re-parsing files
///
/// # Returns
/// * List of Fingerprint objects with only executed blocks
#[pyfunction]
#[pyo3(signature = (coverage_data, project_root, test_file, verbose, scope_paths, cache=None))]
pub fn process_coverage_data(
    coverage_data: HashMap<String, Vec<usize>>,
    project_root: &str,
    test_file: &str,
    verbose: bool,
    scope_paths: Vec<String>,
    cache: Option<&crate::fingerprint_cache::FingerprintCache>,
) -> PyResult<Vec<Fingerprint>> {
    let fingerprints = process_coverage_data_internal(coverage_data, project_root, test_file, verbose, scope_paths, cache)
        .map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Failed to process coverage data: {}",
                e
            ))
        })?;

    Ok(fingerprints)
}

fn process_coverage_data_internal(
    coverage_data: HashMap<String, Vec<usize>>,
    project_root: &str,
    test_file: &str,
    verbose: bool,
    scope_paths: Vec<String>,
    cache: Option<&crate::fingerprint_cache::FingerprintCache>,
) -> Result<Vec<Fingerprint>> {
    let project_root_path = Path::new(project_root);
    let test_file_path = Path::new(test_file);

    // Convert scope paths to absolute PathBufs for comparison
    let scope_paths_abs: Vec<PathBuf> = scope_paths
        .iter()
        .map(|p| {
            let path = PathBuf::from(p);
            std::fs::canonicalize(&path).unwrap_or(path)
        })
        .collect();

    // Process files in parallel with rayon
    let fingerprints: Vec<Fingerprint> = coverage_data
        .par_iter()
        .filter_map(|(filename, executed_lines)| {
            let filepath = Path::new(filename);

            // 1. File filtering - only include relevant Python files
            if !should_process_file(filepath, project_root_path, test_file_path, &scope_paths_abs) {
                return None;
            }

            // 2. Calculate fingerprint with all blocks (use cache if available)
            let fp = match cache {
                Some(c) => match c.get_or_calculate_internal(filename) {
                    Ok(fp) => fp,
                    Err(e) => {
                        if verbose {
                            eprintln!("⚠ pytest-diff: Could not fingerprint {}: {}", filename, e);
                        }
                        return None;
                    }
                },
                None => match calculate_fingerprint_internal(filename) {
                    Ok(fp) => fp,
                    Err(e) => {
                        if verbose {
                            eprintln!("⚠ pytest-diff: Could not fingerprint {}: {}", filename, e);
                        }
                        return None;
                    }
                },
            };

            // 3. Filter blocks to only those that were executed
            let blocks = match &fp.blocks {
                Some(blocks) => blocks,
                None => return Some(fp), // No blocks info - use full fingerprint
            };

            let executed_lines_set: HashSet<usize> = executed_lines.iter().copied().collect();
            let executed_blocks = filter_executed_blocks_rust(blocks, &executed_lines_set);

            if executed_blocks.is_empty() {
                if verbose {
                    eprintln!("[DEBUG] {}: No blocks executed (skipping)", filename);
                }
                return None;
            }

            if verbose {
                eprintln!(
                    "[DEBUG] {}: {}/{} blocks executed",
                    filename,
                    executed_blocks.len(),
                    blocks.len()
                );
            }

            // 4. Create filtered fingerprint with only executed blocks
            let filtered_checksums: Vec<i32> = executed_blocks.iter().map(|b| b.checksum).collect();

            Some(Fingerprint {
                filename: fp.filename,
                checksums: filtered_checksums,
                file_hash: fp.file_hash,
                mtime: fp.mtime,
                blocks: None, // Don't need to store full blocks in DB
            })
        })
        .collect();

    Ok(fingerprints)
}

/// Check if a file should be processed based on filtering rules
fn should_process_file(filepath: &Path, project_root: &Path, test_file: &Path, scope_paths: &[PathBuf]) -> bool {
    // Must be a .py file
    if filepath.extension().and_then(|s| s.to_str()) != Some("py") {
        return false;
    }

    // Must be in the project root
    let path_str = filepath.to_string_lossy();
    if !path_str.starts_with(project_root.to_string_lossy().as_ref()) {
        return false;
    }

    // Check if file is within any of the scope paths
    let in_scope = scope_paths.iter().any(|scope| filepath.starts_with(scope));
    if !in_scope {
        return false;
    }

    // Filter out test files that aren't the current test file
    // This prevents coverage contamination where test collection
    // causes all tests to depend on all test files
    let is_test_file = path_str.contains("/tests/")
        || path_str.ends_with("_test.py")
        || path_str
            .rsplit('/')
            .next()
            .map(|name| name.starts_with("test_"))
            .unwrap_or(false);

    let is_current_test_file = filepath == test_file;

    // Skip other test files, but include source files and current test file
    if is_test_file && !is_current_test_file {
        return false;
    }

    true
}

/// Filter blocks to only those where at least one line was executed
///
/// This implements testmon-style block-level granularity in Rust for performance:
/// - Only blocks that were actually executed are tracked as dependencies
/// - If function_a() is never called, changing it won't re-run this test
///
/// # Arguments
/// * `blocks` - List of Block objects with start_line/end_line
/// * `executed_lines` - Set of line numbers that were executed
///
/// # Returns
/// * Vec of Block objects that were executed
fn filter_executed_blocks_rust(blocks: &[Block], executed_lines: &HashSet<usize>) -> Vec<Block> {
    blocks
        .iter()
        .filter(|block| {
            // Check if any line in this block was executed
            // Block lines are inclusive: [start_line, end_line]
            (block.start_line..=block.end_line).any(|line| executed_lines.contains(&line))
        })
        .cloned()
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
