/// Delta (incremental) mutation testing.
///
/// Uses git diff to identify changed functions, builds a call graph to find
/// affected callers, and limits mutation generation to only those functions.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Information about a changed region from git diff
#[derive(Debug, Clone)]
pub struct ChangedRegion {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Result of delta analysis: which functions should be mutated
#[derive(Debug, Clone)]
pub struct DeltaAnalysis {
    pub changed_files: Vec<String>,
    pub changed_functions: HashMap<String, Vec<String>>, // file -> function names
    pub affected_functions: HashMap<String, Vec<String>>, // file -> function names (includes callers)
    pub reduction_pct: f64,
}

/// Run git diff to find changed .rs files against the given base ref.
pub fn get_changed_files(repo_root: &Path, base_ref: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", base_ref])
        .current_dir(repo_root)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .filter(|l| l.ends_with(".rs"))
                .map(|l| l.to_string())
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Parse git diff output to extract changed line ranges per file.
pub fn get_changed_lines(repo_root: &Path, base_ref: &str) -> Vec<ChangedRegion> {
    let output = Command::new("git")
        .args(["diff", "-U0", base_ref])
        .current_dir(repo_root)
        .output();

    let diff_text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Vec::new(),
    };

    let mut regions = Vec::new();
    let mut current_file: Option<String> = None;

    for line in diff_text.lines() {
        if line.starts_with("diff --git ") {
            // Extract file path from "diff --git a/path b/path"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let b_path = parts[3];
                if b_path.starts_with("b/") && b_path.ends_with(".rs") {
                    current_file = Some(b_path[2..].to_string());
                }
            }
        } else if line.starts_with("@@") {
            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
            if let Some(ref file) = current_file {
                if let Some(range) = parse_hunk_header(line) {
                    regions.push(ChangedRegion {
                        file: file.clone(),
                        start_line: range.0,
                        end_line: range.1,
                    });
                }
            }
        }
    }

    regions
}

/// Parse a diff hunk header to extract the new file line range.
/// Returns (start_line, end_line) for the new file side.
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // Format: @@ -l,s +l,s @@ or @@ -l +l @@
    let trimmed = line.trim_start_matches('@').trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // The second token is the new file range: +start,count
    let new_range = parts[1].trim_start_matches('+');
    let nums: Vec<&str> = new_range.split(',').collect();

    let start = nums.first()?.parse::<usize>().ok()?;
    let count = if nums.len() > 1 {
        nums.get(1)?.parse::<usize>().ok()?
    } else {
        1
    };

    Some((start, start + count.saturating_sub(1)))
}

/// Scan source code to find all function definitions and their line ranges.
/// Returns HashMap<function_name, (start_line, end_line)>.
pub fn find_function_ranges(source: &str) -> HashMap<String, (usize, usize)> {
    let mut functions = HashMap::new();
    let mut current_fn: Option<(String, usize)> = None;
    let mut brace_depth = 0;
    let mut line_num = 0;

    for line in source.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Detect function signature
        let is_signature = (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") ||
            trimmed.starts_with("pub async fn ") || trimmed.starts_with("async fn ") ||
            trimmed.starts_with("pub unsafe fn ") || trimmed.starts_with("unsafe fn "))
            && trimmed.contains('(');

        if is_signature {
            // Try to extract function name
            let after_fn = trimmed.find("fn ").map(|i| &trimmed[i + 3..]);
            if let Some(rest) = after_fn {
                let name_end = rest.find(|c: char| c == '(' || c.is_whitespace());
                if let Some(end) = name_end {
                    let name = rest[..end].trim();
                    if !name.is_empty() {
                        current_fn = Some((name.to_string(), line_num));
                        // Start fresh brace count for this line only
                        brace_depth = trimmed.matches('{').count().saturating_sub(trimmed.matches('}').count());
                    }
                }
            }
        }

        if current_fn.is_some() {
            // Only count braces if this is NOT the signature line (already counted above)
            if !is_signature {
                brace_depth += trimmed.matches('{').count();
                brace_depth = brace_depth.saturating_sub(trimmed.matches('}').count());
            }

            if brace_depth == 0 {
                if let Some((name, start)) = current_fn.take() {
                    functions.insert(name, (start, line_num));
                }
            }
        }
    }

    functions
}

