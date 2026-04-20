use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use ast_parse::analyze_file;

#[derive(Parser)]
#[command(name = "riskmap", about = "Risk map -- cross-reference git churn with code complexity to find danger zones")]
struct Cli {
    /// Path to the repository root
    path: String,

    /// Git log time range (e.g., '3 months ago', '2024-01-01')
    #[arg(short, long, default_value = "6 months ago")]
    since: String,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Only show files above this risk score (0-100)
    #[arg(long, default_value = "0")]
    min_risk: u32,
}

#[derive(Debug, Clone, Serialize)]
struct FileRisk {
    file: String,
    /// Number of commits touching this file in the time range
    churn: u32,
    /// Total cyclomatic complexity across all functions
    complexity: u32,
    /// Number of functions
    function_count: u32,
    /// Risk score: churn * complexity (normalized 0-100)
    risk_score: u32,
    /// Risk category
    category: String,
    /// Most complex functions
    hot_functions: Vec<String>,
}

#[derive(Serialize)]
struct RiskReport {
    files: Vec<FileRisk>,
    summary: RiskSummary,
}

#[derive(Serialize)]
struct RiskSummary {
    total_files: usize,
    high_risk: usize,
    medium_risk: usize,
    low_risk: usize,
    danger_zone: Vec<String>, // files with both high churn AND high complexity
}

fn main() {
    let cli = Cli::parse();

    let repo_root = Path::new(&cli.path);

    // Step 1: Get git churn data
    let churn_data = get_git_churn(repo_root, &cli.since);
    if churn_data.is_empty() {
        eprintln!("No git churn data found. Is this a git repository?");
        std::process::exit(1);
    }

    // Step 2: Get complexity data for each file
    let mut file_risks = Vec::new();

    for (file_path, churn_count) in &churn_data {
        let full_path = repo_root.join(file_path);
        if !full_path.exists() || full_path.extension().map_or(true, |e| e != "rs") {
            continue;
        }

        let full_path_str = full_path.to_string_lossy().to_string();
        match analyze_file(&full_path_str) {
            Ok(analysis) => {
                let total_complexity: u32 = analysis.functions.iter().map(|f| f.complexity).sum();
                let function_count = analysis.functions.len() as u32;

                // Hot functions: top 3 most complex
                let mut funcs = analysis.functions.clone();
                funcs.sort_by(|a, b| b.complexity.cmp(&a.complexity));
                let hot_functions: Vec<String> = funcs
                    .iter()
                    .take(3)
                    .filter(|f| f.complexity > 3)
                    .map(|f| format!("{} (c:{})", f.name, f.complexity))
                    .collect();

                // Risk score: normalized churn * complexity
                // We use a simple formula: min(100, churn * complexity / 10)
                let raw_risk = (*churn_count as f64 * total_complexity as f64) / 10.0;
                let risk_score = (raw_risk as u32).min(100);

                let category = if risk_score >= 70 {
                    "DANGER".to_string()
                } else if risk_score >= 40 {
                    "HIGH".to_string()
                } else if risk_score >= 20 {
                    "MEDIUM".to_string()
                } else {
                    "LOW".to_string()
                };

                file_risks.push(FileRisk {
                    file: file_path.clone(),
                    churn: *churn_count,
                    complexity: total_complexity,
                    function_count,
                    risk_score,
                    category,
                    hot_functions,
                });
            }
            Err(_) => {
                // Non-Rust or parse error, just report churn
                file_risks.push(FileRisk {
                    file: file_path.clone(),
                    churn: *churn_count,
                    complexity: 0,
                    function_count: 0,
                    risk_score: (*churn_count).min(100) / 5,
                    category: "CHURN_ONLY".to_string(),
                    hot_functions: vec![],
                });
            }
        }
    }

    // Sort by risk score descending
    file_risks.sort_by(|a, b| b.risk_score.cmp(&a.risk_score));

    // Filter
    if cli.min_risk > 0 {
        file_risks.retain(|f| f.risk_score >= cli.min_risk);
    }

    match cli.format.as_str() {
        "json" => output_json(&file_risks),
        _ => output_table(&file_risks),
    }
}

