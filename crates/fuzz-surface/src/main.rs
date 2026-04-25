use clap::Parser;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use quality_common::{Column, print_table_header, print_table_row, separator, truncate};

#[derive(Parser)]
#[command(name = "fuzz", about = "Fuzzing surface analyzer — identify functions ideal for fuzz testing")]
struct Cli {
    /// Path to scan (file or directory)
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Only show functions with score >= this value
    #[arg(long, default_value = "0")]
    min_score: u32,

    /// Limit output to top N functions
    #[arg(long, default_value = "20")]
    top: usize,
}

#[derive(Debug, Clone, Serialize)]
struct FuzzableFunction {
    name: String,
    file: String,
    line: usize,
    params: Vec<String>,
    score: u32,
    is_public: bool,
    complexity: u32,
    has_harness: bool,
}

#[derive(Serialize)]
struct FuzzReport {
    functions: Vec<FuzzableFunction>,
    summary: FuzzSummary,
}

#[derive(Serialize)]
struct FuzzSummary {
    total_functions: usize,
    fuzzable_functions: usize,
    functions_with_harnesses: usize,
    avg_score: f64,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let target_path = Path::new(&cli.path);

    let source_files = if target_path.is_dir() {
        find_rs_files(target_path, cli.recursive)
    } else if target_path.is_file() && target_path.extension().map_or(false, |e| e == "rs") {
        vec![target_path.to_path_buf()]
    } else {
        return Err(format!("No Rust source files found at {}", cli.path));
    };

    if source_files.is_empty() {
        return Err("No .rs files found to analyze.".to_string());
    }

    // Check for existing fuzz harnesses
    let harnesses = find_fuzz_harnesses(target_path);

    let mut all_functions: Vec<FuzzableFunction> = Vec::new();

    for file_path in &source_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let functions = analyze_file(&source, file_path, &harnesses);
        all_functions.extend(functions);
    }

    // Filter by min score and sort by score descending
    all_functions.retain(|f| f.score >= cli.min_score);
    all_functions.sort_by(|a, b| b.score.cmp(&a.score));

    let display_count = cli.top.min(all_functions.len());
    let display = &all_functions[..display_count];

    match cli.format.as_str() {
        "json" => output_json(display, &all_functions),
        _ => output_table(display, &all_functions),
    }

    Ok(())
}

fn find_fuzz_harnesses(base: &Path) -> HashSet<String> {
    let fuzz_dir = base.join("fuzz");
    let mut harnesses = HashSet::new();

    if fuzz_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&fuzz_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        extract_harness_names(&content, &mut harnesses);
                    }
                }
            }
        }
    }

    harnesses
}

fn extract_harness_names(content: &str, harnesses: &mut HashSet<String>) {
    // Look for fuzz_target! macro invocations: fuzz_target!(|data: &[u8]| { ... })
    for line in content.lines() {
        if line.contains("fuzz_target!") {
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if !word.is_empty() && word != "fuzz_target" && word != "libfuzzer" {
                    harnesses.insert(word.to_string());
                }
            }
        }
    }
}

