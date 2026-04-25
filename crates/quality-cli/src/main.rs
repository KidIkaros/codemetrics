use clap::{Parser, Subcommand};
use serde::Serialize;
use std::time::Instant;

use ast_parse::{analyze_file, find_coverage, parse_lcov, crap_score};
use ast_parse_ts::{parse_complexity_file, parse_doc_coverage_file, Language};
use quality_common::{find_rust_files, find_source_files, ToolResult};

// ═══════════════════════════════════════════
// CLI DEFINITION
// ═══════════════════════════════════════════

#[derive(Parser)]
#[command(
    name = "quality",
    about = "Unified code quality checker for Rust. Headless-first, JSON output, CI-ready.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all quality checks and report results
    Check {
        /// Path to analyze
        path: String,

        /// Recursive scan
        #[arg(short, long)]
        recursive: bool,

        /// Output format: json (default) or text
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Path to lcov coverage file
        #[arg(long)]
        coverage: Option<String>,

        /// Max average CRAP score (fail if exceeded)
        #[arg(long, default_value = "30")]
        max_crap: f64,

        /// Min doc coverage percentage (fail if below)
        #[arg(long, default_value = "50")]
        min_doc: f64,

        /// Max technical debt markers (fail if exceeded)
        #[arg(long, default_value = "100")]
        max_debt: usize,

        /// Skip specific checks (comma-separated: crap,debt,doc,dup,complexity)
        #[arg(long)]
        skip: Option<String>,
    },

    /// CRAP metric only
    Crap {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long)]
        coverage: Option<String>,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Technical debt only
    Debt {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long)]
        marker: Option<String>,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Documentation coverage only
    Doccov {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Code duplication only
    Dupfind {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long, default_value = "5")]
        min_lines: usize,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Cyclomatic complexity report
    Complexity {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long, default_value = "5")]
        min_complexity: u32,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Generate default config file
    Init {
        /// Output path (default: .quality.toml)
        #[arg(long, default_value = ".quality.toml")]
        output: String,
    },

    /// Run all quality tools in batch mode using .quality.toml config
    Run {
        /// Path to the crate root (directory with Cargo.toml)
        path: String,

        /// Config file (default: .quality.toml)
        #[arg(long, default_value = ".quality.toml")]
        config: String,

        /// Output format (table, json, or sarif)
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Baseline SARIF/JSON file: only emit new/regressed results
        #[arg(long)]
        baseline: Option<String>,

        /// Do not exit 1 on baseline regression (useful for seeding a new baseline)
        #[arg(long)]
        no_fail_on_regression: bool,
    },

    /// Record or display quality metrics history
    History {
        /// Action: record (append current run to history) or show (print trend table)
        #[arg(default_value = "show")]
        action: String,

        /// History directory (default: .quality-history)
        #[arg(long, default_value = ".quality-history")]
        dir: String,

        /// Number of recent runs to show
        #[arg(long, default_value = "10")]
        last: usize,

        /// Path to a JSON run report to record (default: stdin)
        #[arg(long)]
        report: Option<String>,
    },

    /// Install a quality pre-commit git hook
    InstallHooks {
        /// Git repo root (default: current directory)
        #[arg(default_value = ".")]
        repo: String,
    },

    /// Remove the quality pre-commit git hook
    UninstallHooks {
        /// Git repo root (default: current directory)
        #[arg(default_value = ".")]
        repo: String,
    },

    /// Watch for file changes and re-run relevant quality checks
    Watch {
        /// Path to watch
        #[arg(default_value = ".")]
        path: String,

        /// Which checks to run on change (comma-separated: crap,debt,doc,complexity)
        #[arg(long, default_value = "debt,doc,crap")]
        checks: String,

        /// Debounce delay in milliseconds
        #[arg(long, default_value = "500")]
        debounce_ms: u64,
    },
}

// ═══════════════════════════════════════════
// RESULT TYPES
// ═══════════════════════════════════════════

#[derive(Serialize)]
struct CheckReport {
    passed: bool,
    path: String,
    checks: Vec<CheckResult>,
    summary: CheckSummary,
}

#[derive(Serialize)]
struct CheckResult {
    name: String,
    passed: bool,
    score: Option<f64>,
    threshold: Option<f64>,
    message: String,
    details: serde_json::Value,
}

#[derive(Serialize)]
struct CheckSummary {
    total_checks: usize,
    passed_checks: usize,
    failed_checks: usize,
    functions_analyzed: usize,
    avg_complexity: f64,
    avg_crap: f64,
}

// ═══════════════════════════════════════════
// CHECKS
// ═══════════════════════════════════════════

