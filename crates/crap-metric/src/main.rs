#![deny(clippy::all)]

use clap::Parser;
use serde::Serialize;

use ast_parse_ts::parse_complexity_file;
use quality_common::{crap_category, crap_score, parse_lcov, CoverageRecord};
use quality_common::{find_source_files, print_table_header, print_table_row, Column};

#[derive(Parser)]
#[command(
    name = "crap",
    about = "CRAP metric calculator — maintenance risk scoring"
)]
struct Cli {
    /// Path to analyze (file or directory)
    path: String,

    /// Path to lcov coverage file
    #[arg(short, long)]
    coverage: Option<String>,

    /// Coverage percentage override (0-100) if no lcov file
    #[arg(short = 'p', long)]
    coverage_pct: Option<f64>,

    /// Output format: table (default), json, or ndjson
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Only show functions with CRAP score above this threshold
    #[arg(short, long, default_value = "0")]
    min_score: f64,

    /// Recursively find .rs files in directory
    #[arg(short, long)]
    recursive: bool,
}

#[derive(Serialize)]
struct CrapReport {
    functions: Vec<FunctionReport>,
    summary: Summary,
}

#[derive(Serialize, Clone)]
struct FunctionReport {
    name: String,
    file: String,
    line: usize,
    complexity: u32,
    line_count: usize,
    coverage_pct: f64,
    crap_score: f64,
    category: String,
    severity: String,
    help: String,
    rule_id: String,
}

#[derive(Serialize)]
struct Summary {
    total_functions: usize,
    total_complexity: u32,
    avg_complexity: f64,
    avg_crap: f64,
    crappy_count: usize,
    acceptable_count: usize,
    good_count: usize,
    excellent_count: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Find all source files
    let files = find_source_files(
        &cli.path,
        cli.recursive,
        &[
            "rs", "py", "js", "ts", "go", "c", "cpp", "cs", "java", "php", "rb", "swift",
        ],
    );
    if files.is_empty() {
        eprintln!("No source files found at {}", cli.path);
        std::process::exit(1);
    }

    // Load coverage if provided
    let coverage_data = cli.coverage.as_ref().and_then(|path| {
        std::fs::read_to_string(path)
            .ok()
            .map(|content| parse_lcov(&content))
    });

    // Analyze all files (skip CLI entry points and output formatting — not unit-testable)
    let mut all_functions = Vec::new();
    for file in &files {
        let functions = parse_complexity_file(file);
        let skip: &[&str] = &[
            "main",
            "output_table",
            "output_json",
            "output_ndjson",
            "run_self_test",
        ];
        let testable: Vec<_> = functions
            .into_iter()
            .filter(|f| !skip.contains(&f.name.as_str()))
            .collect();
        if testable.is_empty() {
            eprintln!("Warning: No functions found in {}", file);
        } else {
            all_functions.extend(testable);
        }
    }

    // Calculate CRAP scores
    let reports: Vec<FunctionReport> = all_functions
        .into_iter()
        .map(|func| {
            let line_count = func.end_line.saturating_sub(func.line);
            let coverage_pct = if let Some(ref cov_data) = coverage_data {
                let func_cov = function_coverage(cov_data, &func.name);
                if func_cov > 0.0 {
                    func_cov
                } else {
                    cli.coverage_pct.unwrap_or(0.0)
                }
            } else {
                cli.coverage_pct.unwrap_or(0.0)
            };

            let score = crap_score(func.complexity, coverage_pct);
            let category = crap_category(score).to_string();

            let (severity, rule_id, help) = match category.as_str() {
                "excellent" | "good" => (
                    "info".to_string(),
                    "crap-pass".to_string(),
                    "Function has acceptable CRAP score.".to_string(),
                ),
                "acceptable" => (
                    "warning".to_string(),
                    "crap-warning".to_string(),
                    "Function has moderate CRAP score. Consider refactoring or adding tests."
                        .to_string(),
                ),
                _ => (
                    "error".to_string(),
                    "crap-error".to_string(),
                    "Function has high CRAP score. Reduce complexity or increase test coverage."
                        .to_string(),
                ),
            };

            FunctionReport {
                name: func.name,
                file: func.file,
                line: func.line,
                complexity: func.complexity,
                line_count,
                coverage_pct,
                crap_score: score,
                category,
                severity,
                help,
                rule_id,
            }
        })
        .filter(|r| r.crap_score >= cli.min_score)
        .collect();

    // Sort by CRAP score descending
    let mut sorted_reports = reports;
    sorted_reports.sort_by(|a, b| b.crap_score.partial_cmp(&a.crap_score).unwrap());

