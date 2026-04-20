use std::path::Path;

// ═══════════════════════════════════════════
// FILE DISCOVERY
// ═══════════════════════════════════════════

/// Find all Rust source files at a path (file or directory).
pub fn find_rust_files(path: &str, recursive: bool) -> Vec<String> {
    let path = Path::new(path);
    let mut files = Vec::new();

    if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
        files.push(path.to_string_lossy().to_string());
    } else if path.is_dir() {
        scan_dir(path, recursive, &["rs"], &mut files);
    }

    files.sort();
    files
}

/// Find source files with any of the given extensions.
pub fn find_source_files(path: &str, recursive: bool, extensions: &[&str]) -> Vec<String> {
    let path = Path::new(path);
    let mut files = Vec::new();

    if path.is_file() {
        if let Some(ext) = path.extension() {
            if extensions.contains(&ext.to_string_lossy().as_ref()) {
                files.push(path.to_string_lossy().to_string());
            }
        }
    } else if path.is_dir() {
        scan_dir(path, recursive, extensions, &mut files);
    }

    files.sort();
    files
}

/// Check whether a file path has one of the given extensions.
fn should_include_file(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .map_or(false, |ext| extensions.contains(&ext.to_string_lossy().as_ref()))
}

/// Check whether a directory should be traversed (not a skipped/hidden dir).
fn should_scan_dir(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    !matches!(name.as_ref(), "target" | ".git" | "node_modules") && !name.starts_with('.')
}

/// Recursively scan a directory for files with given extensions.
/// Skips target/, .git/, node_modules/, and hidden directories.
pub fn scan_dir(dir: &Path, recursive: bool, extensions: &[&str], files: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && should_include_file(&path, extensions) {
            files.push(path.to_string_lossy().to_string());
        } else if recursive && path.is_dir() && should_scan_dir(&path) {
            scan_dir(&path, recursive, extensions, files);
        }
    }
}

// ═══════════════════════════════════════════
// STRING UTILITIES
// ═══════════════════════════════════════════

/// Truncate a string to max length, adding "…" if truncated.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 1 {
        format!("…{}", &s[s.len() - max + 1..])
    } else {
        "…".to_string()
    }
}

/// Truncate from the left (keep end).
pub fn truncate_left(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 1 {
        format!("{}…", &s[..max - 1])
    } else {
        "…".to_string()
    }
}

// ═══════════════════════════════════════════
// LINE NUMBER ESTIMATION
// ═══════════════════════════════════════════

/// Estimate the line number of a pattern in source code.
pub fn estimate_line(source: &str, pattern: &str) -> usize {
    for (i, line) in source.lines().enumerate() {
        if line.contains(pattern) {
            return i + 1;
        }
    }
    1
}

/// Estimate line number of a function definition.
pub fn estimate_fn_line(source: &str, fn_name: &str) -> usize {
    estimate_line(source, &format!("fn {}", fn_name))
}

// ═══════════════════════════════════════════
// OUTPUT FORMATTING HELPERS
// ═══════════════════════════════════════════

/// Print a standard separator line.
pub fn separator(width: usize) -> String {
    "─".repeat(width)
}

/// Print a section header.
pub fn section_header(title: &str) {
    println!();
    println!("{}", title);
    println!("{}", separator(title.len().max(40)));
}

// ═══════════════════════════════════════════
// GIT INTEGRATION
// ═══════════════════════════════════════════

/// Get git churn data: file -> number of commits since a date.
pub fn get_git_churn(repo_root: &Path, since: &str) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;
    use std::process::Command;

    let output = Command::new("git")
        .args(["log", "--since", since, "--name-only", "--pretty=format:"])
        .current_dir(repo_root)
        .output();

    let mut churn: HashMap<String, u32> = HashMap::new();

    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let file = line.trim();
                if !file.is_empty() && !file.starts_with('.') {
                    *churn.entry(file.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    churn
}

/// Get git blame info for a specific line.
pub fn get_git_blame(file_path: &str, line: usize) -> (Option<String>, Option<String>) {
    use std::process::Command;

    let output = Command::new("git")
        .args(["blame", "-L", &format!("{},{}", line, line), "--porcelain", file_path])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut author = None;
            let mut date = None;

            for line in text.lines() {
                if let Some(name) = line.strip_prefix("author ") {
                    author = Some(name.to_string());
                }
                if let Some(d) = line.strip_prefix("author-time ") {
                    if let Ok(ts) = d.parse::<i64>() {
                        date = Some(format_timestamp(ts));
                    }
                }
            }

            (author, date)
        }
        _ => (None, None),
    }
}

fn format_timestamp(ts: i64) -> String {
    let days = ts / 86400;
    let year = 1970 + days / 365;
    let remaining = days % 365;
    let month = remaining / 30 + 1;
    let day = remaining % 30 + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

// ═══════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "…world");
        assert_eq!(truncate("hi", 1), "…");
    }

    #[test]
    fn test_truncate_left() {
        assert_eq!(truncate_left("hello", 10), "hello");
        assert_eq!(truncate_left("hello world", 6), "hello…");
    }

    #[test]
    fn test_estimate_line() {
        let source = "fn main() {\n    let x = 1;\n    println!(\"hi\");\n}";
        assert_eq!(estimate_line(source, "fn main"), 1);
        assert_eq!(estimate_line(source, "println"), 3);
        assert_eq!(estimate_line(source, "missing"), 1);
    }

    #[test]
    fn test_estimate_fn_line() {
        let source = "fn foo() {}\n\nfn bar() {\n    x\n}";
        assert_eq!(estimate_fn_line(source, "foo"), 1);
        assert_eq!(estimate_fn_line(source, "bar"), 3);
    }
}
