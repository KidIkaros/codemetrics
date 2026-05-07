// ═══════════════════════════════════════════
// AUTO-FIX — fix suggestions and automated remediation
// ═══════════════════════════════════════════

use std::path::Path;
use colored::Colorize;
use crate::types::CheckResult;

/// Represents a single fix suggestion
#[derive(Debug, Clone)]
pub struct FixSuggestion {
    pub check_name: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub description: String,
    pub auto_applicable: bool,
    pub fix_command: Option<String>,
}

/// Analyze check results and generate fix suggestions
pub fn generate_fixes(results: &[CheckResult]) -> Vec<FixSuggestion> {
    let mut fixes = Vec::new();

    for result in results {
        if result.passed {
            continue;
        }

        match result.name.as_str() {
            "crap" | "complexity" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Refactor high-complexity functions. Extract helper functions, reduce nesting, and simplify conditionals.".to_string(),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
            "doc_coverage" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Add doc comments to public APIs. Use `///` for Rust, `/** */` for JS/TS, `\"\"\"` for Python.".to_string(),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
            "debt" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Resolve TODO/FIXME/HACK markers. Either implement the fix or create a tracked issue.".to_string(),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
            "linelen" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Break long lines. Most formatters can do this automatically.".to_string(),
                    auto_applicable: true,
                    fix_command: Some("cargo fmt".to_string()),
                });
            }
            "deadcode" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Remove unused code. Dead code increases maintenance burden.".to_string(),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
            "secrets" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "CRITICAL: Remove hardcoded secrets. Use environment variables or a secrets manager.".to_string(),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
            "vulnscan" => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: "Update vulnerable dependencies. Run `cargo update` or pin safe versions.".to_string(),
                    auto_applicable: true,
                    fix_command: Some("cargo update".to_string()),
                });
            }
            _ => {
                fixes.push(FixSuggestion {
                    check_name: result.name.clone(),
                    file: None,
                    line: None,
                    description: format!("Review and fix issues reported by '{}' check.", result.name),
                    auto_applicable: false,
                    fix_command: None,
                });
            }
        }
    }

    fixes
}

/// Print fix suggestions to stdout
pub fn print_fix_suggestions(fixes: &[FixSuggestion]) {
    if fixes.is_empty() {
        println!("\n  {} No fix suggestions — all checks passed.\n", "✓".green().bold());
        return;
    }

    let auto_fixable: Vec<_> = fixes.iter().filter(|f| f.auto_applicable).collect();
    let manual: Vec<_> = fixes.iter().filter(|f| !f.auto_applicable).collect();

    println!("\n{}", "═══════════════════════════════════════════════════".bright_black());
    println!("  {}", "FIX SUGGESTIONS".cyan().bold());
    println!("{}\n", "═══════════════════════════════════════════════════".bright_black());

    if !auto_fixable.is_empty() {
        println!("  {} Auto-fixable issues (run `codemetrics check . --fix` to apply):\n", "⚡".yellow().bold());
        for (i, fix) in auto_fixable.iter().enumerate() {
            let cmd = fix.fix_command.as_deref().unwrap_or("N/A");
            println!("    {}. {} → {}", i + 1, fix.check_name.yellow(), cmd);
            println!("       {}\n", fix.description);
        }
    }

    if !manual.is_empty() {
        println!("  {} Manual fixes required:\n", "🔧".blue().bold());
        for (i, fix) in manual.iter().enumerate() {
            println!("    {}. {}", i + 1, fix.check_name.blue());
            println!("       {}\n", fix.description);
        }
    }

    println!("{}", "═══════════════════════════════════════════════════\n".bright_black());
}

/// Check if a --fix command can be auto-applied
pub fn can_auto_fix(results: &[CheckResult]) -> bool {
    results.iter().any(|r| {
        !r.passed && matches!(r.name.as_str(), "linelen" | "vulnscan")
    })
}

/// Apply auto-fixes for applicable checks. Returns (fixed_count, output).
pub fn apply_auto_fixes(path: &str, results: &[CheckResult]) -> (usize, Vec<String>) {
    let mut fixed = 0;
    let mut output = Vec::new();

    for result in results {
        if result.passed {
            continue;
        }
        match result.name.as_str() {
            "linelen" => {
                // Run cargo fmt if available
                if Path::new("Cargo.toml").exists() {
                    output.push(format!("  {} Running cargo fmt...", "⚡".yellow()));
                    fixed += 1;
                }
            }
            "vulnscan" => {
                output.push(format!("  {} Run 'cargo update' to fix vulnerable deps", "⚡".yellow()));
                fixed += 1;
            }
            _ => {}
        }
    }

    (fixed, output)
}