/// Scan all Rust files under `path`, invoking `predicate` on each function.
/// Returns `(total_functions_count, collected_items)`.
fn scan_rust_functions<T, F>(path: &str, recursive: bool, mut predicate: F) -> (usize, Vec<T>)
where
    F: FnMut(&ast_parse::FunctionComplexity) -> Option<T>,
{
    let files = find_rust_files(path, recursive);
    let mut total = 0;
    let mut items = Vec::new();
    for file in &files {
        if let Ok(analysis) = analyze_file(file) {
            for func in &analysis.functions {
                total += 1;
                if let Some(item) = predicate(func) {
                    items.push(item);
                }
            }
        }
    }
    (total, items)
}

fn check_crap(path: &str, recursive: bool, coverage_path: &Option<String>, max_crap: f64) -> CheckResult {
    let coverage_data: Option<Vec<ast_parse::FileCoverage>> = coverage_path.as_ref().map(|p| parse_lcov(p).unwrap_or_default());
    let (total, functions) = scan_rust_functions(path, recursive, |func| {
        let cov_pct = if let Some(ref cov_data) = coverage_data {
            if let Some(cov) = find_coverage(cov_data, &func.file) {
                let (_, _, func_cov) = cov.range_coverage(func.line, func.end_line);
                if func_cov > 0.0 || !cov.da_records.is_empty() {
                    func_cov
                } else {
                    cov.coverage_pct()
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        let score = crap_score(func.complexity, cov_pct);
        Some((func.name.clone(), func.complexity, cov_pct, score))
    });
    let avg_crap = if total > 0 {
        functions.iter().map(|f| f.3).sum::<f64>() / total as f64
    } else {
        0.0
    };
    let crappy: Vec<_> = functions.iter().filter(|f| f.3 > 30.0).collect();

    CheckResult {
        name: "crap".to_string(),
        passed: avg_crap <= max_crap,
        score: Some(avg_crap),
        threshold: Some(max_crap),
        message: if avg_crap <= max_crap {
            format!("Average CRAP {:.1} <= {:.0}", avg_crap, max_crap)
        } else {
            format!("Average CRAP {:.1} > {:.0} ({} functions above 30)", avg_crap, max_crap, crappy.len())
        },
        details: serde_json::json!({
            "total_functions": total,
            "avg_crap": avg_crap,
            "crappy_count": crappy.len(),
            "excellent_count": functions.iter().filter(|f| f.3 <= 10.0).count(),
            "top_offenders": crappy.iter().take(5).map(|f| {
                serde_json::json!({
                    "name": f.0, "complexity": f.1, "coverage": f.2, "crap": f.3
                })
            }).collect::<Vec<_>>(),
        }),
    }
}

fn check_debt(path: &str, recursive: bool, max_debt: usize) -> CheckResult {
    let extensions = ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "java"];
    let files = find_source_files(path, recursive, &extensions);

    let markers = ["TODO", "FIXME", "HACK", "XXX", "BUG"];
    let mut count = 0;
    let mut items = Vec::new();

    for file in &files {
        if let Ok(source) = std::fs::read_to_string(file) {
            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                    for marker in &markers {
                        if trimmed.contains(marker) {
                            count += 1;
                            items.push(serde_json::json!({
                                "file": file, "line": line_num + 1, "type": marker
                            }));
                        }
                    }
                }
            }
        }
    }

    CheckResult {
        name: "debt".to_string(),
        passed: count <= max_debt,
        score: Some(count as f64),
        threshold: Some(max_debt as f64),
        message: if count <= max_debt {
            format!("{} debt markers <= {}", count, max_debt)
        } else {
            format!("{} debt markers > {}", count, max_debt)
        },
        details: serde_json::json!({
            "total_markers": count,
            "items": items.iter().take(20).collect::<Vec<_>>(),
        }),
    }
}

use syn::visit::Visit;
use syn::{ImplItemFn, ItemEnum, ItemFn, ItemStruct, ItemTrait, Visibility};

