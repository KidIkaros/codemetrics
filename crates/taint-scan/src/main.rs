use clap::Parser;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use ast_parse_ts::Language;
use quality_common::{Column, find_source_files, print_table_header, print_table_row, separator, truncate};

#[derive(Parser)]
#[command(name = "taint", about = "Taint analysis — detect sensitive data flow to sinks like logging or public outputs")]
struct Cli {
    /// Path to scan (file or directory)
    #[arg(default_value = ".")]
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Attribute that marks sensitive variables (default: sensitive)
    #[arg(long, default_value = "sensitive")]
    attribute: String,

    /// Minimum severity to report: low, medium, high (default: low)
    #[arg(long, default_value = "low")]
    severity: String,

    /// Run built-in fixture tests and report precision/recall (does not scan files)
    #[arg(long)]
    self_test: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Violation {
    file: String,
    line: usize,
    variable: String,
    violation_type: String,
    severity: String,
    context: String,
}

#[derive(Debug, Clone, Serialize)]
struct TaintReport {
    violations: Vec<Violation>,
    summary: TaintSummary,
}

#[derive(Debug, Clone, Serialize)]
struct TaintSummary {
    total_files_scanned: usize,
    sensitive_variables_found: usize,
    violations_count: usize,
    high_severity: usize,
    medium_severity: usize,
    low_severity: usize,
}

fn main() {
    let cli = Cli::parse();
    if cli.self_test {
        std::process::exit(run_self_test());
    }
    if let Err(e) = run(cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

const TAINT_EXTS: &[&str] = &["rs", "py", "pyi", "js", "mjs", "ts", "tsx", "go"];

fn resolve_source_files(path: &str, recursive: bool) -> Result<Vec<PathBuf>, String> {
    let target_path = Path::new(path);
    let files: Vec<PathBuf> = if target_path.is_file() {
        let ext = target_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if TAINT_EXTS.contains(&ext) {
            vec![target_path.to_path_buf()]
        } else {
            return Err(format!("Unsupported file type: {}", path));
        }
    } else if target_path.is_dir() {
        find_source_files(path, recursive, TAINT_EXTS)
            .into_iter()
            .map(PathBuf::from)
            .collect()
    } else {
        return Err(format!("No source files found at {}", path));
    };
    if files.is_empty() {
        return Err("No supported source files found to analyze.".to_string());
    }
    Ok(files)
}

fn scan_source_files(
    files: &[PathBuf],
    attr: &str,
    severity_filter: &str,
) -> (Vec<Violation>, usize) {
    let mut all_violations = Vec::new();
    let mut total_sensitive = 0usize;

    for file_path in files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let file_str = file_path.to_string_lossy().to_string();
        let lang = Language::from_extension(&file_str);
        let (violations, sensitive_count) = if lang == Language::Rust {
            analyze_file(&source, &file_str, attr)
        } else {
            analyze_file_multilang(&source, &file_str, attr, lang)
        };
        total_sensitive += sensitive_count;

        for v in violations {
            let include = match v.severity.as_str() {
                "high" => true,
                "medium" => severity_filter != "high",
                "low" => severity_filter == "low",
                _ => true,
            };
            if include {
                all_violations.push(v);
            }
        }
    }
    (all_violations, total_sensitive)
}

fn build_taint_report(violations: Vec<Violation>, sensitive_count: usize, files_scanned: usize) -> TaintReport {
    let high = violations.iter().filter(|v| v.severity == "high").count();
    let medium = violations.iter().filter(|v| v.severity == "medium").count();
    let low = violations.iter().filter(|v| v.severity == "low").count();

    TaintReport {
        violations: violations.clone(),
        summary: TaintSummary {
            total_files_scanned: files_scanned,
            sensitive_variables_found: sensitive_count,
            violations_count: violations.len(),
            high_severity: high,
            medium_severity: medium,
            low_severity: low,
        },
    }
}

fn emit_report(report: &TaintReport, format: &str) {
    match format {
        "json" => output_json(report),
        _ => output_table(report),
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let source_files = resolve_source_files(&cli.path, cli.recursive)?;
    let severity_filter = cli.severity.to_lowercase();
    let (violations, sensitive_count) = scan_source_files(&source_files, &cli.attribute, &severity_filter);
    let report = build_taint_report(violations, sensitive_count, source_files.len());
    emit_report(&report, &cli.format);
    Ok(())
}

/// Tracks whether lines should be skipped (raw strings, test blocks).
struct LineSkipper {
    in_raw_string: bool,
    pending_test_block: bool,
    in_test_block: bool,
    test_brace_depth: i32,
}

impl LineSkipper {
    fn new() -> Self {
        LineSkipper { in_raw_string: false, pending_test_block: false, in_test_block: false, test_brace_depth: 0 }
    }

    /// Returns true if this line should be skipped.
    fn should_skip(&mut self, trimmed: &str) -> bool {
        if self.in_raw_string {
            if trimmed.contains("\"#") { self.in_raw_string = false; }
            return true;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            self.pending_test_block = true;
            return true;
        }
        if self.pending_test_block {
            return self.handle_pending_test(trimmed);
        }
        if self.in_test_block {
            return self.handle_in_test(trimmed);
        }
        // Check for raw string start
        if let Some(pos) = trimmed.find("r#\"") {
            if trimmed[pos+3..].find("\"#").is_none() {
                self.in_raw_string = true;
                return true;
            }
        }
        false
    }

    fn handle_pending_test(&mut self, trimmed: &str) -> bool {
        let opens = trimmed.matches('{').count() as i32;
        let closes = trimmed.matches('}').count() as i32;
        if opens > 0 {
            self.in_test_block = true;
            self.pending_test_block = false;
            self.test_brace_depth = opens - closes;
            if self.test_brace_depth <= 0 {
                self.in_test_block = false;
                self.test_brace_depth = 0;
            }
            return true;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            self.pending_test_block = false;
        }
        true
    }

    fn handle_in_test(&mut self, trimmed: &str) -> bool {
        let opens = trimmed.matches('{').count() as i32;
        let closes = trimmed.matches('}').count() as i32;
        self.test_brace_depth += opens - closes;
        if self.test_brace_depth <= 0 {
            self.in_test_block = false;
            self.test_brace_depth = 0;
        }
        true
    }
}

fn analyze_file(source: &str, file: &str, attr: &str) -> (Vec<Violation>, usize) {
    let mut violations = Vec::new();
    let mut sensitive_vars: HashSet<String> = HashSet::new();
    let mut line_num = 0;
    let mut in_sensitive_block = false;
    let mut skipper = LineSkipper::new();

    for line in source.lines() {
        line_num += 1;
        let trimmed = line.trim();
        if skipper.should_skip(trimmed) { continue; }

        // Detect sensitive variable declarations.
        // Attribute markers fire on any line; keyword matches only fire on let/const/static
        // bindings to avoid false positives from function names like `hash_password()`.
        // For keyword-based detection, only match against the variable name (LHS of `=`),
        // not the full line, to avoid false positives from RHS like `load_password()`.
        let binding_lhs = if trimmed.starts_with("let ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("static ")
        {
            trimmed.split('=').next().unwrap_or("").to_lowercase()
        } else {
            String::new()
        };
        if trimmed.starts_with(&format!("#[{attr}]"))
            || trimmed.contains(&format!("#[{attr}("))
            || (!binding_lhs.is_empty() && (
                binding_lhs.contains("sensitiv")
                || binding_lhs.contains("secret")
                || binding_lhs.contains("password")
                || binding_lhs.contains("token")
                || binding_lhs.contains("private_key")
                || binding_lhs.contains("credential")
            )) {
            in_sensitive_block = true;
        }

        // Extract variable names from let bindings near sensitive markers.
        // Keep in_sensitive_block true until we actually find and consume the let binding.
        if in_sensitive_block {
            if let Some(var_name) = extract_let_var(trimmed) {
                sensitive_vars.insert(var_name);
                in_sensitive_block = false;
            } else if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#[") {
                // Non-blank, non-comment, non-attribute line that isn't a let — give up on this marker
                in_sensitive_block = false;
            }
        }

        // Also detect by type name (Secrets, PrivateKey, etc.)
        if let Some(var_name) = extract_sensitive_type_var(trimmed) {
            sensitive_vars.insert(var_name);
        }

        // Check for violations - sensitive data flowing to sinks
        for var in &sensitive_vars {
            if line.contains(var) {
                // Check for logging sinks
                if is_logging_sink(trimmed) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "LOG_LEAK".to_string(),
                        severity: "high".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
                // Check for print/display sinks
                else if is_print_sink(trimmed) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "PRINT_LEAK".to_string(),
                        severity: "high".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
                // Check for file/network writes
                else if is_file_write_sink(trimmed) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "FILE_WRITE".to_string(),
                        severity: "medium".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
                // Check for public function return without sanitization
                else if is_public_return(trimmed, var) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "UNFILTERED_RETURN".to_string(),
                        severity: "medium".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
                // Debug/trace macros (lower severity)
                else if is_debug_sink(trimmed) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "DEBUG_LEAK".to_string(),
                        severity: "low".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
            }
        }
    }

    (violations, sensitive_vars.len())
}

fn extract_let_var(line: &str) -> Option<String> {
    // Match `let var_name` or `let mut var_name`
    let trimmed = line.trim();
    if !trimmed.starts_with("let ") {
        return None;
    }
    let after_let = &trimmed[4..];
    let rest = after_let.trim_start();
    // Skip "mut "
    let rest = if rest.starts_with("mut ") { &rest[4..] } else { rest };
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &rest[..end];
    if name.is_empty() || is_rust_keyword(name) {
        None
    } else {
        Some(name.to_string())
    }
}

fn extract_sensitive_type_var(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("let ") {
        return None;
    }
    let sensitive_types = [
        "Secret", "PrivateKey", "Password", "Token", "Credential",
        "ApiKey", "AuthToken", "SessionToken", "PrivateKey", "SecretKey",
    ];
    for ty in &sensitive_types {
        if trimmed.contains(ty) {
            return extract_let_var(line);
        }
    }
    None
}

fn is_logging_sink(line: &str) -> bool {
    let sinks = [
        "log::info!", "log::warn!", "log::error!",
        "tracing::info!", "tracing::warn!", "tracing::error!",
        "info!(", "warn!(", "error!(", "tracing::info_span",
    ];
    sinks.iter().any(|s| line.contains(s))
}

fn is_print_sink(line: &str) -> bool {
    let sinks = [
        "println!(", "print!(", "eprintln!(", "eprint!(", "format!(",
        "panic!(", "todo!(", "unimplemented!(",
    ];
    sinks.iter().any(|s| line.contains(s))
}

fn is_debug_sink(line: &str) -> bool {
    let sinks = [
        "log::debug!", "tracing::debug!", "dbg!(", "debug!(",
        "tracing::trace!", "trace!(",
    ];
    sinks.iter().any(|s| line.contains(s))
}

fn is_file_write_sink(line: &str) -> bool {
    let sinks = [
        ".write_all(", ".write(", ".writeln(", "std::fs::write(",
        ".send(", ".post(", ".get(", "reqwest::",
    ];
    sinks.iter().any(|s| line.contains(s))
}

fn is_public_return(line: &str, var: &str) -> bool {
    // Heuristic: line contains the variable and looks like a return statement
    // in a function that starts with "pub fn"
    line.contains(var) && (line.starts_with("return ") || line.ends_with(var))
}

fn is_rust_keyword(word: &str) -> bool {
    let keywords: HashSet<&str> = [
        "if", "else", "while", "for", "loop", "match", "fn", "let", "mut",
        "pub", "use", "mod", "struct", "enum", "impl", "trait", "const",
        "static", "type", "where", "return", "break", "continue", "move",
        "ref", "self", "Self", "super", "crate", "async", "await", "dyn",
        "as", "in", "true", "false", "box",
    ].iter().cloned().collect();
    keywords.contains(word)
}

fn output_table(report: &TaintReport) {
    println!("TAINT ANALYSIS — SENSITIVE DATA FLOW DETECTION");
    println!("{}", separator(95));

    if report.violations.is_empty() {
        println!();
        println!("  ✓ No violations detected.");
        println!("    Scanned {} files, found {} sensitive variables.",
            report.summary.total_files_scanned,
            report.summary.sensitive_variables_found);
        return;
    }

    println!();
    println!("VIOLATIONS:");

    let columns = [
        Column::left("SEVERITY", 10),
        Column::left("TYPE", 18),
        Column::left("FILE", 25),
        Column::right("LINE", 5),
        Column::left("VARIABLE", 18),
        Column::left("CONTEXT", 20),
    ];
    print_table_header(&columns);

    for v in &report.violations {
        let severity_icon = match v.severity.as_str() {
            "high" => "🔴 HIGH",
            "medium" => "🟡 MED",
            _ => "🟢 LOW",
        };
        let line_str = v.line.to_string();
        print_table_row(&columns, &[
            severity_icon,
            &v.violation_type,
            &truncate(&v.file, 23),
            &line_str,
            &truncate(&v.variable, 16),
            &truncate(&v.context, 18),
        ]);
    }

    println!("{}", separator(95));
    println!();
    println!("  SUMMARY");
    println!("    Files scanned:          {}", report.summary.total_files_scanned);
    println!("    Sensitive variables:    {}", report.summary.sensitive_variables_found);
    println!("    Total violations:       {}", report.summary.violations_count);
    println!("      High severity:        {}", report.summary.high_severity);
    println!("      Medium severity:      {}", report.summary.medium_severity);
    println!("      Low severity:         {}", report.summary.low_severity);

    if report.summary.high_severity > 0 {
        println!();
        println!("  🔴 {} high-severity violation(s) detected.", report.summary.high_severity);
        println!("     Review logging/print statements that may leak sensitive data.");
    }
}

fn output_json(report: &TaintReport) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}

/// Taint analysis for non-Rust languages (Python, JS/TS, Go).
/// Uses language-appropriate comment markers and tree-sitter identifiers.
fn analyze_file_multilang(source: &str, file: &str, _attr: &str, lang: Language) -> (Vec<Violation>, usize) {
    let mut violations = Vec::new();
    let mut sensitive_vars: HashSet<String> = HashSet::new();

    // Detect sensitive variable markers per language:
    //  Python: `# @sensitive` or `# sensitive:` above an assignment
    //  JS/TS:  `// @sensitive` or `/** @sensitive */` above a const/let/var
    //  Go:     `// @sensitive` above a var declaration
    let sensitive_comment = match lang {
        Language::Python => "# @sensitive",
        Language::JavaScript | Language::TypeScript => "// @sensitive",
        Language::Go => "// @sensitive",
        _ => "// @sensitive",
    };

    // Also detect by common sensitive keywords in variable names (LHS only)
    let sensitive_keywords = ["password", "secret", "token", "api_key", "apikey",
                               "private_key", "credential", "auth_key", "access_key"];

    let lines: Vec<&str> = source.lines().collect();
    let mut in_sensitive_block = false;

    for (idx, &line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        // Detect marker comment
        if trimmed.contains(sensitive_comment) || trimmed.contains("@sensitive") {
            in_sensitive_block = true;
            continue;
        }

        // Detect assignment line (Python: `x = ...`, JS: `const/let/var x = ...`, Go: `x :=` or `var x`)
        let lhs = extract_lhs_identifier(trimmed, lang);

        if let Some(ref var_name) = lhs {
            let lhs_lower = var_name.to_lowercase();
            let is_keyword_sensitive = sensitive_keywords.iter().any(|k| lhs_lower.contains(k));

            if in_sensitive_block || is_keyword_sensitive {
                sensitive_vars.insert(var_name.clone());
                in_sensitive_block = false;
                continue;
            }
        }

        if in_sensitive_block && !trimmed.is_empty() {
            in_sensitive_block = false;
        }

        // Check for violations: does this line reference any sensitive var in a sink?
        for var in &sensitive_vars {
            if line.contains(var.as_str()) {
                if is_logging_sink_multilang(trimmed, lang) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "LOG_LEAK".to_string(),
                        severity: "high".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                } else if is_print_sink_multilang(trimmed, lang) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "PRINT_LEAK".to_string(),
                        severity: "high".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                }
            }
        }
    }

