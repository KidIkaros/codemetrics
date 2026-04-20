use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ast_parse::{analyze_file, crap_category, crap_score, find_coverage, parse_lcov, FileCoverage, FunctionComplexity};

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
                find_coverage(cov_data, &func.file)
                    .map(|c| c.coverage_pct())
                    .or(cli.coverage_pct)
                    .unwrap_or(0.0)
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

fn find_rust_files(path: &str, recursive: bool) -> Vec<String> {
    let path = Path::new(path);
    let mut files = Vec::new();

    if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
        files.push(path.to_string_lossy().to_string());
    } else if path.is_dir() {
        find_rust_in_dir(path, recursive, &mut files);
    }

    files
}

fn find_rust_in_dir(dir: &Path, recursive: bool, files: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            files.push(path.to_string_lossy().to_string());
        } else if recursive && path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && name != ".git" && !name.starts_with('.') {
                find_rust_in_dir(&path, recursive, files);
            }
        }
    }
}

fn output_table(reports: &[FunctionReport]) {
    if reports.is_empty() {
        println!("No functions found.");
        return;
    }

    println!(
        "{:<30} {:<40} {:>4} {:>4} {:>10} {:>10} {:<12}",
        "FUNCTION", "FILE", "LINE", "COMP", "LINES", "CRAP", "CATEGORY"
    );
    println!("{}", "─".repeat(112));

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

        // Color the CRAP score
        let score_str = if r.crap_score > 30.0 {
            format!("{:.1}", r.crap_score)
        } else {
            format!("{:.1}", r.crap_score)
        };

        println!(
            "{:<30} {:<40} {:>4} {:>4} {:>10} {:>10} {} {:<10}",
            truncate(&r.name, 28),
            truncate(&r.file, 38),
            r.line,
            r.complexity,
            r.line_count,
            score_str,
            cat_icon,
            r.category,
        );
    }

    println!("{}", "─".repeat(112));

    let total = reports.len();
    let total_comp: u32 = reports.iter().map(|r| r.complexity).sum();
    let total_crap: f64 = reports.iter().map(|r| r.crap_score).sum();

    println!();
    println!("SUMMARY");
    println!("  Functions analyzed: {}", total);
    println!("  Total complexity:   {}", total_comp);
    println!("  Avg complexity:     {:.1}", total_comp as f64 / total as f64);
    println!("  Avg CRAP score:     {:.1}", total_crap / total as f64);
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

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - max + 1..])
    }
}
