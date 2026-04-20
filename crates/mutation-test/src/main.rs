use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use ast_parse::analyze_source;
use quality_common::{Column, print_table_header, print_table_row, separator};

#[derive(Parser)]
#[command(name = "mutate", about = "Mutation testing — evaluate test suite quality by introducing deliberate code changes")]
struct Cli {
    /// Path to the crate root (directory with Cargo.toml)
    path: String,

    /// Only test specific files (comma-separated)
    #[arg(long)]
    files: Option<String>,

    /// Maximum mutants to generate per file
    #[arg(short = 'n', long, default_value = "50")]
    max_mutants: usize,

    /// Timeout per test run in seconds
    #[arg(short, long, default_value = "120")]
    timeout: u64,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Pass environment variable to cargo (KEY=VALUE)
    #[arg(long)]
    env: Vec<String>,
}

/// A single mutation applied to source code
#[derive(Debug, Clone)]
struct Mutant {
    id: usize,
    file: String,
    line: usize,
    description: String,
    original: String,
    mutated: String,
}

/// Result of testing a single mutant
#[derive(Debug, Clone, Serialize)]
struct MutantResult {
    id: usize,
    file: String,
    line: usize,
    description: String,
    status: String, // "killed", "survived", "timeout", "error"
    test_output: String,
}

#[derive(Serialize)]
struct MutationReport {
    results: Vec<MutantResult>,
    summary: MutationSummary,
}

#[derive(Serialize)]
struct MutationSummary {
    total_mutants: usize,
    killed: usize,
    survived: usize,
    timeout: usize,
    error: usize,
    mutation_score: f64,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let crate_root = Path::new(&cli.path);
    if !crate_root.join("Cargo.toml").exists() {
        return Err(format!("No Cargo.toml found at {}", cli.path));
    }

    verify_tests_pass(crate_root, cli.timeout)?;

    let source_files = find_source_files(crate_root, &cli.files);
    if source_files.is_empty() {
        return Err("No source files found to mutate.".to_string());
    }
    println!("Found {} source files to mutate.\n", source_files.len());

    let all_mutants = generate_all_mutants(&source_files, cli.max_mutants);
    if all_mutants.is_empty() || cli.max_mutants == 0 {
        println!("No mutants to test (--max-mutants 0).");
        return Ok(());
    }

    let results = test_all_mutants(&all_mutants, crate_root, cli.timeout);

    match cli.format.as_str() {
        "json" => output_json(&results),
        _ => output_table(&results),
    }

    Ok(())
}

fn verify_tests_pass(crate_root: &Path, timeout: u64) -> Result<(), String> {
    println!("Verifying original tests pass...");
    if !run_cargo_test(crate_root, timeout) {
        return Err("Tests fail on original code. Fix tests before mutating.".to_string());
    }
    println!("✓ Original tests pass.\n");
    Ok(())
}

fn generate_all_mutants(source_files: &[PathBuf], max_mutants: usize) -> Vec<Mutant> {
    let mut all_mutants = Vec::new();
    let mut mutant_id = 0;

    for file_path in source_files {
        let Ok(source) = std::fs::read_to_string(file_path) else {
            eprintln!("Warning: Could not read {}", file_path.display());
            continue;
        };
        let mutants = generate_mutants(&source, &file_path.to_string_lossy(), &mut mutant_id);
        all_mutants.extend(mutants);
    }

    if all_mutants.len() > max_mutants {
        println!("Generated {} mutants, limiting to {}.", all_mutants.len(), max_mutants);
        all_mutants.truncate(max_mutants);
    } else {
        println!("Generated {} mutants.", all_mutants.len());
    }

    all_mutants
}

fn test_all_mutants(all_mutants: &[Mutant], crate_root: &Path, timeout: u64) -> Vec<MutantResult> {
    let mut results = Vec::new();
    for (i, mutant) in all_mutants.iter().enumerate() {
        print!(
            "[{}/{}] Testing mutant {} ({}:{})... ",
            i + 1,
            all_mutants.len(),
            mutant.id,
            mutant.file,
            mutant.line
        );

        let result = test_mutant(mutant, crate_root, timeout);
        let status_str = match result.status.as_str() {
            "killed" => "✓ KILLED",
            "survived" => "✗ SURVIVED",
            "timeout" => "⏱ TIMEOUT",
            _ => "? ERROR",
        };
        println!("{}", status_str);
        results.push(result);
    }
    results
}

