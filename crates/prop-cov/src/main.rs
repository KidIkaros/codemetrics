use clap::Parser;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use quality_common::{Column, print_table_header, print_table_row, separator, truncate};

#[derive(Parser)]
#[command(name = "propcov", about = "Property-based testing coverage — scan for proptest/quickcheck macros and calculate coverage")]
struct Cli {
    /// Path to scan (file or directory)
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Only scan test files (files in tests/ or with #[test] attribute)
    #[arg(long, default_value = "false")]
    only_tests: bool,

    /// Minimum coverage percentage to report (0-100)
    #[arg(long, default_value = "0")]
    min_coverage: u32,
}

#[derive(Debug, Clone, Serialize)]
struct PropertyTest {
    name: String,
    file: String,
    line: usize,
    framework: String, // "proptest", "quickcheck", "custom"
    functions_tested: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FunctionCoverage {
    name: String,
    file: String,
    line: usize,
    has_property_test: bool,
    has_unit_test: bool,
    property_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PropCovReport {
    property_tests: Vec<PropertyTest>,
    function_coverage: Vec<FunctionCoverage>,
    summary: PropCovSummary,
}

#[derive(Debug, Clone, Serialize)]
struct PropCovSummary {
    total_functions: usize,
    with_property_tests: usize,
    with_unit_tests: usize,
    property_test_count: usize,
    unit_test_count: usize,
    coverage_percentage: f64,
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
        find_rs_files(target_path, cli.recursive, cli.only_tests)
    } else if target_path.is_file() && target_path.extension().map_or(false, |e| e == "rs") {
        vec![target_path.to_path_buf()]
    } else {
        return Err(format!("No Rust source files found at {}", cli.path));
    };

    if source_files.is_empty() {
        return Err("No .rs files found to analyze.".to_string());
    }

    // First pass: collect all property tests
    let mut all_property_tests: Vec<PropertyTest> = Vec::new();
    // Track unit tests to know total test count
    let mut total_unit_tests = 0usize;
    // Track which functions are covered by property tests
    let mut function_coverage: HashMap<String, FunctionCoverage> = HashMap::new();

    for file_path in &source_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let file_str = file_path.to_string_lossy().to_string();
        let (props, units, funcs) = analyze_file(&source, &file_str);
        all_property_tests.extend(props);
        total_unit_tests += units;

        for (name, cov) in funcs {
            function_coverage
                .entry(name.clone())
                .and_modify(|existing| {
                    existing.has_property_test = existing.has_property_test || cov.has_property_test;
                    existing.has_unit_test = existing.has_unit_test || cov.has_unit_test;
                    let props: Vec<String> = cov.property_tests.clone();
                    existing.property_tests.extend(props);
                })
                .or_insert(cov);
        }
    }

    let total_functions = function_coverage.len();
    let with_property_tests = function_coverage.values().filter(|f| f.has_property_test).count();
    let with_unit_tests = function_coverage.values().filter(|f| f.has_unit_test).count();

    let coverage_percentage = if total_functions > 0 {
        with_property_tests as f64 / total_functions as f64 * 100.0
    } else {
        0.0
    };

    let report = PropCovReport {
        property_tests: all_property_tests.clone(),
        function_coverage: function_coverage.values().cloned().collect(),
        summary: PropCovSummary {
            total_functions,
            with_property_tests,
            with_unit_tests,
            property_test_count: all_property_tests.len(),
            unit_test_count: total_unit_tests,
            coverage_percentage,
        },
    };

    match cli.format.as_str() {
        "json" => output_json(&report),
        _ => output_table(&report, cli.min_coverage),
    }

