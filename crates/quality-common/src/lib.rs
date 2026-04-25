use std::path::Path;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════
// HEADLESS API TYPES
// ═══════════════════════════════════════════

/// Request to run a quality tool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolRequest {
    pub tool: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Response from a quality tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub tool: String,
    pub version: String,
    pub success: bool,
    pub duration_ms: u64,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Progress event streamed during long-running tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub tool: String,
    pub stage: String,
    pub progress_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Result from one tool run within a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub duration_ms: u64,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Combined report from running multiple tools in one batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedReport {
    pub run_id: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub tools: Vec<ToolResult>,
    pub summary: ReportSummary,
}

/// Summary of a batch run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_tools: usize,
    pub passed: usize,
    pub failed: usize,
    pub languages_detected: Vec<String>,
}

/// Convenience: wrap raw tool data into a ToolResponse envelope.
pub fn wrap_tool_response(
    tool: &str,
    version: &str,
    success: bool,
    duration_ms: u64,
    data: serde_json::Value,
    summary: Option<serde_json::Value>,
    error: Option<String>,
) -> ToolResponse {
    ToolResponse {
        tool: tool.to_string(),
        version: version.to_string(),
        success,
        duration_ms,
        data,
        summary,
        error,
    }
}

/// Convenience: create a new UnifiedReport with a generated run_id.
pub fn new_unified_report(started_at: String) -> UnifiedReport {
    UnifiedReport {
        run_id: format!("run-{}", uuid::Uuid::new_v4()),
        started_at,
        duration_ms: 0,
        tools: Vec::new(),
        summary: ReportSummary {
            total_tools: 0,
            passed: 0,
            failed: 0,
            languages_detected: Vec::new(),
        },
    }
}

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
/// Keeps the RIGHT side (end) of the string.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 1 {
        // Keep the last max-1 chars, find valid char boundary from the right
        let start = s.len() - (max - 1);
        let mut start = start;
        while start > 0 && !s.is_char_boundary(start) {
            start -= 1;
        }
        format!("…{}", &s[start..])
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
// TABLE FORMATTING
// ═══════════════════════════════════════════

/// A column in a table output.
pub struct Column {
    pub header: &'static str,
    pub width: usize,
    pub align_right: bool,
}

impl Column {
    pub fn left(header: &'static str, width: usize) -> Self {
        Self { header, width, align_right: false }
    }
    pub fn right(header: &'static str, width: usize) -> Self {
        Self { header, width, align_right: true }
    }
}

/// Print a table header row.
pub fn print_table_header(columns: &[Column]) {
    let mut line = String::new();
    for col in columns {
        if col.align_right {
            line.push_str(&format!("{:>width$} ", col.header, width = col.width));
        } else {
            line.push_str(&format!("{:<width$} ", col.header, width = col.width));
        }
    }
    println!("{}", line.trim_end());
    let total_width: usize = columns.iter().map(|c| c.width + 1).sum();
    println!("{}", separator(total_width));
}

/// Print a table row with values.
pub fn print_table_row(columns: &[Column], values: &[&str]) {
    let mut line = String::new();
    for (col, val) in columns.iter().zip(values.iter()) {
        let truncated = truncate(val, col.width);
        if col.align_right {
            line.push_str(&format!("{:>width$} ", truncated, width = col.width));
        } else {
            line.push_str(&format!("{:<width$} ", truncated, width = col.width));
        }
    }
    println!("{}", line.trim_end());
}

/// Print a summary section with key-value pairs.
pub fn print_summary(items: &[(&str, String)]) {
    println!();
    for (key, value) in items {
        println!("  {:<25} {}", key, value);
    }
}

