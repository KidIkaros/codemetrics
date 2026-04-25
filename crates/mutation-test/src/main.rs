use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use quality_common::{Column, print_table_header, print_table_row, separator, wrap_tool_response};

mod delta;

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

    /// Mutation strategies to use: all, standard, bitwise, arithmetic
    #[arg(long, default_value = "all")]
    strategy: String,

    /// Enable delta mutation testing: only mutate functions changed since base ref
    #[arg(long)]
    delta: bool,

    /// Git ref (branch, tag, or commit) to diff against for delta mode (default: HEAD~1)
    #[arg(long, default_value = "HEAD~1")]
    base_ref: String,
}

/// A single mutation applied to source code
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Mutant {
    id: usize,
    file: String,
    line: usize,
    description: String,
    original: String,
    mutated: String,
    category: String, // "standard", "bitwise", "arithmetic", "boundary"
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
    let start = std::time::Instant::now();
    let crate_root = Path::new(&cli.path);
    if !crate_root.join("Cargo.toml").exists() {
        return Err(format!("No Cargo.toml found at {}", cli.path));
    }

    verify_tests_pass(crate_root, cli.timeout)?;

    let source_files = find_source_files(crate_root, &cli.files);
    if source_files.is_empty() {
        return Err("No source files found to mutate.".to_string());
    }

    // Delta mutation testing: compute affected functions from git diff
    let delta_analysis = if cli.delta {
        println!("Computing delta mutation analysis against {}...", cli.base_ref);
        let loaded_files: Vec<(String, String)> = source_files.iter()
            .filter_map(|f| {
                let s = std::fs::read_to_string(f).ok()?;
                Some((f.to_string_lossy().to_string(), s))
            })
            .collect();

        let analysis = delta::run_delta_analysis(crate_root, &cli.base_ref, &loaded_files, source_files.len());

        let affected_count: usize = analysis.affected_functions.values().map(|v| v.len()).sum();
        let changed_fn_count: usize = analysis.changed_functions.values().map(|v| v.len()).sum();

        println!("  Changed files:    {}", analysis.changed_files.len());
        println!("  Changed functions: {}", changed_fn_count);
        println!("  Affected by calls: {}", affected_count - changed_fn_count);
        println!("  Reduction:        {:.1}% fewer mutants\n", analysis.reduction_pct);

        Some(analysis)
    } else {
        println!("Found {} source files to mutate.\n", source_files.len());
        None
    };

    // Process files one at a time to keep memory usage low
    let mut all_results: Vec<MutantResult> = Vec::new();
    let mut total_mutants = 0usize;
    let mut killed = 0usize;
    let mut survived = 0usize;
    let mut timeouts = 0usize;
    let mut errors = 0usize;

    for file_path in &source_files {
        if total_mutants >= cli.max_mutants {
            break;
        }

        let Ok(source) = std::fs::read_to_string(file_path) else {
            eprintln!("Warning: Could not read {}", file_path.display());
            continue;
        };

        let remaining = cli.max_mutants.saturating_sub(total_mutants);
        let mut file_mutants = generate_mutants_for_file(
            &source,
            &file_path.to_string_lossy(),
            &cli.strategy,
            remaining
        );

        // In delta mode, filter mutants to only those in affected functions
        if let Some(ref delta) = delta_analysis {
            let file_str = file_path.to_string_lossy().to_string();
            file_mutants.retain(|m| {
                delta::is_line_in_affected_function(
                    &file_str,
                    m.line,
                    &delta.affected_functions,
                    // Pass source for function range lookup
                    &[(file_str.clone(), source.clone())],
                )
            });
        }

        if file_mutants.is_empty() {
            continue;
        }

        // Assign global IDs
        for (idx, mutant) in file_mutants.iter_mut().enumerate() {
            mutant.id = total_mutants + idx + 1;
        }

        let file_count = file_mutants.len();
        println!("\nTesting {} mutants from {}...", file_count, file_path.display());

        // Test mutants for this file and immediately discard them
        for (i, mutant) in file_mutants.iter().enumerate() {
            print!(
                "  [{}/{}] mutant {} (line {})... ",
                i + 1,
                file_count,
                mutant.id,
                mutant.line
            );

            let result = test_mutant(mutant, crate_root, cli.timeout);
            match result.status.as_str() {
                "killed" => { print!("✓ KILLED\n"); killed += 1; }
                "survived" => { print!("✗ SURVIVED\n"); survived += 1; }
                "timeout" => { print!("⏱ TIMEOUT\n"); timeouts += 1; }
                _ => { print!("? ERROR\n"); errors += 1; }
            }
            all_results.push(result);
        }

        total_mutants += file_count;

        // Explicitly drop file data to free memory before next file
        drop(source);
        drop(file_mutants);
    }

    if total_mutants == 0 {
        println!("No mutants to test (--max-mutants 0 or no matching code).");
        return Ok(());
    }

    println!("\nTested {} mutants total.", total_mutants);

    match cli.format.as_str() {
        "json" => {
            let duration_ms = start.elapsed().as_millis() as u64;
            output_json_streaming(&all_results, total_mutants, killed, survived, timeouts, errors, duration_ms);
        }
        _ => output_table_streaming(&all_results, total_mutants, killed, survived, timeouts, errors),
    }

    Ok(())
}