/// Map changed line ranges to affected function names per file.
pub fn map_changed_to_functions(
    changed_regions: &[ChangedRegion],
    source_files: &[(String, String)], // (file_path, source)
) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();

    for (file_path, source) in source_files {
        let functions = find_function_ranges(source);
        let file_regions: Vec<_> = changed_regions
            .iter()
            .filter(|r| file_path.ends_with(&r.file) || r.file == *file_path)
            .collect();

        for (fn_name, (start, end)) in &functions {
            let affected = file_regions.iter().any(|r| {
                // Check if any changed line falls within this function
                (r.start_line >= *start && r.start_line <= *end) ||
                (r.end_line >= *start && r.end_line <= *end) ||
                (r.start_line <= *start && r.end_line >= *end)
            });

            if affected {
                result.entry(file_path.clone())
                    .or_default()
                    .push(fn_name.clone());
            }
        }
    }

    result
}

/// Build a simple call graph: for each function, which functions does it call?
pub fn build_call_graph(source_files: &[(String, String)]) -> HashMap<String, HashSet<String>> {
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();

    for (_file_path, source) in source_files {
        let functions = find_function_ranges(source);
        let lines: Vec<&str> = source.lines().collect();

        for (fn_name, (start, end)) in &functions {
            let callers = graph.entry(fn_name.clone()).or_default();

            // Scan lines within this function for calls to other functions
            // Note: end is inclusive, so we iterate through end (exclusive range goes to end)
            for line_idx in (start.saturating_sub(1))..(*end).min(lines.len()) {
                let line = lines[line_idx];
                let trimmed = line.trim();

                // Skip comments and local let bindings (but not fn bodies on same line)
                if trimmed.starts_with("//") || trimmed.starts_with("let ") {
                    continue;
                }

                for (other_name, _) in &functions {
                    if other_name == fn_name {
                        continue;
                    }
                    // Look for function call patterns
                    let call_patterns = [
                        format!("{}(", other_name),
                        format!(" {}(", other_name),
                        format!("{}::", other_name),
                        format!("&mut {}", other_name),
                    ];
                    if call_patterns.iter().any(|p| line.contains(p)) {
                        callers.insert(other_name.clone());
                    }
                }
            }
        }
    }

    graph
}

/// Get the transitive closure of affected functions (changed + all callers).
pub fn get_affected_functions(
    changed_functions: &HashMap<String, Vec<String>>,
    call_graph: &HashMap<String, HashSet<String>>,
) -> HashMap<String, Vec<String>> {
    let mut affected = HashMap::new();

    // Build reverse call graph: who calls whom
    let mut reverse_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (caller, callees) in call_graph {
        for callee in callees {
            reverse_graph.entry(callee.clone()).or_default().insert(caller.clone());
        }
    }

    for (file, functions) in changed_functions {
        let mut all_affected = HashSet::new();
        let mut to_process: Vec<String> = functions.clone();

        while let Some(func) = to_process.pop() {
            if all_affected.insert(func.clone()) {
                // Add all callers of this function
                if let Some(callers) = reverse_graph.get(&func) {
                    for caller in callers {
                        to_process.push(caller.clone());
                    }
                }
            }
        }

        affected.insert(file.clone(), all_affected.into_iter().collect());
    }

    affected
}