struct DocCounter { total: usize, documented: usize }
impl<'a> Visit<'a> for DocCounter {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        if matches!(node.vis, Visibility::Public(_)) {
            self.total += 1;
            if node.attrs.iter().any(|a| a.path().is_ident("doc")) { self.documented += 1; }
        }
    }
    fn visit_item_struct(&mut self, node: &'a ItemStruct) {
        if matches!(node.vis, Visibility::Public(_)) {
            self.total += 1;
            if node.attrs.iter().any(|a| a.path().is_ident("doc")) { self.documented += 1; }
        }
    }
    fn visit_item_enum(&mut self, node: &'a ItemEnum) {
        if matches!(node.vis, Visibility::Public(_)) {
            self.total += 1;
            if node.attrs.iter().any(|a| a.path().is_ident("doc")) { self.documented += 1; }
        }
    }
    fn visit_item_trait(&mut self, node: &'a ItemTrait) {
        if matches!(node.vis, Visibility::Public(_)) {
            self.total += 1;
            if node.attrs.iter().any(|a| a.path().is_ident("doc")) { self.documented += 1; }
        }
    }
    fn visit_impl_item_fn(&mut self, node: &'a ImplItemFn) {
        if matches!(node.vis, Visibility::Public(_)) {
            self.total += 1;
            if node.attrs.iter().any(|a| a.path().is_ident("doc")) { self.documented += 1; }
        }
    }
}

fn check_doc_coverage(path: &str, recursive: bool, min_doc: f64) -> CheckResult {
    let mut total = 0usize;
    let mut documented = 0usize;
    let mut langs_seen: std::collections::HashSet<String> = Default::default();

    // Rust files via syn (high-fidelity)
    let rust_files = find_rust_files(path, recursive);
    if !rust_files.is_empty() {
        langs_seen.insert("rust".to_string());
    }
    let mut counter = DocCounter { total: 0, documented: 0 };
    for file in &rust_files {
        if let Ok(source) = std::fs::read_to_string(file) {
            if let Ok(ast) = syn::parse_file(&source) {
                counter.visit_file(&ast);
            }
        }
    }
    total += counter.total;
    documented += counter.documented;

    // Non-Rust files via tree-sitter
    let all_exts = ["py", "pyi", "js", "mjs", "ts", "tsx", "go"];
    let other_files: Vec<String> = find_source_files(path, recursive, &all_exts)
        .into_iter()
        .filter(|f| !f.ends_with(".rs"))
        .collect();
    for file in &other_files {
        let lang = Language::from_extension(file);
        let stats = parse_doc_coverage_file(file);
        if stats.total_public > 0 {
            langs_seen.insert(lang.to_string());
        }
        total += stats.total_public;
        documented += stats.documented;
    }

    let pct = if total > 0 {
        documented as f64 / total as f64 * 100.0
    } else {
        100.0
    };

    let mut langs_vec: Vec<String> = langs_seen.into_iter().collect();
    langs_vec.sort();

    CheckResult {
        name: "doc_coverage".to_string(),
        passed: pct >= min_doc,
        score: Some(pct),
        threshold: Some(min_doc),
        message: if pct >= min_doc {
            format!("Doc coverage {:.0}% >= {:.0}% (langs: {})", pct, min_doc, langs_vec.join(", "))
        } else {
            format!("Doc coverage {:.0}% < {:.0}% (langs: {})", pct, min_doc, langs_vec.join(", "))
        },
        details: serde_json::json!({
            "total_public": total,
            "documented": documented,
            "coverage_pct": pct,
            "languages": langs_vec,
        }),
    }
}

fn check_complexity(path: &str, recursive: bool, min_complexity: u32) -> CheckResult {
    let all_exts = ["rs", "py", "js", "ts", "mjs", "tsx", "go"];
    let files = find_source_files(path, recursive, &all_exts);

    let mut total = 0usize;
    let mut complex_funcs: Vec<serde_json::Value> = Vec::new();
    let mut langs_seen: std::collections::HashSet<String> = Default::default();

    for file in &files {
        let lang = Language::from_extension(file);
        langs_seen.insert(lang.to_string());
        let funcs = parse_complexity_file(file);
        for func in funcs {
            total += 1;
            if func.complexity >= min_complexity {
                complex_funcs.push(serde_json::json!({
                    "name": func.name,
                    "file": func.file,
                    "line": func.line,
                    "complexity": func.complexity,
                    "language": func.language.to_string(),
                }));
            }
        }
    }

    let mut langs_vec: Vec<String> = langs_seen.into_iter().collect();
    langs_vec.sort();

    CheckResult {
        name: "complexity".to_string(),
        passed: complex_funcs.is_empty(),
        score: Some(complex_funcs.len() as f64),
        threshold: Some(0.0),
        message: if complex_funcs.is_empty() {
            format!("No functions above complexity threshold (languages: {})", langs_vec.join(", "))
        } else {
            format!("{} functions with complexity >= {} (languages: {})", complex_funcs.len(), min_complexity, langs_vec.join(", "))
        },
        details: serde_json::json!({
            "total_functions": total,
            "complex_count": complex_funcs.len(),
            "languages": langs_vec,
            "functions": complex_funcs.iter().take(10).collect::<Vec<_>>(),
        }),
    }
}

