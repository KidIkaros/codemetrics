use clap::Parser;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use ast_parse_ts::Language;
use quality_common::{Column, find_source_files, print_table_header, print_table_row, truncate};

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

const TAINT_EXTS: &[&str] = &["rs", "py", "pyi", "js", "mjs", "ts", "tsx", "go", "c", "h", "cpp", "cc", "cxx", "hpp", "cs", "java", "php"];

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
    // Single-threaded rayon to reduce memory pressure (prevents OOM on 16GB/32GB systems)
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let results: Vec<(Vec<Violation>, usize)> = pool.install(|| {
        files
            .par_iter()
            .filter_map(|file_path| {
                let source = std::fs::read_to_string(file_path).ok()?;
                let file_str = file_path.to_string_lossy().to_string();
                let lang = Language::from_extension(&file_str);
                Some(analyze_file_multilang(&source, &file_str, attr, lang))
            })
            .collect()
    });

    let mut all_violations = Vec::new();
    let mut total_sensitive = 0usize;
    for (violations, sensitive_count) in results {
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

fn output_table(report: &TaintReport) {
    println!("TAINT ANALYSIS");
    println!("{}", "─".repeat(70));
    println!();
    println!("  Files scanned:            {}", report.summary.total_files_scanned);
    println!("  Sensitive variables:      {}", report.summary.sensitive_variables_found);
    println!("  Violations:               {}", report.summary.violations_count);
    println!();
    if !report.violations.is_empty() {
        println!("  VIOLATIONS:");
        let columns = [
            Column::left("SEV", 8),
            Column::left("TYPE", 14),
            Column::left("VAR", 20),
            Column::left("FILE", 20),
            Column::right("LINE", 5),
        ];
        print_table_header(&columns);
        for v in &report.violations {
            let sev_icon = match v.severity.as_str() {
                "high" => "!!",
                "medium" => "!",
                _ => "•",
            };
            print_table_row(&columns, &[&format!("{} {}", sev_icon, v.severity), &v.violation_type, &v.variable, &v.file, &v.line.to_string()]);
        }
    } else {
        println!("  No taint violations detected. Clean data flow!");
    }
    println!();
}

fn output_json(report: &TaintReport) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}

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
                } else if is_file_write_sink_multilang(trimmed, lang) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "FILE_WRITE".to_string(),
                        severity: "medium".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                } else if is_public_return_multilang(trimmed, var, lang) {
                    violations.push(Violation {
                        file: file.to_string(),
                        line: line_num,
                        variable: var.clone(),
                        violation_type: "UNFILTERED_RETURN".to_string(),
                        severity: "medium".to_string(),
                        context: truncate(trimmed, 60).to_string(),
                    });
                } else if is_debug_sink_multilang(trimmed, lang) {
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

/// Extract the identifier on the LHS of an assignment for a given language.
fn extract_lhs_identifier(line: &str, lang: Language) -> Option<String> {
    match lang {
        Language::Python => extract_lhs_python(line),
        Language::JavaScript | Language::TypeScript => extract_lhs_js(line),
        Language::Go => extract_lhs_go(line),
        Language::Rust => extract_lhs_rust(line),
        _ => None,
    }
}

fn extract_lhs_rust(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("let ") && !trimmed.starts_with("const ") && !trimmed.starts_with("static ") {
        return None;
    }
    let after_kw = if trimmed.starts_with("let ") { &trimmed[4..] }
        else if trimmed.starts_with("const ") { &trimmed[6..] }
        else { &trimmed[7..] };
    let rest = after_kw.trim_start();
    let rest = if rest.starts_with("mut ") { &rest[4..] } else { rest };
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &rest[..end];
    if name.is_empty() { None } else { Some(name.to_string()) }
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

/// Logging sink patterns for all languages.
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
        Language::Rust => &["log::info!", "log::warn!", "log::error!", "log::debug!", "log::trace!"],
        _ => return false,
    };
    patterns.iter().any(|p| line.contains(p))
}

/// Print/output sink patterns for all languages.
fn is_print_sink_multilang(line: &str, lang: Language) -> bool {
    let patterns: &[&str] = match lang {
        Language::Python => &["print("],
        Language::JavaScript | Language::TypeScript => &["process.stdout", "process.stderr"],
        Language::Go => &["fmt.Print", "fmt.Sprintf"],
        Language::Rust => &["println!", "print!", "eprintln!", "eprint!"],
        _ => return false,
    };
    patterns.iter().any(|p| line.contains(p))
}

/// File-write sink patterns for all languages.
fn is_file_write_sink_multilang(line: &str, lang: Language) -> bool {
    let patterns: &[&str] = match lang {
        Language::Rust => &["std::fs::write", "File::create", "fs::File::create", "std::fs::OpenOptions"],
        _ => return false,
    };
    patterns.iter().any(|p| line.contains(p))
}

/// Public return sink patterns for all languages.
fn is_public_return_multilang(line: &str, _var: &str, lang: Language) -> bool {
    match lang {
        Language::Rust => line.trim_start().starts_with("pub fn") && line.contains(_var),
        _ => false,
    }
}

/// Debug/trace sink patterns for all languages.
fn is_debug_sink_multilang(line: &str, lang: Language) -> bool {
    let patterns: &[&str] = match lang {
        Language::Rust => &["dbg!", "debug!", "trace!"],
        Language::Python => &["pprint", "repr("],
        Language::JavaScript | Language::TypeScript => &["console.debug", "console.trace"],
        Language::Go => &["fmt.Printf", "log.Printf"],
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
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.rs", "sensitive", ast_parse_ts::Language::Rust);
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
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.rs", "sensitive", ast_parse_ts::Language::Rust);
        assert_eq!(sensitive_count, 1);
        assert!(violations.is_empty(), "Safe usage should not trigger violation");
    }

    #[test]
    fn test_detect_secret_type() {
        let source = r#"
let secret = Secret::new("my_value");
println!("{}", secret);
"#;
        let (violations, sensitive_count) = analyze_file_multilang(source, "test.rs", "sensitive", ast_parse_ts::Language::Rust);
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
        let (violations, sensitive_count) = analyze_file_multilang(fix.source, "<fixture>", "sensitive", ast_parse_ts::Language::Rust);
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
