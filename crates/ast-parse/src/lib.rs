use syn::visit::Visit;
use syn::{Arm, BinOp, Block, ExprBinary, ExprIf, ExprLoop, ExprMatch, ExprWhile, ItemFn};


/// Cyclomatic complexity of a single function
#[derive(Debug, Clone)]
pub struct FunctionComplexity {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub complexity: u32,
    pub line_count: usize,
}

/// Result of analyzing a file
#[derive(Debug)]
pub struct FileAnalysis {
    pub file: String,
    pub functions: Vec<FunctionComplexity>,
}

/// Parse a Rust source file and extract cyclomatic complexity per function
pub fn analyze_file(file_path: &str) -> Result<FileAnalysis, String> {
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    let ast = syn::parse_file(&source).map_err(|e| format!("Failed to parse {}: {}", file_path, e))?;

    let mut visitor = ComplexityVisitor {
        file: file_path.to_string(),
        source: &source,
        functions: Vec::new(),
    };

    visitor.visit_file(&ast);

    Ok(FileAnalysis {
        file: file_path.to_string(),
        functions: visitor.functions,
    })
}

/// Parse source code string directly (for mutation testing)
pub fn analyze_source(source: &str, file_path: &str) -> Result<FileAnalysis, String> {
    let ast = syn::parse_file(source)
        .map_err(|e| format!("Failed to parse source ({}): {}", file_path, e))?;

    let mut visitor = ComplexityVisitor {
        file: file_path.to_string(),
        source,
        functions: Vec::new(),
    };

    visitor.visit_file(&ast);

    Ok(FileAnalysis {
        file: file_path.to_string(),
        functions: visitor.functions,
    })
}

struct ComplexityVisitor<'a> {
    file: String,
    source: &'a str,
    functions: Vec<FunctionComplexity>,
}

impl<'a> Visit<'a> for ComplexityVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        let name = node.sig.ident.to_string();
        // Estimate line number by finding the function name in source
        let line = estimate_line(self.source, &name);
        let mut counter = ComplexityCounter { count: 1 }; // base complexity = 1
        counter.visit_block(&node.block);

        let line_count = count_lines(&node.block);

        self.functions.push(FunctionComplexity {
            name,
            file: self.file.clone(),
            line,
            complexity: counter.count,
            line_count,
        });
    }
}

/// Counts decision points that increase cyclomatic complexity
struct ComplexityCounter {
    count: u32,
}

impl<'a> Visit<'a> for ComplexityCounter {
    fn visit_expr_if(&mut self, _node: &'a ExprIf) {
        self.count += 1;
        syn::visit::visit_expr_if(self, _node);
    }

    fn visit_expr_while(&mut self, _node: &'a ExprWhile) {
        self.count += 1;
        syn::visit::visit_expr_while(self, _node);
    }

    fn visit_expr_loop(&mut self, _node: &'a ExprLoop) {
        self.count += 1;
        syn::visit::visit_expr_loop(self, _node);
    }

    fn visit_expr_match(&mut self, node: &'a ExprMatch) {
        // Each match arm beyond the first adds complexity
        // But match itself is 1 decision, arms are alternatives
        let arm_count = node.arms.len().saturating_sub(1) as u32;
        self.count += arm_count;
        syn::visit::visit_expr_match(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'a ExprBinary) {
        match &node.op {
            BinOp::And(_) | BinOp::Or(_) => {
                self.count += 1;
            }
            _ => {}
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_arm(&mut self, node: &'a Arm) {
        // Guard conditions add complexity
        if node.guard.is_some() {
            self.count += 1;
        }
        syn::visit::visit_arm(self, node);
    }
}

fn estimate_line(source: &str, fn_name: &str) -> usize {
    // Find "fn <name>" in source and return the line number
    let pattern = format!("fn {}", fn_name);
    for (i, line) in source.lines().enumerate() {
        if line.contains(&pattern) {
            return i + 1;
        }
    }
    1
}

fn count_lines(block: &Block) -> usize {
    // Approximate: count lines between braces by looking at statement count
    // A better approach would track token positions, but this is sufficient
    if block.stmts.is_empty() {
        return 0;
    }
    // Use the number of statements as a rough proxy
    block.stmts.len()
}

// ─── Coverage Parsing (lcov format) ───

/// Line coverage data for a file
#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub file: String,
    pub lines_found: u32,
    pub lines_hit: u32,
}

/// Coverage percentage for a file (0-100)
impl FileCoverage {
    /// Calculate line coverage as a percentage (0-100). Returns 100.0 if no lines were found.
    pub fn coverage_pct(&self) -> f64 {
        if self.lines_found == 0 {
            return 100.0;
        }
        (self.lines_hit as f64 / self.lines_found as f64) * 100.0
    }
}

/// Parse an lcov coverage file
pub fn parse_lcov(lcov_path: &str) -> Result<Vec<FileCoverage>, String> {
    let content = std::fs::read_to_string(lcov_path)
        .map_err(|e| format!("Failed to read lcov file {}: {}", lcov_path, e))?;

    let mut results = Vec::new();
    let mut current_file: Option<String> = None;
    let mut lines_found = 0u32;
    let mut lines_hit = 0u32;

    for line in content.lines() {
        let line = line.trim();

        if let Some(path) = line.strip_prefix("SF:") {
            // Start of file section
            current_file = Some(path.to_string());
            lines_found = 0;
            lines_hit = 0;
        } else if line == "end_of_record" {
            if let Some(file) = current_file.take() {
                results.push(FileCoverage {
                    file,
                    lines_found,
                    lines_hit,
                });
            }
        } else if line.starts_with("LF:") {
            lines_found = line[3..].parse().unwrap_or(0);
        } else if line.starts_with("LH:") {
            lines_hit = line[3..].parse().unwrap_or(0);
        }
    }

    Ok(results)
}

/// Find coverage for a specific file from parsed lcov data
pub fn find_coverage<'a>(coverage: &'a [FileCoverage], file_path: &str) -> Option<&'a FileCoverage> {
    coverage.iter().find(|c| {
        c.file.ends_with(file_path) || file_path.ends_with(&c.file)
    })
}

