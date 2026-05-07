// ═══════════════════════════════════════════
// RESULT TYPES
// ═══════════════════════════════════════════

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct CheckReport {
    pub passed: bool,
    pub path: String,
    pub checks: Vec<CheckResult>,
    pub summary: CheckSummary,
}

#[derive(Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub score: Option<f64>,
    pub threshold: Option<f64>,
    pub message: String,
    pub details: serde_json::Value,
    pub severity: Option<String>,
    pub help: Option<String>,
    pub rule_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CheckSummary {
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub functions_analyzed: usize,
    pub avg_complexity: f64,
    pub avg_crap: f64,
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub binary: String,
    pub description: String,
    pub supported_formats: Vec<String>,
    pub output_fields: Vec<String>,
    pub rule_ids: Vec<String>,
}
