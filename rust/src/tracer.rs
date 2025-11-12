// Coverage collection for Python tests
//
// This module provides a coverage collector that integrates with
// Python's sys.settrace (3.8-3.11) or sys.monitoring (3.12+)

use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Coverage collector for tracking line execution during tests
///
/// Usage from Python:
/// ```python
/// collector = CoverageCollector()
/// collector.start_test("test_example")
///
/// # In trace function:
/// collector.record_line(filename, line_no)
///
/// coverage = collector.finish_test()
/// ```
#[pyclass]
pub struct CoverageCollector {
    current_test: Mutex<Option<String>>,
    coverage: Mutex<HashMap<String, HashMap<String, HashSet<usize>>>>,
}

#[pymethods]
impl CoverageCollector {
    #[new]
    fn new() -> Self {
        Self {
            current_test: Mutex::new(None),
            coverage: Mutex::new(HashMap::new()),
        }
    }

    /// Start collecting coverage for a test
    fn start_test(&self, test_name: String) {
        let mut current = self.current_test.lock().unwrap();
        *current = Some(test_name);
    }

    /// Record a line execution
    ///
    /// Called from Python's trace function for each line executed
    fn record_line(&self, filename: String, line_no: usize) {
        let current = self.current_test.lock().unwrap();
        if let Some(ref test_name) = *current {
            let mut coverage = self.coverage.lock().unwrap();
            coverage
                .entry(test_name.clone())
                .or_default()
                .entry(filename)
                .or_default()
                .insert(line_no);
        }
    }

    /// Finish collecting coverage for current test
    ///
    /// Returns a dict mapping filename -> list of line numbers
    fn finish_test(&self) -> PyResult<HashMap<String, Vec<usize>>> {
        let mut current = self.current_test.lock().unwrap();
        let test_name = current.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("No test currently running")
        })?;

        let mut coverage = self.coverage.lock().unwrap();
        let test_coverage = coverage.remove(&test_name).unwrap_or_default();

        // Convert HashSet to sorted Vec for each file
        let result = test_coverage
            .into_iter()
            .map(|(file, lines)| {
                let mut lines_vec: Vec<_> = lines.into_iter().collect();
                lines_vec.sort_unstable();
                (file, lines_vec)
            })
            .collect();

        Ok(result)
    }

    /// Clear all collected coverage data
    fn clear(&self) {
        let mut coverage = self.coverage.lock().unwrap();
        coverage.clear();

        let mut current = self.current_test.lock().unwrap();
        *current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_collection() {
        let collector = CoverageCollector::new();

        collector.start_test("test_example".to_string());
        collector.record_line("file1.py".to_string(), 10);
        collector.record_line("file1.py".to_string(), 11);
        collector.record_line("file2.py".to_string(), 5);

        let coverage = collector.finish_test().unwrap();

        assert_eq!(coverage.len(), 2);
        assert_eq!(coverage["file1.py"], vec![10, 11]);
        assert_eq!(coverage["file2.py"], vec![5]);
    }

    #[test]
    fn test_no_test_running() {
        let collector = CoverageCollector::new();
        let result = collector.finish_test();

        assert!(result.is_err());
    }
}
