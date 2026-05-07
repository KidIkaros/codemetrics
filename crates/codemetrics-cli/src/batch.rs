// ═══════════════════════════════════════════
// BATCH EXECUTION — run_tool + run_batch
// ═══════════════════════════════════════════

use std::time::Instant;
use serde_json;
use colored::Colorize;

use codemetrics_common::*;
use codemetrics_common::memory::MemoryMonitor;

use crate::progress::{format_ms, Bar};
use crate::types::CheckReport;

pub fn run_tool(crate_name: &str, bin_name: &str, args: &[&str], tool_start: Instant) -> ToolResult {
    use std::process::{Command, Stdio};

    let output = Command::new(bin_name)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match output {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => o,
        _ => {
            let cargo_output = Command::new("cargo")
                .args(["run", "--quiet", "-p", crate_name, "--bin", bin_name, "--"])
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            match cargo_output {
                Ok(o) => o,
                Err(e) => {
                    let msg = if e.kind() == std::io::ErrorKind::NotFound {
                        format!(
                            "Binary '{}' not found. Install with: cargo install --path crates/{} (error: {})",
                            bin_name, crate_name, e
                        )
                    } else {
                        format!("Failed to run '{}': {}", bin_name, e)
                    };
                    return ToolResult {
                        tool: bin_name.to_string(),
                        success: false,
                        duration_ms: tool_start.elapsed().as_millis() as u64,
                        data: serde_json::Value::Null,
                        error: Some(msg),
                        suggested_fix: None,
                        auto_fix_available: None,
                    };
                }
            }
        }
    };

    let duration_ms = tool_start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let (data, error) = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => (json, None),
        Err(_) => {
            let trimmed = stdout.trim();
            if trimmed.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                (
                    serde_json::Value::Null,
                    Some(format!("No output. stderr: {}", stderr.trim())),
                )
            } else {
                (serde_json::json!({ "raw": trimmed }), None)
            }
        }
    };

    ToolResult {
        tool: bin_name.to_string(),
        success: error.is_none() && output.status.success(),
        duration_ms,
        data,
        error,
        suggested_fix: None,
        auto_fix_available: None,
    }
}