/// Print a verdict line with icon.
pub fn print_verdict(score: f64, good_threshold: f64, label_good: &str, label_bad: &str) {
    if score <= good_threshold {
        println!("\n  {} {:.1} — {}", "✓", score, label_good);
    } else {
        println!("\n  {} {:.1} — {}", "✗", score, label_bad);
    }
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

/// Get git blame info for multiple lines in a file efficiently.
/// Returns a HashMap mapping line number to (author, date).
pub fn get_git_blame_batch(file_path: &str, lines: &[usize]) -> std::collections::HashMap<usize, (Option<String>, Option<String>)> {
    use std::process::Command;
    use std::collections::HashMap;

    if lines.is_empty() {
        return HashMap::new();
    }

    // Sort and deduplicate lines
    let mut sorted_lines = lines.to_vec();
    sorted_lines.sort_unstable();
    sorted_lines.dedup();

    // Build line ranges to minimize git blame calls
    // Group consecutive lines into ranges
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start = sorted_lines[0];
    let mut prev_line = sorted_lines[0];

    for &line in &sorted_lines[1..] {
        if line == prev_line + 1 {
            // Consecutive, extend current range
            prev_line = line;
        } else {
            // Gap, close current range and start new one
            ranges.push((range_start, prev_line));
            range_start = line;
            prev_line = line;
        }
    }
    ranges.push((range_start, prev_line));

    // Call git blame for each range and collect results
    let mut results: HashMap<usize, (Option<String>, Option<String>)> = HashMap::new();

    for (start, end) in ranges {
        let output = Command::new("git")
            .args(["blame", "-L", &format!("{},{}", start, end), "--porcelain", file_path])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let mut current_line: Option<usize> = None;
                let mut current_author: Option<String> = None;
                let mut current_date: Option<String> = None;

                for line_text in text.lines() {
                    // Parse the header line which contains the original line number
                    // Format: <sha1> <original_line> <final_line> <line_count>
                    if line_text.starts_with('\t') {
                        // Content line - associate collected data with current line
                        if let Some(line_num) = current_line {
                            results.insert(line_num, (current_author.clone(), current_date.clone()));
                        }
                    } else if let Some(author) = line_text.strip_prefix("author ") {
                        current_author = Some(author.to_string());
                    } else if let Some(time_str) = line_text.strip_prefix("author-time ") {
                        if let Ok(ts) = time_str.parse::<i64>() {
                            current_date = Some(format_timestamp(ts));
                        }
                    } else if line_text.len() >= 40 && !line_text.starts_with('\t') && !line_text.starts_with("author") {
                        // Header line: extract the original line number
                        // Format: <40-char-sha> <original-line> <final-line> <line-count>
                        let parts: Vec<&str> = line_text.split_whitespace().collect();
                        if parts.len() >= 3 {
                            if let Ok(orig_line) = parts[1].parse::<usize>() {
                                current_line = Some(orig_line);
                            }
                        }
                    }
                }
                // Don't forget the last entry
                if let Some(line_num) = current_line {
                    results.insert(line_num, (current_author.clone(), current_date.clone()));
                }
            }
        }
    }

    results
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

// ═══════════════════════════════════════════
// SARIF OUTPUT
// ═══════════════════════════════════════════

/// Minimal SARIF v2.1.0 structures for GitHub Security / VS Code ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocations: Option<Vec<SarifInvocation>>,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifDriver {
    pub name: String,
    pub version: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRule {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_description: Option<SarifMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_description: Option<SarifMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_configuration: Option<SarifRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRuleConfig {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifInvocation {
    pub execution_successful: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifResult {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<usize>,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifPhysicalLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_location: Option<SarifArtifactLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<SarifRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRegion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<usize>,
}

impl SarifLog {
    /// Build a minimal SARIF log from a tool name and findings.
    pub fn new(_tool_name: &str, _tool_version: &str) -> Self {
        SarifLog {
            schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
            version: "2.1.0".to_string(),
            runs: Vec::new(),
        }
    }

    pub fn add_run(&mut self, run: SarifRun) {
        self.runs.push(run);
    }
}

/// Convenience builder for a single-tool SARIF run.
pub fn sarif_run(
    tool_name: &str,
    tool_version: &str,
    results: Vec<SarifResult>,
    exit_code: i32,
) -> SarifRun {
    let mut rule_ids: Vec<String> = results.iter().map(|r| r.rule_id.clone()).collect();
    rule_ids.sort();
    rule_ids.dedup();

    let rules: Vec<SarifRule> = rule_ids.into_iter().map(|id| {
        SarifRule {
            id: id.clone(),
            name: Some(id.clone()),
            short_description: Some(SarifMessage { text: format!("Rule {}", id) }),
            full_description: None,
            default_configuration: Some(SarifRuleConfig { level: "warning".to_string() }),
        }
    }).collect();

    SarifRun {
        tool: SarifTool {
            driver: SarifDriver {
                name: tool_name.to_string(),
                version: tool_version.to_string(),
                rules,
            },
        },
        invocations: Some(vec![SarifInvocation {
            execution_successful: exit_code == 0,
            exit_code: Some(exit_code),
            end_time_utc: Some(chrono::Utc::now().to_rfc3339()),
        }]),
        results,
    }
}

/// Convert a quality-level string to SARIF level.
/// "error" | "warning" | "note" | "none"
pub fn sarif_level(level: &str) -> &'static str {
    match level.to_lowercase().as_str() {
        "error" | "critical" | "high" => "error",
        "warning" | "medium" => "warning",
        "note" | "info" | "low" => "note",
        _ => "warning",
    }
}

// ═══════════════════════════════════════════
// BASELINE DIFF
// ═══════════════════════════════════════════

/// Compare current SARIF results against a baseline and return only new/regressed.
pub fn diff_results(
    current: &[SarifResult],
    baseline: &[SarifResult],
) -> Vec<SarifResult> {
    let baseline_keys: std::collections::HashSet<String> = baseline.iter().map(|r| result_key(r)).collect();
    current
        .iter()
        .filter(|r| !baseline_keys.contains(&result_key(r)))
        .cloned()
        .collect()
}

fn result_key(result: &SarifResult) -> String {
    let location = result.locations.first()
        .map(|l| {
            let uri = l.physical_location.artifact_location.as_ref()
                .map(|a| a.uri.clone())
                .unwrap_or_default();
            let line = l.physical_location.region.as_ref()
                .and_then(|r| r.start_line)
                .unwrap_or(0);
            format!("{}:{}:{}", uri, line, result.rule_id)
        })
        .unwrap_or_else(|| result.rule_id.clone());
    location
}