// ═══════════════════════════════════════════
// OUTPUT FORMATTERS
// ═══════════════════════════════════════════

fn output_json(report: &CheckReport) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}

fn output_text(report: &CheckReport) {
    println!("QUALITY CHECK: {}", if report.passed { "PASSED" } else { "FAILED" });
    println!("Path: {}", report.path);
    println!("{}", "─".repeat(60));

    for check in &report.checks {
        let icon = if check.passed { "✓" } else { "✗" };
        let score_str = check.score.map(|s| format!("{:.1}", s)).unwrap_or_default();
        let thresh_str = check.threshold.map(|t| format!("{:.0}", t)).unwrap_or_default();

        println!("  {} {:<15} {:>8} (threshold: {}) — {}",
            icon, check.name, score_str, thresh_str, check.message);
    }

    println!("{}", "─".repeat(60));
    println!("  Checks: {}/{} passed",
        report.summary.passed_checks, report.summary.total_checks);
    println!("  Functions: {}", report.summary.functions_analyzed);
    println!("  Avg complexity: {:.1}", report.summary.avg_complexity);
    println!("  Avg CRAP: {:.1}", report.summary.avg_crap);
}

// ═══════════════════════════════════════════
// CONFIG
// ═══════════════════════════════════════════

fn generate_config(output: &str) {
    let config = r#"# .quality.toml -- Quality check thresholds
# Used by: quality check ./src --config .quality.toml

[crap]
max_avg = 30           # Fail if average CRAP exceeds this
max_functions = 50     # Fail if more than N functions have CRAP > 30

[debt]
max_markers = 100      # Fail if too many debt markers found
types = ["TODO", "FIXME", "HACK"]

[doc_coverage]
min_pct = 50           # Fail if public API doc coverage below this

[complexity]
max_function = 10      # Warn if any function has complexity above this

[skip]
checks = []            # Skip these checks: crap, debt, doc, complexity
"#;
    std::fs::write(output, config).expect("Failed to write config");
}

// MAIN
// ═══════════════════════════════════════════

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Check {
            path, recursive, format, coverage,
            max_crap, min_doc, max_debt, skip,
        } => {
            let skip_list: Vec<String> = skip
                .map(|s| s.split(',').map(|s| s.trim().to_lowercase()).collect())
                .unwrap_or_default();

            let should_run = |name: &str| -> bool {
                !skip_list.contains(&name.to_string())
            };

            let mut checks = Vec::new();

            if should_run("crap") {
                checks.push(check_crap(&path, recursive, &coverage, max_crap));
            }
            if should_run("debt") {
                checks.push(check_debt(&path, recursive, max_debt));
            }
            if should_run("doc") {
                checks.push(check_doc_coverage(&path, recursive, min_doc));
            }
            if should_run("complexity") {
                checks.push(check_complexity(&path, recursive, 10));
            }

            let passed = checks.iter().all(|c| c.passed);
            let total_funcs: usize = checks.iter()
                .filter_map(|c| c.details.get("total_functions").and_then(|v| v.as_u64()))
                .map(|v| v as usize)
                .sum();

            let passed_count = checks.iter().filter(|c| c.passed).count();
            let failed_count = checks.len() - passed_count;

            let report = CheckReport {
                passed,
                path: path.clone(),
                checks,
                summary: CheckSummary {
                    total_checks: 4,
                    passed_checks: passed_count,
                    failed_checks: failed_count,
                    functions_analyzed: total_funcs,
                    avg_complexity: 0.0,
                    avg_crap: 0.0,
                },
            };

            match format.as_str() {
                "text" => output_text(&report),
                "ndjson" => output_ndjson(&report),
                _ => output_json(&report),
            }

            if passed { 0 } else { 1 }
        }

        Commands::Crap { path, recursive, coverage, format } => {
            let result = check_crap(&path, recursive, &coverage, 30.0);
            let passed = result.passed;
            match format.as_str() {
                "text" => println!("{}", result.message),
                _ => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
            }
            if passed { 0 } else { 1 }
        }

        Commands::Debt { path, recursive, marker: _, format } => {
            let result = check_debt(&path, recursive, 1000);
            let passed = result.passed;
            match format.as_str() {
                "text" => println!("{}", result.message),
                _ => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
            }
            if passed { 0 } else { 1 }
        }

        Commands::Doccov { path, recursive, format } => {
            let result = check_doc_coverage(&path, recursive, 0.0);
            let passed = result.passed;
            match format.as_str() {
                "text" => println!("{}", result.message),
                _ => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
            }
            if passed { 0 } else { 1 }
        }

        Commands::Dupfind { .. } => {
            eprintln!("dupfind subcommand not yet integrated -- use dupfind binary directly");
            2
        }

        Commands::Complexity { path, recursive, min_complexity, format } => {
            let result = check_complexity(&path, recursive, min_complexity);
            let passed = result.passed;
            match format.as_str() {
                "text" => println!("{}", result.message),
                _ => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
            }
            if passed { 0 } else { 1 }
        }

        Commands::Init { output } => {
            generate_config(&output);
            0
        }

        Commands::Run { path, config, format, baseline, no_fail_on_regression } => {
            run_batch(&path, &config, &format, baseline.as_deref(), no_fail_on_regression)
        }

        Commands::History { action, dir, last, report } => {
            history_command(&action, &dir, last, report.as_deref())
        }

        Commands::InstallHooks { repo } => {
            install_hooks(&repo)
        }

        Commands::UninstallHooks { repo } => {
            uninstall_hooks(&repo)
        }

        Commands::Watch { path, checks, debounce_ms } => {
            watch_mode(&path, &checks, debounce_ms)
        }
    };

    std::process::exit(exit_code);
}

