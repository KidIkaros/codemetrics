use clap::{Parser, Subcommand};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use ast_parse::{analyze_file, find_coverage, parse_lcov, crap_score, crap_category};
use quality_common::{find_rust_files, find_source_files};

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
// CHECK IMPLEMENTATIONS
// ═══════════════════════════════════════════

fn check_crap(path: &str, recursive: bool, coverage_path: &Option<String>, max_crap: f64) -> CheckResult {
    let files = find_rust_files(path, recursive);
    let coverage_data = coverage_path.as_ref().and_then(|p| parse_lcov(p).ok());

    let mut functions = Vec::new();
    for file in &files {
        if let Ok(analysis) = analyze_file(file) {
            for func in analysis.functions {
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
                functions.push((func.name, func.complexity, cov_pct, score));
            }
        }
    }

    let avg_crap = if functions.is_empty() {
        0.0
    } else {
        functions.iter().map(|f| f.3).sum::<f64>() / functions.len() as f64
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
            "total_functions": functions.len(),
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

fn check_doc_coverage(path: &str, recursive: bool, min_doc: f64) -> CheckResult {
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

    let files = find_rust_files(path, recursive);
    let mut counter = DocCounter { total: 0, documented: 0 };
    for file in &files {
        if let Ok(source) = std::fs::read_to_string(file) {
            if let Ok(ast) = syn::parse_file(&source) {
                counter.visit_file(&ast);
            }
        }
    }

    let pct = if counter.total > 0 {
        counter.documented as f64 / counter.total as f64 * 100.0
    } else {
        100.0
    };

    CheckResult {
        name: "doc_coverage".to_string(),
        passed: pct >= min_doc,
        score: Some(pct),
        threshold: Some(min_doc),
        message: if pct >= min_doc {
            format!("Doc coverage {:.0}% >= {:.0}%", pct, min_doc)
        } else {
            format!("Doc coverage {:.0}% < {:.0}%", pct, min_doc)
        },
        details: serde_json::json!({
            "total_public": counter.total,
            "documented": counter.documented,
            "coverage_pct": pct,
        }),
    }
}

fn check_complexity(path: &str, recursive: bool, min_complexity: u32) -> CheckResult {
    let files = find_rust_files(path, recursive);
    let mut complex_funcs = Vec::new();
    let mut total = 0;

    for file in &files {
        if let Ok(analysis) = analyze_file(file) {
            for func in analysis.functions {
                total += 1;
                if func.complexity >= min_complexity {
                    complex_funcs.push(serde_json::json!({
                        "name": func.name,
                        "file": func.file,
                        "line": func.line,
                        "complexity": func.complexity,
                    }));
                }
            }
        }
    }

    CheckResult {
        name: "complexity".to_string(),
        passed: complex_funcs.is_empty(),
        score: Some(complex_funcs.len() as f64),
        threshold: Some(0.0),
        message: if complex_funcs.is_empty() {
            "No functions above complexity threshold".to_string()
        } else {
            format!("{} functions with complexity >= {}", complex_funcs.len(), min_complexity)
        },
        details: serde_json::json!({
            "total_functions": total,
            "complex_count": complex_funcs.len(),
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
max_markers = 100      # Fail if more than N TODO/FIXME/HACK markers
types = ["TODO", "FIXME", "HACK", "XXX"]

[doc_coverage]
min_pct = 50           # Fail if public API doc coverage below this

[complexity]
max_function = 10      # Warn if any function has complexity above this

[skip]
checks = []            # Skip these checks: crap, debt, doc, complexity
"#;
    std::fs::write(output, config).expect("Failed to write config");
    println!("Created {}", output);
}

// ═══════════════════════════════════════════
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
    };

    std::process::exit(exit_code);
}