/// Generate all possible mutants for a source file
fn generate_mutants(source: &str, file_path: &str, next_id: &mut usize) -> Vec<Mutant> {
    let mut mutants = Vec::new();

    // Strategy 1: Binary operator swaps
    let operator_swaps = [
        ("+", "-"), ("-", "+"), ("*", "/"), ("/", "*"),
        ("==", "!="), ("!=", "=="), (">", "<"), ("<", ">"),
        (">=", "<="), ("<=", ">="), ("&&", "||"), ("||", "&&"),
    ];

    for (original_op, mutated_op) in &operator_swaps {
        for (line_num, line) in source.lines().enumerate() {
            if line.contains(original_op) && !line.trim_start().starts_with("//") {
                *next_id += 1;
                mutants.push(Mutant {
                    id: *next_id,
                    file: file_path.to_string(),
                    line: line_num + 1,
                    description: format!("Replace '{}' with '{}'", original_op, mutated_op),
                    original: line.to_string(),
                    mutated: line.replace(original_op, mutated_op),
                });
            }
        }
    }

    // Strategy 2: Boolean literal swaps
    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            if line.contains("true") {
                *next_id += 1;
                mutants.push(Mutant {
                    id: *next_id,
                    file: file_path.to_string(),
                    line: line_num + 1,
                    description: "Replace 'true' with 'false'".to_string(),
                    original: line.to_string(),
                    mutated: line.replace("true", "false"),
                });
            }
            if line.contains("false") {
                *next_id += 1;
                mutants.push(Mutant {
                    id: *next_id,
                    file: file_path.to_string(),
                    line: line_num + 1,
                    description: "Replace 'false' with 'true'".to_string(),
                    original: line.to_string(),
                    mutated: line.replace("false", "true"),
                });
            }
        }
    }

    // Strategy 3: Boundary value mutations (off-by-one)
    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            // Replace < with <=, > with >= (and vice versa)
            if line.contains(" < ") && !line.contains(" <= ") {
                *next_id += 1;
                mutants.push(Mutant {
                    id: *next_id,
                    file: file_path.to_string(),
                    line: line_num + 1,
                    description: "Replace '<' with '<=' (boundary)".to_string(),
                    original: line.to_string(),
                    mutated: line.replacen(" < ", " <= ", 1),
                });
            }
            if line.contains(" > ") && !line.contains(" >= ") {
                *next_id += 1;
                mutants.push(Mutant {
                    id: *next_id,
                    file: file_path.to_string(),
                    line: line_num + 1,
                    description: "Replace '>' with '>=' (boundary)".to_string(),
                    original: line.to_string(),
                    mutated: line.replacen(" > ", " >= ", 1),
                });
            }
        }
    }

    mutants
}

/// Test a single mutant: apply mutation, run tests, restore
fn test_mutant(mutant: &Mutant, crate_root: &Path, timeout: u64) -> MutantResult {
    let file_path = crate_root.join(&mutant.file);

    // Read original
    let original_source = match std::fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => {
            return MutantResult {
                id: mutant.id,
                file: mutant.file.clone(),
                line: mutant.line,
                description: mutant.description.clone(),
                status: "error".to_string(),
                test_output: format!("Could not read file: {}", e),
            };
        }
    };

    // Apply mutation (replace the specific line)
    let mutated_source = replace_line(&original_source, mutant.line, &mutant.mutated);

    // Write mutated source
    if std::fs::write(&file_path, &mutated_source).is_err() {
        return MutantResult {
            id: mutant.id,
            file: mutant.file.clone(),
            line: mutant.line,
            description: mutant.description.clone(),
            status: "error".to_string(),
            test_output: "Could not write mutated file".to_string(),
        };
    }

    // Run tests
    let output = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(crate_root)
        .output();

    // Restore original immediately
    let _ = std::fs::write(&file_path, &original_source);

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);

            if output.status.success() {
                // Tests still pass = mutant SURVIVED (bad)
                MutantResult {
                    id: mutant.id,
                    file: mutant.file.clone(),
                    line: mutant.line,
                    description: mutant.description.clone(),
                    status: "survived".to_string(),
                    test_output: combined,
                }
            } else {
                // Tests failed = mutant KILLED (good)
                MutantResult {
                    id: mutant.id,
                    file: mutant.file.clone(),
                    line: mutant.line,
                    description: mutant.description.clone(),
                    status: "killed".to_string(),
                    test_output: combined,
                }
            }
        }
        Err(e) => MutantResult {
            id: mutant.id,
            file: mutant.file.clone(),
            line: mutant.line,
            description: mutant.description.clone(),
            status: "error".to_string(),
            test_output: format!("Failed to run cargo test: {}", e),
        },
    }
}