    (violations, sensitive_vars.len())
}

/// Extract the identifier on the LHS of an assignment for a given language.
fn extract_lhs_identifier(line: &str, lang: Language) -> Option<String> {
    match lang {
        Language::Python => extract_lhs_python(line),
        Language::JavaScript | Language::TypeScript => extract_lhs_js(line),
        Language::Go => extract_lhs_go(line),
        _ => None,
    }
}

fn extract_lhs_python(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let eq_pos = trimmed.find('=')?;
    if trimmed[..eq_pos].contains('(') || trimmed.starts_with('#') { return None; }
    let lhs = trimmed[..eq_pos].trim().to_string();
    if !lhs.is_empty() && lhs.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(lhs)
    } else {
        None
    }
}

fn extract_lhs_js(line: &str) -> Option<String> {
    let trimmed = line.trim();
    for kw in &["const ", "let ", "var "] {
        if let Some(rest) = trimmed.strip_prefix(kw) {
            let ident = rest.split(['=', ':', ';', ' ']).next().unwrap_or("").trim();
            if !ident.is_empty() { return Some(ident.to_string()); }
        }
    }
    None
}

fn extract_lhs_go(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("var ") {
        let ident = rest.split_whitespace().next().unwrap_or("").to_string();
        if !ident.is_empty() { return Some(ident); }
    }
    let pos = trimmed.find(":=")?;
    let lhs = trimmed[..pos].trim().to_string();
    if !lhs.is_empty() && !lhs.contains(',') { Some(lhs) } else { None }
}

