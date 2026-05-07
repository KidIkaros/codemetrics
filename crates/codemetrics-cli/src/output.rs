// ═══════════════════════════════════════════
// OUTPUT FORMATTERS
// ═══════════════════════════════════════════

use serde_json;

use crate::types::{CheckReport, ToolInfo};

pub fn output_json(report: &CheckReport) {
    println!(
        "{}",
        serde_json::to_string_pretty(report).expect("Failed to serialize report to JSON")
    );
}

pub fn output_ndjson(report: &CheckReport) {
    for check in &report.checks {
        let severity = check.severity.as_deref().unwrap_or("warning");
        let rule_id = check.rule_id.as_deref().unwrap_or(&check.name);
        let help = check.help.as_deref().unwrap_or("");
        if !check.passed {
            let items = check
                .details
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                println!(
                    "{}",
                    serde_json::json!({
                        "tool": check.name,
                        "severity": severity,
                        "rule_id": rule_id,
                        "message": check.message,
                        "help": help,
                        "file": report.path,
                        "line": null,
                        "col": null,
                    })
                );
            } else {
                for item in &items {
                    println!(
                        "{}",
                        serde_json::json!({
                            "tool": check.name,
                            "severity": severity,
                            "rule_id": rule_id,
                            "message": item.get("type").and_then(|v| v.as_str()).unwrap_or(&check.name),
                            "help": help,
                            "file": item.get("file"),
                            "line": item.get("line"),
                            "col": null,
                        })
                    );
                }
            }
        }
    }
}

pub fn discover_command(format: &str) {
    let tools = vec![
        ToolInfo {
            name: "crap".to_string(),
            binary: "crap".to_string(),
            description: "CRAP score calculator (maintenance risk)".to_string(),
            supported_formats: vec![
                "json".to_string(),
                "text".to_string(),
                "sarif".to_string(),
                "ndjson".to_string(),
            ],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["crap-error".to_string(), "crap-warning".to_string()],
        },
        ToolInfo {
            name: "debt".to_string(),
            binary: "debt".to_string(),
            description: "Technical debt scanner (TODO/FIXME/HACK)".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "type".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec![
                "debt-todo".to_string(),
                "debt-fixme".to_string(),
                "debt-hack".to_string(),
                "debt-xxx".to_string(),
                "debt-bug".to_string(),
            ],
        },
        ToolInfo {
            name: "doccov".to_string(),
            binary: "doccov".to_string(),
            description: "Documentation coverage for public APIs".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["doccov-missing-doc".to_string()],
        },
        ToolInfo {
            name: "dupfind".to_string(),
            binary: "dupfind".to_string(),
            description: "Code duplication detection".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["dupfind-duplicate".to_string()],
        },
        ToolInfo {
            name: "coupling".to_string(),
            binary: "coupling".to_string(),
            description: "Module dependency analysis".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["coupling-high".to_string()],
        },
        ToolInfo {
            name: "riskmap".to_string(),
            binary: "riskmap".to_string(),
            description: "Risk map (churn × complexity)".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["riskmap-high-risk".to_string()],
        },
        ToolInfo {
            name: "mutate".to_string(),
            binary: "mutate".to_string(),
            description: "Mutation testing (Rust-only)".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["mutate-unmutated".to_string()],
        },
        ToolInfo {
            name: "fuzz".to_string(),
            binary: "fuzz".to_string(),
            description: "Fuzz surface analysis".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["fuzz-unsafe-surface".to_string()],
        },
        ToolInfo {
            name: "propcov".to_string(),
            binary: "propcov".to_string(),
            description: "Property test coverage".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["propcov-low-coverage".to_string()],
        },
        ToolInfo {
            name: "taint".to_string(),
            binary: "taint".to_string(),
            description: "Taint analysis (data flow)".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string(), "ndjson".to_string()],
            output_fields: vec![
                "rule_id".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "file".to_string(),
                "line".to_string(),
                "help".to_string(),
            ],
            rule_ids: vec!["taint-unsafe-flow".to_string()],
        },
        ToolInfo {
            name: "init".to_string(),
            binary: "codemetrics".to_string(),
            description: "Auto-detect project ecosystem and write .quality.toml. Use --ci for full GitHub Actions + pre-commit hook + baseline bootstrap.".to_string(),
            supported_formats: vec!["text".to_string()],
            output_fields: vec!["ecosystem".to_string(), "config_path".to_string()],
            rule_ids: vec![],
        },
        ToolInfo {
            name: "check".to_string(),
            binary: "codemetrics".to_string(),
            description: "Run all quality checks in one call. Auto-loads .quality.toml thresholds. Exit 0=pass, 1=fail, 2=error.".to_string(),
            supported_formats: vec!["json".to_string(), "text".to_string()],
            output_fields: vec![
                "passed".to_string(),
                "checks".to_string(),
                "score".to_string(),
                "threshold".to_string(),
                "message".to_string(),
            ],
            rule_ids: vec![],
        },
        ToolInfo {
            name: "watch".to_string(),
            binary: "codemetrics".to_string(),
            description: "Watch for file changes and re-run checks. Auto-detects test runner and coverage. Use --no-tests for metrics-only mode.".to_string(),
            supported_formats: vec!["text".to_string()],
            output_fields: vec![],
            rule_ids: vec![],
        },
        ToolInfo {
            name: "install-hooks".to_string(),
            binary: "codemetrics".to_string(),
            description: "Install a pre-commit git hook. Default: full hook (tests + coverage + check). Use --fast for lightweight metrics-only hook.".to_string(),
            supported_formats: vec!["text".to_string()],
            output_fields: vec![],
            rule_ids: vec![],
        },
    ];

    match format {
        "text" => {
            for tool in &tools {
                println!("{} ({})", tool.name, tool.binary);
                println!("  Description: {}", tool.description);
                println!("  Supported Formats: {}", tool.supported_formats.join(", "));
                println!("  Output Fields: {}", tool.output_fields.join(", "));
                println!("  Rule IDs: {}", tool.rule_ids.join(", "));
                println!();
            }
        }
        _ => {
            println!(
                "{}",
                serde_json::to_string_pretty(&tools).expect("Failed to serialize tools to JSON")
            );
        }
    }
}