/// Replace a specific line (1-indexed) in source
fn replace_line(source: &str, line_num: usize, new_content: &str) -> String {
    source
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i + 1 == line_num {
                new_content.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find source files to mutate
fn find_source_files(crate_root: &Path, filter: &Option<String>) -> Vec<PathBuf> {
    if let Some(files) = filter {
        return files
            .split(',')
            .map(|f| crate_root.join(f.trim()))
            .filter(|p| p.exists())
            .collect();
    }

    let src_dir = crate_root.join("src");
    let mut files = Vec::new();
    find_rs_files(&src_dir, &mut files);
    files.sort();
    files
}

fn find_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                files.push(path);
            } else if path.is_dir() {
                find_rs_files(&path, files);
            }
        }
    }
}

/// Run cargo test and return whether it passed
fn run_cargo_test(crate_root: &Path, timeout: u64) -> bool {
    Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(crate_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn output_table(results: &[MutantResult]) {
    let killed = results.iter().filter(|r| r.status == "killed").count();
    let survived = results.iter().filter(|r| r.status == "survived").count();
    let timeout = results.iter().filter(|r| r.status == "timeout").count();
    let error = results.iter().filter(|r| r.status == "error").count();
    let total = results.len();

    println!();
    println!("MUTATION TESTING RESULTS");
    println!("{}", separator(80));

    if survived > 0 {
        println!();
        println!("SURVIVED MUTANTS (tests didn't catch these changes):");

        let columns = [
            Column::left("ID", 6),
            Column::left("FILE", 40),
            Column::right("LINE", 5),
            Column::left("DESCRIPTION", 30),
        ];
        print_table_header(&columns);

        for r in results.iter().filter(|r| r.status == "survived") {
            let id_str = format!("[{}]", r.id);
            let line_str = r.line.to_string();
            print_table_row(&columns, &[&id_str, &r.file, &line_str, &r.description]);
        }
    }

    println!();
    println!("{}", separator(80));

    let score = if total > 0 { killed as f64 / total as f64 * 100.0 } else { 0.0 };
    let verdict = if score >= 90.0 {
        "Excellent -- strong test suite"
    } else if score >= 70.0 {
        "Good -- most mutations caught"
    } else if score >= 50.0 {
        "Weak -- many mutations survived"
    } else {
        "Poor -- test suite needs significant work"
    };

    let summary = vec![
        ("Total mutants:", total.to_string()),
        ("Killed:", format!("{} ({:.0}%)", killed, killed as f64 / total as f64 * 100.0)),
        ("Survived:", format!("{} ({:.0}%)", survived, survived as f64 / total as f64 * 100.0)),
        ("Mutation Score:", format!("{:.0}%", score)),
        ("Verdict:", verdict.to_string()),
    ];
    quality_common::print_summary(&summary);

    if timeout > 0 {
        println!("  Timeout:        {}", timeout);
    }
    if error > 0 {
        println!("  Error:          {}", error);
    }

    if survived > 0 {
        println!();
        println!("  {} mutant(s) survived. Your tests didn't detect these code changes.", survived);
        println!("    Consider adding tests for the affected functions.");
    }
}

fn output_json(results: &[MutantResult]) {
    let killed = results.iter().filter(|r| r.status == "killed").count();
    let survived = results.iter().filter(|r| r.status == "survived").count();
    let timeout = results.iter().filter(|r| r.status == "timeout").count();
    let error = results.iter().filter(|r| r.status == "error").count();
    let total = results.len();
    let score = if total > 0 { killed as f64 / total as f64 * 100.0 } else { 0.0 };

    let report = MutationReport {
        results: results.to_vec(),
        summary: MutationSummary {
            total_mutants: total,
            killed,
            survived,
            timeout,
            error,
            mutation_score: score,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