/// ... (rest of the code remains the same)
fn verify_tests_pass(crate_root: &Path, timeout: u64) -> Result<(), String> {
    println!("Verifying original tests pass...");
    if !run_cargo_test(crate_root, timeout) {
        return Err("Tests fail on original code. Fix tests before mutating.".to_string());
    }
    println!("✓ Original tests pass.\n");
    Ok(())
}

/// Generate mutants for a single file with a limit to prevent memory blowup
fn generate_mutants_for_file(source: &str, file_path: &str, strategy: &str, limit: usize) -> Vec<Mutant> {
    generate_mutants(source, file_path, &mut 0, strategy, limit)
}

/// Generate all possible mutants for a source file
fn generate_mutants(source: &str, file_path: &str, next_id: &mut usize, strategy: &str, limit: usize) -> Vec<Mutant> {
    let mut mutants = Vec::with_capacity(limit.min(1000));
    let include_standard = strategy == "all" || strategy == "standard";
    let include_bitwise = strategy == "all" || strategy == "bitwise";
    let include_arithmetic = strategy == "all" || strategy == "arithmetic";
    let include_boundary = strategy == "all" || strategy == "boundary";

    macro_rules! push_if_limit {
        ($mutant:expr) => {
            if mutants.len() >= limit {
                return mutants;
            }
            mutants.push($mutant);
        };
    }

    // Strategy 1: Binary operator swaps (standard)
    if include_standard {
        let operator_swaps = [
            ("+", "-"), ("-", "+"), ("*", "/"), ("/", "*"),
            ("==", "!="), ("!=", "=="), (">", "<"), ("<", ">"),
            (">=", "<="), ("<=", ">="), ("&&", "||"), ("||", "&&"),
        ];

        for (original_op, mutated_op) in &operator_swaps {
            for (line_num, line) in source.lines().enumerate() {
                if line.contains(original_op) && !line.trim_start().starts_with("//") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: format!("Replace '{}' with '{}'", original_op, mutated_op),
                        original: line.to_string(),
                        mutated: line.replace(original_op, mutated_op),
                        category: "standard".to_string(),
                    });
                }
            }
        }

        // Boolean literal swaps (standard)
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//") {
                if line.contains("true") && !line.contains("// true") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace 'true' with 'false'".to_string(),
                        original: line.to_string(),
                        mutated: line.replace("true", "false"),
                        category: "standard".to_string(),
                    });
                }
                if line.contains("false") && !line.contains("// false") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace 'false' with 'true'".to_string(),
                        original: line.to_string(),
                        mutated: line.replace("false", "true"),
                        category: "standard".to_string(),
                    });
                }
            }
        }
    }

    // Strategy 2: Boundary value mutations
    if include_boundary {
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//") {
                if line.contains(" < ") && !line.contains(" <= ") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace '<' with '<=' (boundary)".to_string(),
                        original: line.to_string(),
                        mutated: line.replacen(" < ", " <= ", 1),
                        category: "boundary".to_string(),
                    });
                }
                if line.contains(" <= ") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace '<=' with '<' (boundary)".to_string(),
                        original: line.to_string(),
                        mutated: line.replacen(" <= ", " < ", 1),
                        category: "boundary".to_string(),
                    });
                }
                if line.contains(" >= ") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace '>=' with '>' (boundary)".to_string(),
                        original: line.to_string(),
                        mutated: line.replacen(" >= ", " > ", 1),
                        category: "boundary".to_string(),
                    });
                }
                if line.contains(" > ") && !line.contains(" >= ") {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: "Replace '>' with '>=' (boundary)".to_string(),
                        original: line.to_string(),
                        mutated: line.replacen(" > ", " >= ", 1),
                        category: "boundary".to_string(),
                    });
                }
            }
        }
    }

    // Strategy 3: Bitwise operator mutations
    if include_bitwise {
        let bitwise_swaps = [
            (" ^ ", " | "), (" | ", " ^ "),
            (" << ", " >> "), (" >> ", " << "),
            (" & ", " | "), (" | ", " & "),
        ];

        for (original_op, mutated_op) in &bitwise_swaps {
            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with("//") && line.contains(original_op) {
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: format!("Replace '{}' with '{}' (bitwise)", original_op.trim(), mutated_op.trim()),
                        original: line.to_string(),
                        mutated: line.replace(original_op, mutated_op),
                        category: "bitwise".to_string(),
                    });
                }
            }
        }
    }

    // Strategy 4: Arithmetic overflow mutations
    if include_arithmetic {
        let arithmetic_mutations = [
            ("wrapping_add", "+", "Replace wrapping_add with + (overflow check)"),
            ("wrapping_sub", "-", "Replace wrapping_sub with - (overflow check)"),
            ("wrapping_mul", "*", "Replace wrapping_mul with * (overflow check)"),
            ("saturating_add", "+", "Replace saturating_add with + (overflow check)"),
            ("saturating_sub", "-", "Replace saturating_sub with - (overflow check)"),
            ("saturating_mul", "*", "Replace saturating_mul with * (overflow check)"),
            ("checked_add", "+", "Replace checked_add with + (unwrap result)"),
            ("checked_sub", "-", "Replace checked_sub with - (unwrap result)"),
            ("checked_mul", "*", "Replace checked_mul with * (unwrap result)"),
        ];

        for (func_name, _operator, desc) in &arithmetic_mutations {
            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with("//") && line.contains(func_name) {
                    let mutated = line.replace(&format!(".{func_name}("), ".");
                    let mutated = mutated.replace(&format!("{func_name}("), "( ");
                    *next_id += 1;
                    push_if_limit!(Mutant {
                        id: *next_id,
                        file: file_path.to_string(),
                        line: line_num + 1,
                        description: desc.to_string(),
                        original: line.to_string(),
                        mutated,
                        category: "arithmetic".to_string(),
                    });
                }
            }
        }
    }

    mutants
}