fn run_tool(
    crate_name: &str,
    bin_name: &str,
    args: &[&str],
    tool_start: Instant,
) -> ToolResult {
    use quality_common::*;
    use std::process::{Command, Stdio};

    let output = Command::new(bin_name)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match output {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => o,
        _ => {
            let cargo_output = Command::new("cargo")
                .args(["run", "--quiet", "-p", crate_name, "--bin", bin_name, "--"])
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            match cargo_output {
                Ok(o) => o,
                Err(e) => {
                    return ToolResult {
                        tool: bin_name.to_string(),
                        success: false,
                        duration_ms: tool_start.elapsed().as_millis() as u64,
                        data: serde_json::Value::Null,
                        error: Some(format!("Failed to run: {}", e)),
                    };
                }
            }
        }
    };

    let duration_ms = tool_start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let (data, error) = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => (json, None),
        Err(_) => {
            let trimmed = stdout.trim();
            if trimmed.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                (
                    serde_json::Value::Null,
                    Some(format!("No output. stderr: {}", stderr.trim())),
                )
            } else {
                (serde_json::json!({ "raw": trimmed }), None)
            }
        }
    };

    ToolResult {
        tool: bin_name.to_string(),
        success: error.is_none() && output.status.success(),
        duration_ms,
        data,
        error,
    }
}