/// Get git churn data: file -> number of commits
fn get_git_churn(repo_root: &Path, since: &str) -> HashMap<String, u32> {
    let output = Command::new("git")
        .args(["log", "--since", since, "--name-only", "--pretty=format:"])
        .current_dir(repo_root)
        .output();

    let mut churn: HashMap<String, u32> = HashMap::new();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let file = line.trim();
                if !file.is_empty() && !file.starts_with('.') {
                    *churn.entry(file.to_string()).or_insert(0) += 1;
                }
            }
        }
        _ => {}
    }

    churn
}

fn output_table(file_risks: &[FileRisk]) {
    if file_risks.is_empty() {
        println!("No risk data found.");
        return;
    }

    let high = file_risks.iter().filter(|f| f.category == "DANGER" || f.category == "HIGH").count();
    let medium = file_risks.iter().filter(|f| f.category == "MEDIUM").count();
    let low = file_risks.iter().filter(|f| f.category == "LOW").count();

    // Danger zone: high churn AND high complexity
    let danger_zone: Vec<&FileRisk> = file_risks
        .iter()
        .filter(|f| f.churn > 5 && f.complexity > 20)
        .collect();

    println!("RISK MAP: CHURN × COMPLEXITY");
    println!("{}", "─".repeat(95));
    println!(
        "\n{:<45} {:>6} {:>6} {:>5} {:<8} {}",
        "FILE", "CHURN", "COMP", "RISK", "STATUS", "HOT FUNCTIONS"
    );
    println!("{}", "─".repeat(95));

    for f in file_risks.iter().take(30) {
        let icon = match f.category.as_str() {
            "DANGER" => "🔴",
            "HIGH" => "🟠",
            "MEDIUM" => "🟡",
            "LOW" => "🟢",
            _ => "⚪",
        };

        let hot = if f.hot_functions.is_empty() {
            String::new()
        } else {
            f.hot_functions[0].clone()
        };

        println!(
            "{:<45} {:>6} {:>6} {:>5} {} {:<6} {}",
            truncate(&f.file, 43),
            f.churn,
            f.complexity,
            f.risk_score,
            icon,
            f.category,
            truncate(&hot, 35),
        );
    }

    println!("{}", "─".repeat(95));
    println!();
    println!("RISK SUMMARY");
    println!("  Files analyzed:    {}", file_risks.len());
    println!("  🔴 DANGER/HIGH:    {} (changing often AND complex)", high);
    println!("  🟡 MEDIUM:         {}", medium);
    println!("  🟢 LOW:            {}", low);

    if !danger_zone.is_empty() {
        println!();
        println!("  ⚠ DANGER ZONE (high churn + high complexity):");
        for f in danger_zone.iter().take(5) {
            println!("    {} (churn: {}, complexity: {})", f.file, f.churn, f.complexity);
            for hf in &f.hot_functions {
                println!("      └─ {}", hf);
            }
        }
        println!();
        println!("  These files are changing often AND are complex.");
        println!("  They're the most likely source of bugs. Consider refactoring.");
    } else {
        println!();
        println!("  ✓ No danger zone detected. Complex files aren't changing much.");
    }
}

fn output_json(file_risks: &[FileRisk]) {
    let high = file_risks.iter().filter(|f| f.category == "DANGER" || f.category == "HIGH").count();
    let medium = file_risks.iter().filter(|f| f.category == "MEDIUM").count();
    let low = file_risks.iter().filter(|f| f.category == "LOW").count();

    let danger_zone: Vec<String> = file_risks
        .iter()
        .filter(|f| f.churn > 5 && f.complexity > 20)
        .map(|f| f.file.clone())
        .collect();

    let report = RiskReport {
        files: file_risks.to_vec(),
        summary: RiskSummary {
            total_files: file_risks.len(),
            high_risk: high,
            medium_risk: medium,
            low_risk: low,
            danger_zone,
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