    Ok(())
}

fn analyze_file(
    source: &str,
    file: &str,
) -> (Vec<PropertyTest>, usize, HashMap<String, FunctionCoverage>) {
    let mut property_tests = Vec::new();
    let mut unit_tests = 0usize;
    let mut functions = HashMap::new();
    let mut line_num = 0;

    for line in source.lines() {
        line_num += 1;
        let trimmed = line.trim();

        // Count unit tests
        if trimmed.starts_with("#[test]") || trimmed.contains("# [ test ]") {
            unit_tests += 1;
        }

        // Detect proptest! macro blocks
        if trimmed.contains("proptest!") {
            let props = extract_proptest_names(line, line_num, file);
            property_tests.extend(props);
        }

        // Detect quickcheck! or #[quickcheck] attribute
        if trimmed.contains("quickcheck!") || trimmed.contains("# [ quickcheck ]") {
            let props = extract_quickcheck_names(line, line_num, file);
            property_tests.extend(props);
        }

        // Detect #[test] fn with prop_assert or strategy patterns
        if trimmed.starts_with("fn ") && trimmed.contains("prop") {
            let fn_name = extract_fn_name(trimmed);
            if let Some(name) = fn_name {
                let pt = PropertyTest {
                    name: name.clone(),
                    file: file.to_string(),
                    line: line_num,
                    framework: "proptest_inline".to_string(),
                    functions_tested: vec![name.clone()],
                };
                property_tests.push(pt);
            }
        }

        // Track function definitions for coverage mapping
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            if let Some(name) = extract_fn_name(trimmed) {
                functions.insert(
                    name.clone(),
                    FunctionCoverage {
                        name,
                        file: file.to_string(),
                        line: line_num,
                        has_property_test: false,
                        has_unit_test: unit_tests > 0 && line_num > 0, // simplistic heuristic
                        property_tests: vec![],
                    },
                );
            }
        }
    }

    // Map property tests to functions they test
    for pt in &mut property_tests {
        for func_name in &pt.functions_tested {
            if let Some(func) = functions.get_mut(func_name) {
                func.has_property_test = true;
                func.property_tests.push(pt.name.clone());
            }
        }
    }

    (property_tests, unit_tests, functions)
}

fn extract_proptest_names(line: &str, line_num: usize, file: &str) -> Vec<PropertyTest> {
    let mut tests = Vec::new();
    // Heuristic: extract function calls within proptest! blocks
    // proptest!(|(x in 0..10)| { my_function(x) });
    // Look for identifiers that look like function calls
    for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.len() > 3 && !is_keyword(word) {
            tests.push(PropertyTest {
                name: format!("proptest_{}", word),
                file: file.to_string(),
                line: line_num,
                framework: "proptest".to_string(),
                functions_tested: vec![word.to_string()],
            });
        }
    }
    tests
}

fn extract_quickcheck_names(line: &str, line_num: usize, file: &str) -> Vec<PropertyTest> {
    let mut tests = Vec::new();
    for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.len() > 3 && !is_keyword(word) {
            tests.push(PropertyTest {
                name: format!("quickcheck_{}", word),
                file: file.to_string(),
                line: line_num,
                framework: "quickcheck".to_string(),
                functions_tested: vec![word.to_string()],
            });
        }
    }
    tests
}

fn extract_fn_name(line: &str) -> Option<String> {
    let after_fn = line.find("fn ")?;
    let rest = &line[after_fn + 3..];
    let name_end = rest.find(|c: char| c == '(' || c.is_whitespace())?;
    let name = rest[..name_end].trim();
    if name.is_empty() || is_keyword(name) {
        None
    } else {
        Some(name.to_string())
    }
}

fn is_keyword(word: &str) -> bool {
    let keywords: HashSet<&str> = [
        "if", "else", "while", "for", "loop", "match", "fn", "let", "mut",
        "pub", "use", "mod", "struct", "enum", "impl", "trait", "const",
        "static", "type", "where", "return", "break", "continue", "move",
        "ref", "self", "Self", "super", "crate", "async", "await", "dyn",
        "as", "in", "true", "false", "none", "some", "ok", "err", "result",
        "option", "vec", "string", "str", "u8", "u16", "u32", "u64", "i32",
        "i64", "f32", "f64", "bool", "char", "box", "rc", "arc", "cell",
        "refcell", "mutex", "rwlock", "thread", "spawn", "join", "main",
    ].iter().cloned().collect();
    keywords.contains(word.to_lowercase().as_str())
}

fn find_rs_files(dir: &Path, recursive: bool, only_tests: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                if !only_tests || is_test_file(&path) {
                    files.push(path);
                }
            } else if recursive && path.is_dir() {
                // Skip target directory
                if path.file_name().map_or(true, |n| n != "target") {
                    files.extend(find_rs_files(&path, recursive, only_tests));
                }
            }
        }
    }
    files
}

fn is_test_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    path_str.contains("/tests/") ||
    path_str.contains("\\tests\\") ||
    path_str.ends_with("_test.rs") ||
    path_str.ends_with("_tests.rs")
}