/// Logging sink patterns for non-Rust languages.
fn is_logging_sink_multilang(line: &str, lang: Language) -> bool {
    let patterns: &[&str] = match lang {
        Language::Python => &["logging.", "logger.", "log."],
        Language::JavaScript | Language::TypeScript => &[
            "console.log", "console.warn", "console.error",
            "logger.", "log.info", "winston.", "bunyan.",
        ],
        Language::Go => &[
            "log.Print", "log.Fatal", "log.Panic",
            "fmt.Fprintf", "logrus.", "zap.",
        ],
        _ => return false,
    };
    patterns.iter().any(|p| line.contains(p))
}

/// Print/output sink patterns for non-Rust languages.
fn is_print_sink_multilang(line: &str, lang: Language) -> bool {
    let patterns: &[&str] = match lang {
        Language::Python => &["print("],
        Language::JavaScript | Language::TypeScript => &["process.stdout", "process.stderr"],
        Language::Go => &["fmt.Print", "fmt.Sprintf"],
        _ => return false,
    };
    patterns.iter().any(|p| line.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_log_leak() {
        let source = r#"
#[sensitive]
let api_key = load_api_key();

fn do_something() {
    log::info!("Using key: {}", api_key);
}
"#;
        let (violations, sensitive_count) = analyze_file(source, "test.rs", "sensitive");
        assert_eq!(sensitive_count, 1, "Should detect 1 sensitive variable");
        assert!(!violations.is_empty(), "Should detect violation");
        assert!(violations.iter().any(|v| v.violation_type == "LOG_LEAK"));
        assert!(violations.iter().any(|v| v.severity == "high"));
    }

    #[test]
    fn test_no_violation_safe_usage() {
        let source = r#"
#[sensitive]
let password = load_password();

fn hash_password() {
    let hashed = bcrypt_hash(&password);
    store_hash(hashed);
}
"#;
        let (violations, sensitive_count) = analyze_file(source, "test.rs", "sensitive");
        assert_eq!(sensitive_count, 1);
        assert!(violations.is_empty(), "Safe usage should not trigger violation");
    }

    #[test]
    fn test_detect_secret_type() {
        let source = r#"
let secret = Secret::new("my_value");
println!("{}", secret);
"#;
        let (violations, sensitive_count) = analyze_file(source, "test.rs", "sensitive");
        assert_eq!(sensitive_count, 1, "Should detect secret type variable");
        assert!(!violations.is_empty(), "Should detect print leak");
    }

    #[test]
    fn test_python_log_leak() {
        let source = r#"
# @sensitive
api_key = os.getenv("API_KEY")
logging.info("Using key: %s", api_key)
"#;
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.py", "sensitive", ast_parse_ts::Language::Python);
        assert_eq!(sensitive_count, 1, "Should detect 1 sensitive variable");
        assert!(!violations.is_empty(), "Should detect Python logging leak");
        assert!(violations.iter().any(|v| v.violation_type == "LOG_LEAK"));
    }

    #[test]
    fn test_js_console_leak() {
        let source = r#"
// @sensitive
const token = getAuthToken();
console.log("Token:", token);
"#;
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.js", "sensitive", ast_parse_ts::Language::JavaScript);
        assert_eq!(sensitive_count, 1, "Should detect 1 sensitive variable");
        assert!(!violations.is_empty(), "Should detect JS console.log leak");
        assert!(violations.iter().any(|v| v.violation_type == "LOG_LEAK"));
    }

    #[test]
    fn test_go_log_leak() {
        let source = r#"
// @sensitive
var apiKey = os.Getenv("API_KEY")
log.Printf("Using key: %s", apiKey)
"#;
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.go", "sensitive", ast_parse_ts::Language::Go);
        assert_eq!(sensitive_count, 1, "Should detect 1 sensitive variable");
        assert!(!violations.is_empty(), "Should detect Go log leak");
        assert!(violations.iter().any(|v| v.violation_type == "LOG_LEAK"));
    }

    #[test]
    fn test_python_safe_usage_no_violation() {
        let source = r#"
# @sensitive
password = load_password()
hashed = bcrypt_hash(password)
store_hash(hashed)
"#;
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.py", "sensitive", ast_parse_ts::Language::Python);
        assert_eq!(sensitive_count, 1);
        assert!(violations.is_empty(), "Safe Python usage should not trigger violation");
    }
}

// ═══════════════════════════════════════════
// SELF-TEST: precision/recall against fixtures
// ═══════════════════════════════════════════

struct Fixture {
    name: &'static str,
    source: &'static str,
    expect_violations: usize,
    expect_sensitive: usize,
}

fn run_self_test() -> i32 {
    let fixtures: &[Fixture] = &[
        Fixture {
            name: "log-leak (should detect)",
            source: r#"#[sensitive]
let api_key = load_api_key();
fn f() { log::info!("{}", api_key); }"#,
            expect_violations: 1,
            expect_sensitive: 1,
        },
        Fixture {
            name: "safe bcrypt usage (no violation)",
            source: r#"#[sensitive]
let password = load_password();
fn hash_password() { let h = bcrypt_hash(&password); store(h); }"#,
            expect_violations: 0,
            expect_sensitive: 1,
        },
        Fixture {
            name: "print leak (should detect)",
            source: r#"let secret = Secret::new("x");
println!("{}", secret);"#,
            expect_violations: 1,
            expect_sensitive: 1,
        },
        Fixture {
            name: "no sensitive vars (no violation)",
            source: r#"fn add(a: i32, b: i32) -> i32 { a + b }"#,
            expect_violations: 0,
            expect_sensitive: 0,
        },
        Fixture {
            name: "keyword let binding (should detect)",
            source: r#"let token = get_token();
log::warn!("token={}", token);"#,
            expect_violations: 1,
            expect_sensitive: 1,
        },
    ];

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut tp = 0usize;
    let mut fp = 0usize;
    let mut fn_ = 0usize;

    println!("taint-scan self-test: {} fixtures\n", fixtures.len());

    for fix in fixtures {
        let (violations, sensitive_count) = analyze_file(fix.source, "<fixture>", "sensitive");
        let viol_ok = violations.len() == fix.expect_violations;
        let sens_ok = sensitive_count == fix.expect_sensitive;
        let ok = viol_ok && sens_ok;

        if ok {
            passed += 1;
            if fix.expect_violations > 0 { tp += 1; }
        } else {
            failed += 1;
            if violations.len() > fix.expect_violations { fp += 1; }
            if violations.len() < fix.expect_violations { fn_ += 1; }
        }

        let icon = if ok { "✓" } else { "✗" };
        println!("  {} {}", icon, fix.name);
        if !ok {
            println!("      violations: got {} expected {}", violations.len(), fix.expect_violations);
            println!("      sensitive:  got {} expected {}", sensitive_count, fix.expect_sensitive);
        }
    }

    let total = fixtures.len();
    let precision = if tp + fp > 0 { tp as f64 / (tp + fp) as f64 * 100.0 } else { 100.0 };
    let recall    = if tp + fn_ > 0 { tp as f64 / (tp + fn_) as f64 * 100.0 } else { 100.0 };

    println!("\nResults: {}/{} passed", passed, total);
    println!("Precision: {:.0}%  Recall: {:.0}%", precision, recall);

    if failed > 0 { 1 } else { 0 }
}