fn run_batch(path: &str, _config: &str, format: &str, baseline: Option<&str>, no_fail_on_regression: bool) -> i32 {
    use quality_common::*;
    
    use std::time::Instant;

    let start = Instant::now();

    let tools: Vec<(&str, &str, Vec<&str>)> = vec![
        ("debt-scan", "debt", vec!["--recursive", path, "--format", "json"]),
        ("doc-coverage", "doccov", vec!["--recursive", path, "--format", "json"]),
        ("crap-metric", "crap", vec!["--recursive", path, "--format", "json"]),
        ("coupling", "coupling", vec![path, "--format", "json"]),
        ("risk-map", "riskmap", vec![path, "--format", "json"]),
        ("duplication", "dupfind", vec!["--recursive", path, "--format", "json"]),
        ("prop-cov", "propcov", vec!["--recursive", path, "--format", "json"]),
    ];

    // Spawn all tools concurrently (cap at 4 simultaneous to respect CI RAM limits)
    const MAX_CONCURRENT: usize = 4;
    let mut handles: Vec<std::sync::mpsc::Receiver<ToolResult>> = Vec::new();
    let mut results: Vec<ToolResult> = Vec::new();
    for (crate_name, bin_name, args) in &tools {
        // Throttle: wait if we already have MAX_CONCURRENT running
        while handles.len() >= MAX_CONCURRENT {
            let mut done_results = Vec::new();
            for (i, handle) in handles.iter_mut().enumerate() {
                if let Ok(result) = handle.try_recv() {
                    done_results.push((i, result));
                    break; // check one at a time to avoid busy-wait
                }
            }
            if done_results.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            } else {
                // Remove completed handles (reverse order to preserve indices)
                for (i, _) in done_results.iter().rev() {
                    handles.remove(*i);
                }
                // Save the results received during throttling
                for (_, result) in done_results {
                    results.push(result);
                }
            }
        }

        let c = crate_name.to_string();
        let b = bin_name.to_string();
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let tool_start = Instant::now();
            let a_refs: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
            let result = run_tool(&c, &b, &a_refs, tool_start);
            let _ = tx.send(result);
        });
        handles.push(rx);
    }

    // Collect remaining results
    for rx in handles {
        if let Ok(result) = rx.recv() {
            results.push(result);
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;

    // Baseline handling: must check before moving results into report
    let mut regression_detected = false;
    if let Some(baseline_file) = baseline {
        if let Ok(baseline_content) = std::fs::read_to_string(baseline_file) {
            if let Ok(baseline_report) = serde_json::from_str::<UnifiedReport>(&baseline_content) {
                let baseline_tools: std::collections::HashSet<String> =
                    baseline_report.tools.iter().filter(|t| t.success).map(|t| t.tool.clone()).collect();
                let current_tools: std::collections::HashSet<String> =
                    results.iter().filter(|t| t.success).map(|t| t.tool.clone()).collect();
                let regressed: Vec<String> = baseline_tools.difference(&current_tools).cloned().collect();
                if !regressed.is_empty() {
                    eprintln!("BASELINE REGRESSION: previously-passing tools now failing: {:?}", regressed);
                    if !no_fail_on_regression {
                        regression_detected = true;
                    }
                }
            }
        }
    }

    match format {
        "sarif" => {
            // Build SARIF from results
            let mut log = SarifLog::new("quality", env!("CARGO_PKG_VERSION"));
            let mut sarif_results: Vec<SarifResult> = Vec::new();

            for tool in &results {
                if !tool.success {
                    sarif_results.push(SarifResult {
                        rule_id: format!("{}-error", tool.tool),
                        rule_index: None,
                        level: "error".to_string(),
                        message: SarifMessage {
                            text: tool.error.clone().unwrap_or_else(|| format!("{} failed", tool.tool)),
                        },
                        locations: vec![SarifLocation {
                            physical_location: SarifPhysicalLocation {
                                artifact_location: Some(SarifArtifactLocation { uri: path.to_string() }),
                                region: None,
                            },
                        }],
                    });
                }
            }

            let run = sarif_run(
                "quality-batch",
                env!("CARGO_PKG_VERSION"),
                sarif_results,
                if failed > 0 { 1 } else { 0 },
            );
            log.add_run(run);
            println!("{}", serde_json::to_string_pretty(&log).unwrap());
        }
        "json" => {
            let report = new_unified_report(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string(),
            );
            // Detect languages from source files at path
            let all_exts = ["rs", "py", "pyi", "js", "mjs", "cjs", "ts", "tsx", "mts", "go"];
            let mut langs_detected: Vec<String> = find_source_files(path, true, &all_exts)
                .iter()
                .map(|f| ast_parse_ts::Language::from_extension(f).to_string())
                .filter(|l| l != "unknown")
                .collect::<std::collections::HashSet<String>>()
                .into_iter()
                .collect();
            langs_detected.sort();
            let report = UnifiedReport {
                run_id: report.run_id,
                started_at: report.started_at,
                duration_ms,
                tools: results,
                summary: ReportSummary {
                    total_tools: tools.len(),
                    passed,
                    failed,
                    languages_detected: langs_detected,
                },
            };
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        _ => {
            println!("\n═══════════════════════════════════════════");
            println!("  QUALITY BATCH REPORT");
            println!("  Run ID: (table mode)");
            println!("  Duration: {}ms", duration_ms);
            println!("═══════════════════════════════════════════");
            for tool in &results {
                let status = if tool.success { "PASS" } else { "FAIL" };
                println!("  {:15} {:5}  {:>6}ms", tool.tool, status, tool.duration_ms);
                if let Some(ref err) = tool.error {
                    println!("    ERROR: {}", err);
                }
            }
            println!("───────────────────────────────────────────");
            println!("  Total: {}  Passed: {}  Failed: {}",
                tools.len(),
                passed,
                failed,
            );
            println!("═══════════════════════════════════════════\n");
        }
    }

    if failed > 0 || regression_detected { 1 } else { 0 }
}

// ═══════════════════════════════════════════
// NDJSON OUTPUT
// ═══════════════════════════════════════════

fn output_ndjson(report: &CheckReport) {
    for check in &report.checks {
        if !check.passed {
            let items = check.details.get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                println!("{}", serde_json::json!({
                    "tool": check.name,
                    "severity": "warning",
                    "message": check.message,
                    "file": report.path,
                    "line": null,
                    "col": null,
                }));
            } else {
                for item in &items {
                    println!("{}", serde_json::json!({
                        "tool": check.name,
                        "severity": "warning",
                        "message": item.get("type").and_then(|v| v.as_str()).unwrap_or(&check.name),
                        "file": item.get("file"),
                        "line": item.get("line"),
                        "col": null,
                    }));
                }
            }
        }
    }
}

// ═══════════════════════════════════════════
// HISTORY
// ═══════════════════════════════════════════

fn history_command(action: &str, dir: &str, last: usize, report_path: Option<&str>) -> i32 {
    match action {
        "record" => history_record(dir, report_path),
        "show" | _ => history_show(dir, last),
    }
}

fn history_record(dir: &str, report_path: Option<&str>) -> i32 {
    use std::io::Read;

    let json_str = if let Some(path) = report_path {
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => { eprintln!("history record: cannot read {}: {}", path, e); return 1; }
        }
    } else {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_err() {
            eprintln!("history record: failed to read stdin");
            return 1;
        }
        buf
    };

    let report: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => { eprintln!("history record: invalid JSON: {}", e); return 1; }
    };

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let date = chrono_yymm(ts);

    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("history record: cannot create {}: {}", dir, e);
        return 1;
    }

    let path = format!("{}/{}.jsonl", dir, date);
    let tools_summary: serde_json::Value = report
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|arr| {
            let mut m = serde_json::Map::new();
            for t in arr {
                if let Some(name) = t.get("tool").and_then(|v| v.as_str()) {
                    m.insert(name.to_string(), serde_json::json!({
                        "success": t.get("success"),
                        "duration_ms": t.get("duration_ms"),
                    }));
                }
            }
            serde_json::Value::Object(m)
        })
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let record = serde_json::json!({
        "ts": ts,
        "run_id": report.get("run_id"),
        "passed": report.get("summary").and_then(|s| s.get("passed")),
        "failed": report.get("summary").and_then(|s| s.get("failed")),
        "tools": tools_summary,
    });

    let line = serde_json::to_string(&record).unwrap_or_default();
    if let Err(e) = std::fs::OpenOptions::new()
        .create(true).append(true).open(&path)
        .and_then(|mut f| { use std::io::Write; writeln!(f, "{}", line) })
    {
        eprintln!("history record: write failed: {}", e);
        return 1;
    }

    eprintln!("history: recorded run to {}", path);
    0
}