    match cli.format.as_str() {
        "json" => output_json(&sorted_reports),
        "ndjson" => output_ndjson(&sorted_reports),
        _ => {
            output_table(&sorted_reports);
            Ok(())
        }
    }
}

fn output_table(reports: &[FunctionReport]) {
    if reports.is_empty() {
        println!("No functions found.");
        return;
    }

    let columns = [
        Column::left("FUNCTION", 30),
        Column::left("FILE", 40),
        Column::right("LINE", 4),
        Column::right("COMP", 4),
        Column::right("LINES", 10),
        Column::right("CRAP", 10),
        Column::left("CATEGORY", 15),
    ];
    print_table_header(&columns);

    let mut excellent = 0;
    let mut good = 0;
    let mut acceptable = 0;
    let mut crappy = 0;

    for r in reports {
        let cat_icon = match r.category.as_str() {
            "excellent" => {
                excellent += 1;
                "✓"
            }
            "good" => {
                good += 1;
                "○"
            }
            "acceptable" => {
                acceptable += 1;
                "△"
            }
            "crappy" => {
                crappy += 1;
                "✗"
            }
            _ => "?",
        };

        let line_str = r.line.to_string();
        let comp_str = r.complexity.to_string();
        let lc_str = r.line_count.to_string();
        let score_str = format!("{:.1}", r.crap_score);
        let cat_str = format!("{} {}", cat_icon, r.category);
        print_table_row(
            &columns,
            &[
                &r.name, &r.file, &line_str, &comp_str, &lc_str, &score_str, &cat_str,
            ],
        );
    }

    // Summary
    let total = reports.len();
    let total_comp: u32 = reports.iter().map(|r| r.complexity).sum();
    let total_crap: f64 = reports.iter().map(|r| r.crap_score).sum();

    let summary = vec![
        ("Functions analyzed:", total.to_string()),
        ("Total complexity:", total_comp.to_string()),
        (
            "Avg complexity:",
            format!("{:.1}", total_comp as f64 / total as f64),
        ),
        (
            "Avg CRAP score:",
            format!("{:.1}", total_crap / total as f64),
        ),
    ];
    quality_common::print_summary(&summary);

    println!();
    println!(
        "  {} excellent (≤10)  {} good (≤20)  {} acceptable (≤30)  {} crappy (>30)",
        excellent, good, acceptable, crappy
    );

    if crappy > 0 {
        println!();
        println!(
            "  ⚠ {} function(s) with CRAP > 30 need refactoring or more tests.",
            crappy
        );
    }
}

fn function_coverage(coverage_records: &[CoverageRecord], func_name: &str) -> f64 {
    coverage_records
        .iter()
        .find(|r| r.function == func_name)
        .map_or(0.0, |r| if r.hits > 0 { 1.0 } else { 0.0 })
}

fn output_json(reports: &[FunctionReport]) -> Result<(), Box<dyn std::error::Error>> {
    let total = reports.len();
    let total_complexity: u32 = reports.iter().map(|r| r.complexity).sum();
    let total_crap: f64 = reports.iter().map(|r| r.crap_score).sum();

    let mut excellent = 0;
    let mut good = 0;
    let mut acceptable = 0;
    let mut crappy = 0;
    for r in reports {
        match r.category.as_str() {
            "excellent" => excellent += 1,
            "good" => good += 1,
            "acceptable" => acceptable += 1,
            "crappy" => crappy += 1,
            _ => {}
        }
    }

    let report = CrapReport {
        functions: reports.to_vec(),
        summary: Summary {
            total_functions: total,
            total_complexity,
            avg_complexity: if total > 0 {
                total_complexity as f64 / total as f64
            } else {
                0.0
            },
            avg_crap: if total > 0 {
                total_crap / total as f64
            } else {
                0.0
            },
            crappy_count: crappy,
            acceptable_count: acceptable,
            good_count: good,
            excellent_count: excellent,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn output_ndjson(reports: &[FunctionReport]) -> Result<(), Box<dyn std::error::Error>> {
    for report in reports {
        println!(
            "{}",
            serde_json::json!({
                "tool": "crap",
                "rule_id": report.rule_id,
                "severity": report.severity,
                "message": format!("Function {} has CRAP score {:.1}", report.name, report.crap_score),
                "help": report.help,
                "file": report.file,
                "line": report.line,
                "col": null,
                "details": {
                    "name": report.name,
                    "complexity": report.complexity,
                    "coverage_pct": report.coverage_pct,
                    "crap_score": report.crap_score,
                    "category": report.category,
                }
            })
        );
    }
    Ok(())
}