fn analyze_file(source: &str, file_path: &Path, harnesses: &HashSet<String>) -> Vec<FuzzableFunction> {
    let file_str = file_path.to_string_lossy().to_string();

    // Simple heuristic-based analysis (string-based, no full AST parse)
    let mut functions = Vec::new();
    let mut in_fn = false;
    let mut fn_sig = String::new();
    let mut fn_start_line = 0;
    let mut brace_depth = 0;
    let mut line_num = 0;

    for line in source.lines() {
        line_num += 1;
        let trimmed = line.trim();

        if in_fn {
            fn_sig.push(' ');
            fn_sig.push_str(trimmed);
            brace_depth += trimmed.matches('{').count();
            brace_depth -= trimmed.matches('}').count();

            if brace_depth == 0 && trimmed.contains('}') {
                // End of function - process the signature
                if let Some(f) = parse_fn_sig(&fn_sig, &file_str, fn_start_line, harnesses) {
                    functions.push(f);
                }
                in_fn = false;
                fn_sig.clear();
            }
        } else {
            // Check for function signature
            if (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") || trimmed.starts_with("pub async fn ") || trimmed.starts_with("async fn "))
                && trimmed.contains('(') {
                in_fn = true;
                fn_start_line = line_num;
                fn_sig = trimmed.to_string();
                brace_depth = trimmed.matches('{').count() - trimmed.matches('}').count();
                if brace_depth > 0 {
                    if let Some(f) = parse_fn_sig(&fn_sig, &file_str, fn_start_line, harnesses) {
                        functions.push(f);
                    }
                    in_fn = false;
                    fn_sig.clear();
                }
            }
        }
    }

    functions
}

fn parse_fn_sig(sig: &str, file: &str, line: usize, harnesses: &HashSet<String>) -> Option<FuzzableFunction> {
    // Extract function name
    let after_fn = if let Some(pos) = sig.find("fn ") {
        &sig[pos + 3..]
    } else {
        return None;
    };

    let name_end = after_fn.find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(after_fn.len());
    let name = after_fn[..name_end].trim().to_string();

    // Extract parameters
    let params_start = after_fn.find('(')?;
    let params_end = after_fn.rfind(')')?;
    let params_str = &after_fn[params_start + 1..params_end];

    let params: Vec<String> = if params_str.is_empty() {
        vec![]
    } else {
        params_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    // Check visibility
    let is_public = sig.trim_start().starts_with("pub ");

    // Calculate fuzzability score
    let mut score = 0u32;
    let mut fuzzable_params = Vec::new();

    for param in &params {
        let param_lower = param.to_lowercase();
        // Raw byte input is very fuzzable
        if param_lower.contains("&[u8]") || param_lower.contains("bytes") {
            score += 30;
            fuzzable_params.push(param.clone());
        }
        // String inputs are good fuzz targets
        else if param_lower.contains("string") || param_lower.contains("&str") {
            score += 20;
            fuzzable_params.push(param.clone());
        }
        // Vec<u8> is also fuzzable
        else if param_lower.contains("vec<u8>") {
            score += 25;
            fuzzable_params.push(param.clone());
        }
        // Path/IO types can be fuzz targets
        else if param_lower.contains("path") || param_lower.contains("reader") || param_lower.contains("stream") {
            score += 10;
            fuzzable_params.push(param.clone());
        }
    }

    // No fuzzable params = not worth fuzzing
    if score == 0 {
        return None;
    }

    // Public functions are more valuable targets (higher impact)
    if is_public {
        score += 10;
    }

    // More parameters = more combinations to explore
    score += params.len() as u32 * 2;

    // Functions with more complexity are more likely to have bugs
    let complexity = estimate_complexity(sig);
    if complexity > 5 {
        score += 5;
    }

    let has_harness = harnesses.contains(&name);
    if has_harness {
        // Already has a harness, reduce score (not a gap)
        score = score.saturating_sub(5);
    }

    Some(FuzzableFunction {
        name,
        file: file.to_string(),
        line,
        params: fuzzable_params,
        score,
        is_public,
        complexity,
        has_harness,
    })
}

fn estimate_complexity(sig: &str) -> u32 {
    // Simple heuristic: count control flow keywords in the signature
    // For actual complexity we'd need to parse the body
    let mut complexity = 1;
    if sig.contains("if ") { complexity += 1; }
    if sig.contains("match ") { complexity += 1; }
    if sig.contains("for ") { complexity += 1; }
    if sig.contains("while ") { complexity += 1; }
    complexity
}

fn find_rs_files(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                files.push(path);
            } else if recursive && path.is_dir() {
                files.extend(find_rs_files(&path, recursive));
            }
        }
    }
    files
}