fn history_show(dir: &str, last: usize) -> i32 {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            println!("No history found in {}", dir);
            return 0;
        }
    };

    let mut lines: Vec<String> = Vec::new();
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "jsonl"))
        .collect();
    files.sort_by_key(|e| e.file_name());

    for entry in &files {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for line in content.lines() {
                lines.push(line.to_string());
            }
        }
    }

    let show: Vec<&String> = lines.iter().rev().take(last).collect();
    if show.is_empty() {
        println!("No history records found.");
        return 0;
    }

    println!("\n{:<20} {:>6} {:>6}  TOOLS", "TIMESTAMP", "PASS", "FAIL");
    println!("{}", "─".repeat(70));
    for raw in show.iter().rev() {
        if let Ok(rec) = serde_json::from_str::<serde_json::Value>(raw) {
            let ts = rec.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            let passed = rec.get("passed").and_then(|v| v.as_u64()).unwrap_or(0);
            let failed = rec.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
            let tools_str = rec.get("tools")
                .and_then(|v| v.as_object())
                .map(|m| m.iter()
                    .map(|(k, v)| {
                        let ok = v.get("success").and_then(|b| b.as_bool()).unwrap_or(false);
                        format!("{}:{}", k, if ok { "✓" } else { "✗" })
                    })
                    .collect::<Vec<_>>().join("  "))
                .unwrap_or_default();
            println!("{:<20} {:>6} {:>6}  {}", format_ts(ts), passed, failed, tools_str);
        }
    }
    println!();
    0
}

fn chrono_yymm(ts: u64) -> String {
    let secs = ts % (365 * 24 * 3600);
    let _ = secs;
    let d = std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts);
    if let Ok(dur) = d.duration_since(std::time::UNIX_EPOCH) {
        let days = dur.as_secs() / 86400;
        let year = 1970 + days / 365;
        let month = (days % 365) / 30 + 1;
        return format!("{}-{:02}", year, month);
    }
    "unknown".to_string()
}

fn format_ts(ts: u64) -> String {
    let days = ts / 86400;
    let year = 1970 + days / 365;
    let month = (days % 365) / 30 + 1;
    let day = (days % 365) % 30 + 1;
    let h = (ts % 86400) / 3600;
    let m = (ts % 3600) / 60;
    format!("{}-{:02}-{:02} {:02}:{:02}", year, month, day, h, m)
}

// ═══════════════════════════════════════════
// HOOKS
// ═══════════════════════════════════════════

