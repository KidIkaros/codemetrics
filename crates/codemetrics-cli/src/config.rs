// ═══════════════════════════════════════════
// CONFIG — .quality.toml parsing
// ═══════════════════════════════════════════

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub project: Option<ProjectConfig>,
    pub crap: Option<CrapConfig>,
    pub debt: Option<DebtConfig>,
    pub doc: Option<DocConfig>,
    pub complexity: Option<ComplexityConfig>,
    pub taint: Option<TaintConfig>,
    pub duplication: Option<DuplicationConfig>,
    pub risk: Option<RiskConfig>,
    pub coupling: Option<CouplingConfig>,
    pub mutation: Option<MutationConfig>,
    pub security: Option<SecurityConfig>,
    pub secrets: Option<SecretsConfig>,
    pub licenses: Option<LicensesConfig>,
    pub dead_code: Option<DeadCodeConfig>,
    pub type_coverage: Option<TypeCoverageConfig>,
    pub halstead: Option<HalsteadConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub ecosystem: Option<String>,
    pub test_cmd: Option<String>,
    pub coverage_cmd: Option<String>,
    pub lcov_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CrapConfig {
    pub threshold: Option<f64>,
    pub warn_at: Option<f64>,
    pub max_avg: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DebtConfig {
    pub max_items: Option<usize>,
    pub max_markers: Option<usize>,
    pub types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct DocConfig {
    pub min_coverage: Option<f64>,
    pub min_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ComplexityConfig {
    pub max_violations: Option<usize>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TaintConfig {
    pub max_findings: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DuplicationConfig {
    pub max_duplication: Option<f64>,
    pub max_duplicates: Option<f64>,
    pub min_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RiskConfig {
    pub max_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CouplingConfig {
    pub max_coupling: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MutationConfig {
    pub min_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SecurityConfig {
    pub max_vulnerabilities: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SecretsConfig {
    pub max_findings: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct LicensesConfig {
    pub deny: Option<Vec<String>>,
    pub allow: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct DeadCodeConfig {
    pub max_findings: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct TypeCoverageConfig {
    pub min_coverage: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct HalsteadConfig {
    pub max_bug_estimate: Option<f64>,
}
