use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ast_parse::{analyze_file, crap_category, crap_score, find_coverage, parse_lcov, FileCoverage, FunctionComplexity};
use quality_common::{find_rust_files, Column, print_table_header, print_table_row};

#[derive(Parser)]
#[command(name = "crap", about = "CRAP metric calculator — maintenance risk scoring")]
struct Cli {
    /// Path to analyze (file or directory)
    path: String,

    /// Path to lcov coverage file
    #[arg(short, long)]
    coverage: Option<String>,

    /// Coverage percentage override (0-100) if no lcov file
    #[arg(short = 'p', long)]
    coverage_pct: Option<f64>,

    /// Output format: table (default) or json
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

fn main() {
    let cli = Cli::parse();

    // Find all Rust files
    let files = find_rust_files(&cli.path, cli.recursive);
    if files.is_empty() {
        eprintln!("No .rs files found at {}", cli.path);
        std::process::exit(1);
    }

    // Load coverage if provided
    let coverage_data = cli.coverage.as_ref().and_then(|path| {
        parse_lcov(path).map_err(|e| {
            eprintln!("Warning: Failed to parse coverage: {}", e);
        }).ok()
    });

    // Analyze all files
    let mut all_functions = Vec::new();
    for file in &files {
        match analyze_file(file) {
            Ok(analysis) => all_functions.extend(analysis.functions),
            Err(e) => eprintln!("Warning: {}", e),
        }
    }

    // Calculate CRAP scores
    let reports: Vec<FunctionReport> = all_functions
        .into_iter()
        .map(|func| {
            let coverage_pct = if let Some(ref cov_data) = coverage_data {
                // Try per-function coverage first (from DA records)
                if let Some(cov) = find_coverage(cov_data, &func.file) {
                    let (_, _, func_cov) = cov.range_coverage(func.line, func.end_line);
                    if func_cov > 0.0 || !cov.da_records.is_empty() {
                        func_cov
                    } else {
                        // Fall back to file-level coverage
                        cov.coverage_pct()
                    }
                } else {
                    cli.coverage_pct.unwrap_or(0.0)
                }
            } else {
                cli.coverage_pct.unwrap_or(0.0)
            };

            let score = crap_score(func.complexity, coverage_pct);
            let category = crap_category(score).to_string();

            FunctionReport {
                name: func.name,
                file: func.file,
                line: func.line,
                complexity: func.complexity,
                line_count: func.line_count,
                coverage_pct,
                crap_score: score,
                category,
            }
        })
        .filter(|r| r.crap_score >= cli.min_score)
        .collect();

    // Sort by CRAP score descending
    let mut sorted_reports = reports;
    sorted_reports.sort_by(|a, b| b.crap_score.partial_cmp(&a.crap_score).unwrap());

    match cli.format.as_str() {
        "json" => output_json(&sorted_reports),
        _ => output_table(&sorted_reports),
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
        Column::left("CATEGORY", 12),
    ];
    print_table_header(&columns);

    let mut excellent = 0;
    let mut good = 0;
    let mut acceptable = 0;
    let mut crappy = 0;

    for r in reports {
        let cat_icon = match r.category.as_str() {
            "excellent" => { excellent += 1; "✓" }
            "good" => { good += 1; "○" }
            "acceptable" => { acceptable += 1; "△" }
            "crappy" => { crappy += 1; "✗" }
            _ => "?"
        };

        let line_str = r.line.to_string();
        let comp_str = r.complexity.to_string();
        let lc_str = r.line_count.to_string();
        let score_str = format!("{:.1}", r.crap_score);
        let cat_str = format!("{} {}", cat_icon, r.category);
        print_table_row(&columns, &[
            &r.name,
            &r.file,
            &line_str,
            &comp_str,
            &lc_str,
            &score_str,
            &cat_str,
        ]);
    }

    // Summary
    let total = reports.len();
    let total_comp: u32 = reports.iter().map(|r| r.complexity).sum();
    let total_crap: f64 = reports.iter().map(|r| r.crap_score).sum();

    let summary = vec![
        ("Functions analyzed:", total.to_string()),
        ("Total complexity:", total_comp.to_string()),
        ("Avg complexity:", format!("{:.1}", total_comp as f64 / total as f64)),
        ("Avg CRAP score:", format!("{:.1}", total_crap / total as f64)),
    ];
    quality_common::print_summary(&summary);

    println!();
    println!("  {} excellent (≤10)  {} good (≤20)  {} acceptable (≤30)  {} crappy (>30)",
        excellent, good, acceptable, crappy);

    if crappy > 0 {
        println!();
        println!("  ⚠ {} function(s) with CRAP > 30 need refactoring or more tests.", crappy);
    }
}

fn output_json(reports: &[FunctionReport]) {
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
            avg_complexity: if total > 0 { total_complexity as f64 / total as f64 } else { 0.0 },
            avg_crap: if total > 0 { total_crap / total as f64 } else { 0.0 },
            crappy_count: crappy,
            acceptable_count: acceptable,
            good_count: good,
            excellent_count: excellent,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