/// Check if a line is within any of the given function names in a file.
pub fn is_line_in_affected_function(
    file_path: &str,
    line: usize,
    affected: &HashMap<String, Vec<String>>,
    source_files: &[(String, String)],
) -> bool {
    let source = source_files.iter()
        .find(|(f, _)| file_path.ends_with(f) || f == file_path)
        .map(|(_, s)| s.as_str());

    let Some(source) = source else { return true }; // If we can't find source, allow it

    let functions = find_function_ranges(source);
    let affected_fns = affected.get(file_path)
        .or_else(|| affected.iter().find(|(k, _)| file_path.ends_with(k.as_str())).map(|(_, v)| v))
        .cloned()
        .unwrap_or_default();

    for (fn_name, (start, end)) in &functions {
        if affected_fns.contains(fn_name) && line >= *start && line <= *end {
            return true;
        }
    }

    false
}

/// Run full delta analysis: return changed files, changed functions, and affected functions.
pub fn run_delta_analysis(
    repo_root: &Path,
    base_ref: &str,
    source_files: &[(String, String)],
    _total_files: usize,
) -> DeltaAnalysis {
    let changed_files = get_changed_files(repo_root, base_ref);
    let regions = get_changed_lines(repo_root, base_ref);

    let changed_functions = map_changed_to_functions(&regions, source_files);
    let call_graph = build_call_graph(source_files);
    let affected_functions = get_affected_functions(&changed_functions, &call_graph);

    let _changed_count: usize = changed_functions.values().map(|v| v.len()).sum();
    let total_fn_count: usize = source_files.iter()
        .map(|(_, s)| find_function_ranges(s).len())
        .sum();

    let reduction_pct = if total_fn_count > 0 {
        let affected_count: usize = affected_functions.values().map(|v| v.len()).sum();
        ((total_fn_count - affected_count) as f64 / total_fn_count as f64 * 100.0).max(0.0)
    } else {
        0.0
    };

    DeltaAnalysis {
        changed_files,
        changed_functions,
        affected_functions,
        reduction_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -10,5 +20,7 @@"), Some((20, 26)));
        assert_eq!(parse_hunk_header("@@ -5 +15 @@"), Some((15, 15)));
    }

    #[test]
    fn test_find_function_ranges() {
        let source = r#"
fn foo() {
    let x = 1;
}

fn bar(a: i32) -> i32 {
    a + 1
}
"#;
        let ranges = find_function_ranges(source);
        assert!(ranges.contains_key("foo"));
        assert!(ranges.contains_key("bar"));
        let (foo_start, foo_end) = ranges["foo"];
        assert_eq!(foo_start, 2);
        assert!(foo_end > foo_start);
    }

    #[test]
    fn test_map_changed_to_functions() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
        let regions = vec![
            ChangedRegion { file: "test.rs".to_string(), start_line: 2, end_line: 4 },
        ];
        let files = vec![("test.rs".to_string(), source.to_string())];
        let result = map_changed_to_functions(&regions, &files);
        assert!(result.get("test.rs").unwrap().contains(&"add".to_string()));
        assert!(!result.get("test.rs").unwrap().contains(&"subtract".to_string()));
    }

    #[test]
    fn test_call_graph() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn double(x: i32) -> i32 {
    add(x, x)
}
"#;
        let files = vec![("test.rs".to_string(), source.to_string())];
        let graph = build_call_graph(&files);
        assert!(graph.get("double").unwrap().contains("add"));
    }

    #[test]
    fn test_affected_functions_transitive() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn double(x: i32) -> i32 { add(x, x) }
fn quadruple(x: i32) -> i32 { double(double(x)) }
"#;
        let files = vec![("test.rs".to_string(), source.to_string())];
        let changed = {
            let mut m = HashMap::new();
            m.insert("test.rs".to_string(), vec!["add".to_string()]);
            m
        };
        let graph = build_call_graph(&files);
        let affected = get_affected_functions(&changed, &graph);
        let fns = affected.get("test.rs").unwrap();
        assert!(fns.contains(&"add".to_string()));
        assert!(fns.contains(&"double".to_string()));
        assert!(fns.contains(&"quadruple".to_string()));
    }
}