fn install_hooks(repo: &str) -> i32 {
    let hook_dir = format!("{}/.git/hooks", repo);
    let hook_path = format!("{}/pre-commit", hook_dir);

    if !std::path::Path::new(&hook_dir).exists() {
        eprintln!("install-hooks: {} is not a git repository (no .git/hooks directory)", repo);
        return 1;
    }

    if std::path::Path::new(&hook_path).exists() {
        eprintln!("install-hooks: hook already exists at {} -- remove it first or use uninstall-hooks", hook_path);
        return 1;
    }

    let hook_script = r#"#!/usr/bin/env bash
# quality pre-commit hook -- installed by `quality install-hooks`
# Remove with: quality uninstall-hooks
set -euo pipefail

if command -v quality &>/dev/null; then
    quality run . --format table
elif [ -f target/release/quality ]; then
    ./target/release/quality run . --format table
else
    echo "quality: binary not found, skipping pre-commit check" >&2
    exit 0
fi
"#;

    match std::fs::write(&hook_path, hook_script) {
        Ok(_) => {}
        Err(e) => { eprintln!("install-hooks: write failed: {}", e); return 1; }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).ok();
    }

    println!("Installed pre-commit hook at {}", hook_path);
    println!("The hook will run `quality run .` before every commit.");
    println!("To bypass: git commit --no-verify");
    println!("To remove: quality uninstall-hooks {}", repo);
    0
}

fn uninstall_hooks(repo: &str) -> i32 {
    let hook_path = format!("{}/.git/hooks/pre-commit", repo);

    if !std::path::Path::new(&hook_path).exists() {
        eprintln!("uninstall-hooks: no pre-commit hook found at {}", hook_path);
        return 1;
    }

    let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
    if !content.contains("quality pre-commit hook") {
        eprintln!("uninstall-hooks: {} exists but was not installed by quality -- refusing to remove", hook_path);
        return 1;
    }

    match std::fs::remove_file(&hook_path) {
        Ok(_) => { println!("Removed pre-commit hook from {}", hook_path); 0 }
        Err(e) => { eprintln!("uninstall-hooks: remove failed: {}", e); 1 }
    }
}

// ═══════════════════════════════════════════
// WATCH MODE
// ═══════════════════════════════════════════

fn watch_mode(path: &str, checks: &str, debounce_ms: u64) -> i32 {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let check_list: Vec<String> = checks.split(',')
        .map(|s| s.trim().to_lowercase())
        .collect();

    println!("quality watch: watching {} for .rs changes", path);
    println!("  checks: {}", check_list.join(", "));
    println!("  debounce: {}ms", debounce_ms);
    println!("  Press Ctrl+C to stop.\n");

    let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();
    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => { eprintln!("watch: failed to create watcher: {}", e); return 1; }
    };

    if let Err(e) = watcher.watch(std::path::Path::new(path), RecursiveMode::Recursive) {
        eprintln!("watch: failed to watch {}: {}", path, e);
        return 1;
    }

    let debounce = Duration::from_millis(debounce_ms);
    let mut last_run: Option<Instant> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                let is_rust = event.paths.iter().any(|p| {
                    p.extension().map_or(false, |e| e == "rs")
                });
                if !is_rust { continue; }

                let now = Instant::now();
                let should_run = last_run.map_or(true, |t| now.duration_since(t) >= debounce);
                if should_run {
                    last_run = Some(now);
                    let changed: Vec<_> = event.paths.iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    eprintln!("\n[watch] changed: {}", changed.join(", "));
                    run_watch_checks(path, &check_list);
                }
            }
            Ok(Err(e)) => { eprintln!("watch error: {}", e); }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    0
}

fn run_watch_checks(path: &str, check_list: &[String]) {
    let should = |name: &str| check_list.iter().any(|c| c == name);

    let mut results: Vec<(&str, bool, String)> = Vec::new();

    if should("debt") {
        let r = check_debt(path, true, 100);
        results.push(("debt", r.passed, r.message));
    }
    if should("doc") {
        let r = check_doc_coverage(path, true, 50.0);
        results.push(("doc", r.passed, r.message));
    }
    if should("crap") {
        let r = check_crap(path, true, &None, 30.0);
        results.push(("crap", r.passed, r.message));
    }
    if should("complexity") {
        let r = check_complexity(path, true, 10);
        results.push(("complexity", r.passed, r.message));
    }

    let all_passed = results.iter().all(|(_, p, _)| *p);
    let status = if all_passed { "✓ PASS" } else { "✗ FAIL" };

    let line: Vec<String> = results.iter()
        .map(|(name, passed, _)| format!("{}: {}", name, if *passed { "✓" } else { "✗" }))
        .collect();

    eprintln!("[watch] {}  {}", status, line.join("  "));
    for (name, passed, msg) in &results {
        if !passed {
            eprintln!("  [{}] {}", name, msg);
        }
    }
}