/// Test a single mutant: apply mutation, run tests, restore
fn test_mutant(mutant: &Mutant, crate_root: &Path, _timeout: u64) -> MutantResult {
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
fn run_cargo_test(crate_root: &Path, _timeout: u64) -> bool {
    Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(crate_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

fn output_table_streaming(
    results: &[MutantResult],
    total: usize,
    killed: usize,
    survived: usize,
    timeouts: usize,
    errors: usize,
) {
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

    if timeouts > 0 {
        println!("  Timeout:        {}", timeouts);
    }
    if errors > 0 {
        println!("  Error:          {}", errors);
    }

    if survived > 0 {
        println!();
        println!("  {} mutant(s) survived. Your tests didn't detect these code changes.", survived);
        println!("    Consider adding tests for the affected functions.");
    }
}

fn output_json_streaming(
    results: &[MutantResult],
    total: usize,
    killed: usize,
    survived: usize,
    timeouts: usize,
    errors: usize,
    duration_ms: u64,
) {
    let score = if total > 0 { killed as f64 / total as f64 * 100.0 } else { 0.0 };

    let report = MutationReport {
        results: results.to_vec(),
        summary: MutationSummary {
            total_mutants: total,
            killed,
            survived,
            timeout: timeouts,
            error: errors,
            mutation_score: score,
        },
    };

    let response = wrap_tool_response(
        "mutate",
        env!("CARGO_PKG_VERSION"),
        true,
        duration_ms,
        serde_json::to_value(&report).unwrap(),
        Some(serde_json::json!({
            "total_mutants": total,
            "killed": killed,
            "survived": survived,
            "mutation_score": score,
            "passed": survived == 0 && errors == 0,
        })),
        None,
    );

    println!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bitwise_mutants() {
        let source = r#"
fn test() {
    let a = 1 ^ 2;
    let b = 3 << 4;
    let c = 5 & 6;
}
"#;
        let mut id = 0;
        let mutants = generate_mutants(source, "test.rs", &mut id, "bitwise", 1000);

        // Should find XOR, shift, and AND mutations
        assert!(!mutants.is_empty(), "Should generate bitwise mutants");
        assert!(mutants.iter().any(|m| m.description.contains("bitwise")));
        assert!(mutants.iter().all(|m| m.category == "bitwise"));
    }

    #[test]
    fn test_generate_arithmetic_mutants() {
        let source = r#"
fn test() {
    let a = 1u32.wrapping_add(2);
    let b = 3u32.saturating_sub(1);
}
"#;
        let mut id = 0;
        let mutants = generate_mutants(source, "test.rs", &mut id, "arithmetic", 1000);

        assert!(!mutants.is_empty(), "Should generate arithmetic mutants");
        assert!(mutants.iter().any(|m| m.description.contains("wrapping") || m.description.contains("saturating")));
        assert!(mutants.iter().all(|m| m.category == "arithmetic"));
    }

    #[test]
    fn test_strategy_filtering_standard() {
        let source = r#"
fn test() {
    let a = 1 + 2;
    let b = 3 ^ 4;
}
"#;
        let mut id = 0;
        let standard = generate_mutants(source, "test.rs", &mut id, "standard", 1000);
        assert!(standard.iter().all(|m| m.category == "standard"));
        assert!(!standard.iter().any(|m| m.category == "bitwise"));
    }

    #[test]
    fn test_strategy_filtering_bitwise() {
        let source = r#"
fn test() {
    let a = 1 + 2;
    let b = 3 ^ 4;
}
"#;
        let mut id = 0;
        let bitwise = generate_mutants(source, "test.rs", &mut id, "bitwise", 1000);
        assert!(bitwise.iter().all(|m| m.category == "bitwise"));
        assert!(!bitwise.iter().any(|m| m.category == "standard"));
    }

    #[test]
    fn test_mutant_has_category() {
        let mutants = vec![
            Mutant {
                id: 1,
                file: "test.rs".to_string(),
                line: 1,
                description: "test".to_string(),
                original: "a + b".to_string(),
                mutated: "a - b".to_string(),
                category: "standard".to_string(),
            }
        ];
        assert_eq!(mutants[0].category, "standard");
    }
}
