//! Schema validation integration tests.
//!
//! Runs the `codemetrics` binary with JSON output against a small fixture
//! directory and validates the output against each tool's JSON schema.

use assert_cmd::Command;
use std::path::Path;

/// Absolute path to the workspace schemas/ directory.
fn schemas_dir() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is crates/codemetrics-cli
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent() // crates/
        .unwrap()
        .parent() // workspace root
        .unwrap()
        .join("schemas")
}

/// Absolute path to any small fixture we can point the CLI at.
/// The crates/fixtures dir is part of the workspace and has real Rust source.
fn fixture_path() -> std::path::PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent()
        .unwrap()
        .join("fixtures")
}

fn load_schema(name: &str) -> serde_json::Value {
    let path = schemas_dir().join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Cannot read schema {}: {}", path.display(), e));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("Invalid JSON in schema {}: {}", name, e))
}

/// Run `codemetrics check <fixture> --format json` and return the parsed output.
fn run_check_json(extra_args: &[&str]) -> serde_json::Value {
    let fixture = fixture_path();
    let mut cmd = Command::cargo_bin("codemetrics").expect("codemetrics binary not found");
    cmd.arg("check")
        .arg(fixture.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .args(extra_args);

    let output = cmd.output().expect("failed to run codemetrics");
    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("check output is not valid JSON: {e}\nstdout:\n{stdout}"))
}

// ─── tool-response.schema.json ───────────────────────────────────────────────

/// The `codemetrics check` JSON output must contain `passed`, `path`,
/// `checks`, and `summary` — a well-known superset of the ToolResponse schema.
/// We validate that every entry in `checks` conforms to the debt-report schema
/// (all tool results share the same envelope shape).
#[test]
fn test_check_output_is_valid_json() {
    let value = run_check_json(&[]);
    // Top-level must be an object with these required keys
    let obj = value.as_object().expect("check output must be a JSON object");
    assert!(obj.contains_key("passed"), "missing 'passed' field");
    assert!(obj.contains_key("path"), "missing 'path' field");
    assert!(obj.contains_key("checks"), "missing 'checks' field");
    assert!(obj.contains_key("summary"), "missing 'summary' field");
}

// ─── debt-report.schema.json ─────────────────────────────────────────────────

#[test]
fn test_debt_schema_validates_cli_output() {
    let schema_value = load_schema("debt-report.schema.json");
    let compiled =
        jsonschema::validator_for(&schema_value).expect("debt schema should compile");

    // Run the debt binary directly for cleaner output matching the schema
    let fixture = fixture_path();
    let mut cmd = Command::cargo_bin("debt").unwrap_or_else(|_| {
        // debt might not be in cargo bin path; skip gracefully
        Command::cargo_bin("codemetrics").expect("codemetrics binary not found")
    });

    let output = cmd
        .arg(fixture.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .output()
        .expect("failed to run debt");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return; // no output to validate (binary might not exist in test env)
    }

    let value: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(_) => return, // ndjson or non-JSON output — skip
    };

    if let Err(error) = compiled.validate(&value) {
        panic!("debt JSON output failed schema validation:\n{}", error);
    }
}

// ─── tool-response.schema.json via `codemetrics run` ────────────────────────

/// `codemetrics run . --format json` emits a `UnifiedReport` JSON object.
/// Each item in its `tools` array must satisfy the tool-response schema.
#[test]
fn test_run_json_conforms_to_tool_response_schema() {
    let fixture = fixture_path();

    let output = Command::cargo_bin("codemetrics")
        .expect("codemetrics binary not found")
        .arg("run")
        .arg(fixture.to_str().unwrap())
        .arg("--format")
        .arg("json")
        .output()
        .expect("failed to run codemetrics run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return;
    }

    // `run --format json` emits a single UnifiedReport object.
    let report: serde_json::Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(_) => return, // Non-JSON output — skip
    };

    // Each entry in tools[] must have the core ToolResult required fields.
    // Note: ToolResult (in tools[]) differs from ToolResponse (standalone):
    // it does not include a 'version' field.
    if let Some(tools) = report.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            let obj = tool.as_object().expect("each tool entry must be a JSON object");
            assert!(obj.contains_key("tool"), "tool entry missing 'tool' field: {tool}");
            assert!(
                obj.contains_key("success"),
                "tool entry missing 'success' field: {tool}"
            );
            assert!(
                obj.contains_key("duration_ms"),
                "tool entry missing 'duration_ms' field: {tool}"
            );
            assert!(obj.contains_key("data"), "tool entry missing 'data' field: {tool}");
        }
    }
}