fn output_table(report: &PropCovReport, min_coverage: u32) {
    println!("PROPERTY-BASED TESTING COVERAGE");
    println!("{}", separator(95));

    // Property tests section
    if !report.property_tests.is_empty() {
        println!();
        println!("PROPERTY TESTS FOUND:");
        let columns = [
            Column::left("NAME", 30),
            Column::left("FRAMEWORK", 12),
            Column::left("FILE", 30),
            Column::right("LINE", 5),
        ];
        print_table_header(&columns);
        for pt in report.property_tests.iter().take(20) {
            let line_str = pt.line.to_string();
            print_table_row(&columns, &[
                &truncate(&pt.name, 28),
                &pt.framework,
                &truncate(&pt.file, 28),
                &line_str,
            ]);
        }
        if report.property_tests.len() > 20 {
            println!("  ... and {} more", report.property_tests.len() - 20);
        }
    }

    // Functions needing coverage
    let uncovered: Vec<_> = report
        .function_coverage
        .iter()
        .filter(|f| !f.has_property_test && f.has_unit_test)
        .collect();

    if !uncovered.is_empty() {
        println!();
        println!("FUNCTIONS WITH UNIT TESTS BUT NO PROPERTY TESTS:");
        let columns = [
            Column::left("FUNCTION", 35),
            Column::left("FILE", 35),
            Column::right("LINE", 5),
        ];
        print_table_header(&columns);
        for f in uncovered.iter().take(15) {
            let line_str = f.line.to_string();
            print_table_row(&columns, &[
                &truncate(&f.name, 33),
                &truncate(&f.file, 33),
                &line_str,
            ]);
        }
        if uncovered.len() > 15 {
            println!("  ... and {} more", uncovered.len() - 15);
        }
    }

    println!("{}", separator(95));
    println!();
    println!("  SUMMARY");
    println!("    Total functions:          {}", report.summary.total_functions);
    println!("    With property tests:        {}", report.summary.with_property_tests);
    println!("    With unit tests only:       {}", report.summary.with_unit_tests - report.summary.with_property_tests);
    println!("    Property test count:        {}", report.summary.property_test_count);
    println!("    Unit test count:            {}", report.summary.unit_test_count);
    println!();
    println!("    Property coverage:          {:.1}%", report.summary.coverage_percentage);

    let status = if report.summary.coverage_percentage >= 50.0 {
        "Good property coverage"
    } else if report.summary.coverage_percentage >= 20.0 {
        "Moderate — consider adding proptest/quickcheck for edge cases"
    } else {
        "Low — significant gap in property-based testing"
    };
    println!("    Status:                     {}", status);

    if report.summary.coverage_percentage < min_coverage as f64 {
        println!();
        println!("  ⚠ Coverage below threshold of {}%", min_coverage);
    }
}

fn output_json(report: &PropCovReport) {
    println!("{}", serde_json::to_string_pretty(report).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_proptest() {
        let source = r#"
#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_add_commutative(a in 0..100i32, b in 0..100i32) {
            prop_assert_eq!(add(a, b), add(b, a));
        }
    }
}
"#;
        let (props, _units, _funcs) = analyze_file(source, "test.rs");
        assert!(!props.is_empty(), "Should detect proptest");
        assert!(props.iter().any(|p| p.framework == "proptest"));
    }

    #[test]
    fn test_no_property_tests() {
        let source = r#"
#[cfg(test)]
mod tests {
    #[test]
    fn test_simple() {
        assert_eq!(add(2, 2), 4);
    }
}
"#;
        let (props, units, _funcs) = analyze_file(source, "test.rs");
        assert!(props.is_empty(), "Should not detect property tests in simple unit test file");
        assert_eq!(units, 1, "Should count unit test");
    }

    #[test]
    fn test_quickcheck_detection() {
        let source = r#"
#[quickcheck]
fn prop_reverse_reverse(xs: Vec<u32>) -> bool {
    xs == reverse(reverse(xs))
}
"#;
        let (props, _units, _funcs) = analyze_file(source, "test.rs");
        assert!(!props.is_empty(), "Should detect quickcheck attribute");
        assert!(props.iter().any(|p| p.framework == "quickcheck"));
    }
}