pub fn run_batch(
    path: &str,
    _config: &str,
    format: &str,
    baseline: Option<&str>,
    no_fail_on_regression: bool,
) -> i32 {
    let start = Instant::now();

    let mut memory_monitor = MemoryMonitor::from_env();
    let mem_limit_mb = memory_monitor.max_rss_bytes / 1024 / 1024;
    let mem_display = if mem_limit_mb >= 1024 {
        format!("{:.1} GB", mem_limit_mb as f64 / 1024.0)
    } else {
        format!("{} MB", mem_limit_mb)
    };
    eprintln!(
        "  {} CodeMetrics batch  ·  path: {}  ·  memory limit: {}",
        "▶".cyan().bold(),
        path.cyan(),
        mem_display.bright_black()
    );

    let tools: Vec<(&str, &str, Vec<&str>)> = vec![
        ("debt-scan", "debt", vec!["--recursive", path, "--format", "json"]),
        ("doc-coverage", "doccov", vec!["--recursive", path, "--format", "json"]),
        ("crap-metric", "crap", vec!["--recursive", path, "--format", "json"]),
        ("coupling", "coupling", vec![path, "--format", "json"]),
        ("risk-map", "riskmap", vec![path, "--format", "json"]),
        ("duplication", "dupfind", vec!["--recursive", path, "--format", "json"]),
        ("prop-cov", "propcov", vec!["--recursive", path, "--format", "json"]),
        ("taint-scan", "taint", vec!["--recursive", path, "--format", "json"]),
        ("fuzz-surface", "fuzz", vec!["--recursive", path, "--format", "json"]),
        (
            "mutation-test",
            "mutate",
            vec![
                path, "-p", "ast-parse-ts", "--max-mutants", "5", "--timeout", "30",
                "--format", "json",
            ],
        ),
        ("line-length", "linelen", vec!["--recursive", path, "--format", "json"]),
        ("halstead", "halstead", vec!["--recursive", path, "--format", "json"]),
        ("secrets", "secrets", vec!["--recursive", path, "--format", "json"]),
        ("dead-code", "deadcode", vec!["--recursive", path, "--format", "json"]),
        ("cohesion", "cohesion", vec!["--recursive", path, "--format", "json"]),
        ("comment-ratio", "comments", vec!["--recursive", path, "--format", "json"]),
        ("error-handling", "errhandle", vec!["--recursive", path, "--format", "json"]),
        ("type-coverage", "typecov", vec!["--recursive", path, "--format", "json"]),
        ("vuln-scan", "vulnscan", vec![path, "--format", "json"]),
        ("sast", "sast", vec!["--recursive", path, "--format", "json"]),
        ("crypto-check", "cryptocheck", vec!["--recursive", path, "--format", "json"]),
        ("licenses", "licenses", vec![path, "--format", "json"]),
    ];

    let mut results: Vec<ToolResult> = Vec::new();
    let mut bar = Bar::new(tools.len());
    for (crate_name, bin_name, args) in &tools {
        bar.set_current(bin_name);

        if let Err(usage) = memory_monitor.check() {
            bar.finish();
            eprintln!(
                "  {} Memory limit exceeded before running {} ({} MB used). Stopping batch.",
                "✗".red().bold(),
                bin_name,
                usage.rss_bytes / 1024 / 1024
            );
            break;
        }

        let tool_start = Instant::now();
        let result = run_tool(crate_name, bin_name, args, tool_start);
        let duration_ms = result.duration_ms;
        let success = result.success;
        results.push(result);
        bar.advance(bin_name, success, duration_ms);

        if let Err(usage) = memory_monitor.check() {
            bar.finish();
            eprintln!(
                "  {} Memory limit exceeded after {} ({} MB used). Stopping batch.",
                "✗".red().bold(),
                bin_name,
                usage.rss_bytes / 1024 / 1024
            );
            break;
        }
    }
    bar.finish();

    let duration_ms = start.elapsed().as_millis() as u64;
    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;

    let mut regression_detected = false;
    if let Some(baseline_file) = baseline {
        if let Ok(baseline_content) = std::fs::read_to_string(baseline_file) {
            if let Ok(baseline_report) = serde_json::from_str::<UnifiedReport>(&baseline_content) {
                let baseline_tools: std::collections::HashSet<String> = baseline_report
                    .tools
                    .iter()
                    .filter(|t| t.success)
                    .map(|t| t.tool.clone())
                    .collect();
                let current_tools: std::collections::HashSet<String> = results
                    .iter()
                    .filter(|t| t.success)
                    .map(|t| t.tool.clone())
                    .collect();
                let regressed: Vec<String> =
                    baseline_tools.difference(&current_tools).cloned().collect();
                if !regressed.is_empty() {
                    eprintln!(
                        "BASELINE REGRESSION: previously-passing tools now failing: {:?}",
                        regressed
                    );
                    if !no_fail_on_regression {
                        regression_detected = true;
                    }
                }
            }
        }
    }

    match format {
        "sarif" => {
            let mut log = SarifLog::new("codemetrics", env!("CARGO_PKG_VERSION"));
            let mut sarif_results: Vec<SarifResult> = Vec::new();
            for tool in &results {
                if !tool.success {
                    sarif_results.push(SarifResult {
                        rule_id: format!("{}-error", tool.tool),
                        rule_index: None,
                        level: "error".to_string(),
                        message: SarifMessage {
                            text: tool
                                .error
                                .clone()
                                .unwrap_or_else(|| format!("{} failed", tool.tool)),
                        },
                        locations: vec![SarifLocation {
                            physical_location: SarifPhysicalLocation {
                                artifact_location: Some(SarifArtifactLocation {
                                    uri: path.to_string(),
                                }),
                                region: None,
                            },
                        }],
                    });
                }
            }
            let run = sarif_run(
                "codemetrics-batch",
                env!("CARGO_PKG_VERSION"),
                sarif_results,
                if failed > 0 { 1 } else { 0 },
            );
            log.add_run(run);
            println!(
                "{}",
                serde_json::to_string_pretty(&log).expect("Failed to serialize log to JSON")
            );
        }
        "json" => {
            let report = new_unified_report(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("System time is before UNIX epoch")
                    .as_secs()
                    .to_string(),
            );
            let all_exts = [
                "rs", "py", "pyi", "js", "mjs", "cjs", "ts", "tsx", "mts", "go", "c", "h", "cpp",
                "cc", "cxx", "hpp", "cs", "java", "php", "rb", "swift",
            ];
            let mut langs_detected: Vec<String> = find_source_files(path, true, &all_exts)
                .iter()
                .map(|f| ast_parse_ts::Language::from_extension(f).to_string())
                .filter(|l| l != "unknown")
                .collect::<std::collections::HashSet<String>>()
                .into_iter()
                .collect();
            langs_detected.sort();
            let report = UnifiedReport {
                run_id: report.run_id,
                started_at: report.started_at,
                duration_ms,
                tools: results,
                summary: ReportSummary {
                    total_tools: tools.len(),
                    passed,
                    failed,
                    languages_detected: langs_detected,
                },
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("Failed to serialize report to JSON")
            );
        }
        _ => {
            let all_ok = failed == 0;
            let summary_str = format!(
                "{}/{} tools passed  ·  {}",
                passed,
                results.len(),
                format_ms(duration_ms)
            );
            let summary_col = if all_ok {
                summary_str.green().to_string()
            } else {
                summary_str.red().to_string()
            };
            let inner = 46usize;
            let border = "═".repeat(inner + 2);
            eprintln!();
            eprintln!("  ╔{}╗", border);
            let title = format!(
                "CODEMETRICS RUN  ·  {}",
                if all_ok {
                    "PASSED ✓".green().bold().to_string()
                } else {
                    "FAILED ✗".red().bold().to_string()
                }
            );
            crate::health::box_row(&title, inner);
            eprintln!("  ╠{}╣", border);
            crate::health::box_row(&summary_col, inner);
            crate::health::box_row(&format!("Path: {}", path), inner);
            eprintln!("  ╚{}╝", border);
            if !all_ok {
                eprintln!();
                for tool in results.iter().filter(|t| !t.success) {
                    let err = tool.error.as_deref().unwrap_or("check output for details");
                    eprintln!("  {} {}: {}", "✗".red(), tool.tool.red().bold(), err);
                }
            }
            eprintln!();
        }
    }

    if failed > 0 || regression_detected {
        1
    } else {
        0
    }
}