fn output_table(display: &[FuzzableFunction], all: &[FuzzableFunction]) {
    println!("FUZZING SURFACE ANALYSIS");
    println!("{}", separator(95));

    let columns = [
        Column::left("FUNCTION", 30),
        Column::left("FILE", 25),
        Column::right("LINE", 5),
        Column::right("SCORE", 6),
        Column::left("PARAMS", 20),
    ];
    print_table_header(&columns);

    for f in display {
        let params_str = f.params.join(", ");
        let harness_icon = if f.has_harness { "✓" } else { "·" };
        let pub_icon = if f.is_public { "[pub]" } else { "[priv]" };
        let name_with_icons = format!("{} {} {}", harness_icon, pub_icon, f.name);
        let line_str = f.line.to_string();
        let score_str = f.score.to_string();
        let file_short = truncate(&f.file, 24);

        print_table_row(&columns, &[
            &name_with_icons,
            &file_short,
            &line_str,
            &score_str,
            &truncate(&params_str, 19),
        ]);
    }

    println!("{}", separator(95));

    let fuzzable_count = all.len();
    let with_harnesses = all.iter().filter(|f| f.has_harness).count();
    let avg_score = if fuzzable_count > 0 {
        all.iter().map(|f| f.score).sum::<u32>() as f64 / fuzzable_count as f64
    } else {
        0.0
    };

    println!();
    println!("  Total functions analyzed: {}", all.len());
    println!("  Fuzzable functions:     {}", fuzzable_count);
    println!("  With harnesses:           {}", with_harnesses);
    println!("  Without harnesses:        {}", fuzzable_count - with_harnesses);
    println!("  Avg fuzzability score:    {:.1}", avg_score);

    if fuzzable_count > with_harnesses {
        println!();
        println!("  {} function(s) could benefit from fuzzing harnesses.", fuzzable_count - with_harnesses);
    }
}

fn output_json(display: &[FuzzableFunction], all: &[FuzzableFunction]) {
    let fuzzable_count = all.len();
    let with_harnesses = all.iter().filter(|f| f.has_harness).count();
    let avg_score = if fuzzable_count > 0 {
        all.iter().map(|f| f.score).sum::<u32>() as f64 / fuzzable_count as f64
    } else {
        0.0
    };

    let report = FuzzReport {
        functions: display.to_vec(),
        summary: FuzzSummary {
            total_functions: all.len(),
            fuzzable_functions: fuzzable_count,
            functions_with_harnesses: with_harnesses,
            avg_score,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fn_sig_fuzzable() {
        let harnesses = HashSet::new();
        let f = parse_fn_sig(
            "pub fn parse_data(data: &[u8]) -> Result<String, Error> { }",
            "test.rs",
            1,
            &harnesses,
        ).unwrap();
        assert_eq!(f.name, "parse_data");
        assert!(f.is_public);
        assert_eq!(f.score, 42); // 30 for &[u8] + 10 for pub + 1 param*2
        assert!(f.params.iter().any(|p| p.contains("u8")));
    }

    #[test]
    fn test_parse_fn_sig_not_fuzzable() {
        let harnesses = HashSet::new();
        let f = parse_fn_sig(
            "fn internal_helper(x: i32) -> i32 { }",
            "test.rs",
            1,
            &harnesses,
        );
        assert!(f.is_none(), "No fuzzable params should return None");
    }

    #[test]
    fn test_parse_fn_sig_string() {
        let harnesses = HashSet::new();
        let f = parse_fn_sig(
            "pub fn process_name(name: String) -> bool { }",
            "test.rs",
            1,
            &harnesses,
        ).unwrap();
        assert_eq!(f.score, 32); // 20 for String + 10 for pub + 1 param*2
        assert!(f.params.iter().any(|p| p.contains("String")));
    }

    #[test]
    fn test_harness_detection() {
        let mut harnesses = HashSet::new();
        harnesses.insert("parse_data".to_string());
        let f = parse_fn_sig(
            "pub fn parse_data(data: &[u8]) -> Result<String, Error> { }",
            "test.rs",
            1,
            &harnesses,
        ).unwrap();
        assert!(f.has_harness);
        assert_eq!(f.score, 37); // 30 + 10 + 1 param*2 - 5 for having harness
    }
}
