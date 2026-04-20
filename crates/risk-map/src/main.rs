use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use ast_parse::analyze_file;
use quality_common::{get_git_churn, Column, print_table_header, print_table_row, separator};

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
    if let Err(e) = run(cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let repo_root = Path::new(&cli.path);

    let churn_data = get_git_churn(repo_root, &cli.since);
    if churn_data.is_empty() {
        return Err("No git churn data found. Is this a git repository?".to_string());
    }

    let mut file_risks = build_file_risks(repo_root, &churn_data);
    file_risks.sort_by(|a, b| b.risk_score.cmp(&a.risk_score));

    if cli.min_risk > 0 {
        file_risks.retain(|f| f.risk_score >= cli.min_risk);
    }

    match cli.format.as_str() {
        "json" => output_json(&file_risks),
        _ => output_table(&file_risks),
    }

    Ok(())
}

fn build_file_risks(repo_root: &Path, churn_data: &HashMap<String, u32>) -> Vec<FileRisk> {
    let mut file_risks = Vec::new();
    for (file_path, churn_count) in churn_data {
        let full_path = repo_root.join(file_path);
        if !full_path.exists() || full_path.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        let full_path_str = full_path.to_string_lossy().to_string();
        match analyze_file(&full_path_str) {
            Ok(analysis) => {
                file_risks.push(build_risk_from_analysis(file_path, *churn_count, &analysis));
            }
            Err(_) => {
                file_risks.push(build_churn_only_risk(file_path, *churn_count));
            }
        }
    }
    file_risks
}

fn build_risk_from_analysis(file_path: &str, churn: u32, analysis: &ast_parse::FileAnalysis) -> FileRisk {
    let total_complexity: u32 = analysis.functions.iter().map(|f| f.complexity).sum();
    let function_count = analysis.functions.len() as u32;

    let mut funcs = analysis.functions.clone();
    funcs.sort_by(|a, b| b.complexity.cmp(&a.complexity));
    let hot_functions: Vec<String> = funcs
        .iter()
        .take(3)
        .filter(|f| f.complexity > 3)
        .map(|f| format!("{} (c:{})", f.name, f.complexity))
        .collect();

    let raw_risk = (churn as f64 * total_complexity as f64) / 10.0;
    let risk_score = (raw_risk as u32).min(100);

    let category = risk_category(risk_score);

    FileRisk {
        file: file_path.to_string(),
        churn,
        complexity: total_complexity,
        function_count,
        risk_score,
        category,
        hot_functions,
    }
}

fn build_churn_only_risk(file_path: &str, churn: u32) -> FileRisk {
    FileRisk {
        file: file_path.to_string(),
        churn,
        complexity: 0,
        function_count: 0,
        risk_score: churn.min(100) / 5,
        category: "CHURN_ONLY".to_string(),
        hot_functions: vec![],
    }
}

fn risk_category(risk_score: u32) -> String {
    if risk_score >= 70 {
        "DANGER".to_string()
    } else if risk_score >= 40 {
        "HIGH".to_string()
    } else if risk_score >= 20 {
        "MEDIUM".to_string()
    } else {
        "LOW".to_string()
    }
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

    println!("RISK MAP: CHURN x COMPLEXITY");
    println!("{}", separator(95));

    let columns = [
        Column::left("FILE", 45),
        Column::right("CHURN", 6),
        Column::right("COMP", 6),
        Column::right("RISK", 5),
        Column::left("STATUS", 8),
        Column::left("HOT FUNCTIONS", 25),
    ];
    print_table_header(&columns);

    for f in file_risks.iter().take(30) {
        let icon = match f.category.as_str() {
            "DANGER" => "*",
            "HIGH" => "!",
            "MEDIUM" => "~",
            "LOW" => ".",
            _ => "?",
        };

        let hot = if f.hot_functions.is_empty() {
            String::new()
        } else {
            f.hot_functions[0].clone()
        };

        let churn_str = f.churn.to_string();
        let comp_str = f.complexity.to_string();
        let risk_str = f.risk_score.to_string();
        let status_str = format!("{} {}", icon, f.category);
        print_table_row(&columns, &[
            &f.file,
            &churn_str,
            &comp_str,
            &risk_str,
            &status_str,
            &hot,
        ]);
    }

    println!("{}", separator(95));
    println!();
    println!("RISK SUMMARY");
    println!("  Files analyzed:    {}", file_risks.len());
    println!("  ! DANGER/HIGH:     {} (changing often AND complex)", high);
    println!("  ~ MEDIUM:          {}", medium);
    println!("  . LOW:             {}", low);

    if !danger_zone.is_empty() {
        println!();
        println!("  DANGER ZONE (high churn + high complexity):");
        for f in danger_zone.iter().take(5) {
            println!("    {} (churn: {}, complexity: {})", f.file, f.churn, f.complexity);
            for hf in &f.hot_functions {
                println!("      |- {}", hf);
            }
        }
        println!();
        println!("  These files are changing often AND are complex.");
        println!("  They're the most likely source of bugs. Consider refactoring.");
    } else {
        println!();
        println!("  No danger zone detected. Complex files aren't changing much.");
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