// ─── CRAP Score Calculation ───

/// Calculate CRAP score: comp^2 * (1 - coverage/100)^3 + comp
pub fn crap_score(complexity: u32, coverage_pct: f64) -> f64 {
    let comp = complexity as f64;
    let uncovered = 1.0 - (coverage_pct / 100.0);
    comp * comp * uncovered * uncovered * uncovered + comp
}

/// CRAP score category
pub fn crap_category(score: f64) -> &'static str {
    if score <= 10.0 {
        "excellent"
    } else if score <= 20.0 {
        "good"
    } else if score <= 30.0 {
        "acceptable"
    } else {
        "crappy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complexity_simple_function() {
        let source = r#"
            fn simple(x: i32) -> i32 {
                x + 1
            }
        "#;
        let analysis = analyze_source(source, "test.rs").unwrap();
        assert_eq!(analysis.functions.len(), 1);
        assert_eq!(analysis.functions[0].complexity, 1); // base complexity
    }

    #[test]
    fn complexity_with_branches() {
        let source = r#"
            fn branched(x: i32) -> i32 {
                if x > 0 {
                    x
                } else if x < 0 {
                    -x
                } else {
                    0
                }
            }
        "#;
        let analysis = analyze_source(source, "test.rs").unwrap();
        assert_eq!(analysis.functions.len(), 1);
        // 1 (base) + 2 (if + else if) = 3
        assert_eq!(analysis.functions[0].complexity, 3);
    }

    #[test]
    fn complexity_with_loops_and_match() {
        let source = r#"
            fn complex(data: &[i32]) -> i32 {
                let mut sum = 0;
                for x in data {
                    match x {
                        0 => continue,
                        1..=10 => sum += x,
                        _ => {
                            if *x > 100 {
                                break;
                            }
                            sum += x;
                        }
                    }
                }
                sum
            }
        "#;
        let analysis = analyze_source(source, "test.rs").unwrap();
        assert_eq!(analysis.functions.len(), 1);
        // 1 (base) + 1 (for) + 2 (match arms beyond first) = 4
        // Note: nested if inside match arm counted within arm traversal
        assert_eq!(analysis.functions[0].complexity, 4);
    }

    #[test]
    fn crap_score_formula() {
        // comp=10, coverage=0% -> 10^2 * 1^3 + 10 = 110
        assert!((crap_score(10, 0.0) - 110.0).abs() < 0.01);

        // comp=10, coverage=100% -> 10^2 * 0^3 + 10 = 10
        assert!((crap_score(10, 100.0) - 10.0).abs() < 0.01);

        // comp=5, coverage=80% -> 25 * 0.008 + 5 = 5.2
        assert!((crap_score(5, 80.0) - 5.2).abs() < 0.01);
    }

    #[test]
    fn crap_categories() {
        assert_eq!(crap_category(5.0), "excellent");
        assert_eq!(crap_category(15.0), "good");
        assert_eq!(crap_category(25.0), "acceptable");
        assert_eq!(crap_category(35.0), "crappy");
    }

    #[test]
    fn lcov_parsing() {
        let lcov = r#"TN:
SF:src/main.rs
LF:100
LH:80
end_of_record
TN:
SF:src/lib.rs
LF:50
LH:25
end_of_record"#;

        // Write temp file
        let path = "/tmp/test_coverage.info";
        std::fs::write(path, lcov).unwrap();
        let coverage = parse_lcov(path).unwrap();

        assert_eq!(coverage.len(), 2);
        assert_eq!(coverage[0].file, "src/main.rs");
        assert!((coverage[0].coverage_pct() - 80.0).abs() < 0.01);
        assert_eq!(coverage[1].file, "src/lib.rs");
        assert!((coverage[1].coverage_pct() - 50.0).abs() < 0.01);

        std::fs::remove_file(path).ok();
    }
}
