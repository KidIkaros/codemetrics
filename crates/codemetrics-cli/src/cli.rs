// ═══════════════════════════════════════════
// CLI DEFINITION
// ═══════════════════════════════════════════

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "codemetrics",
    about = "Unified code quality tool for Rust. Headless-first, JSON output, CI-ready.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run all CodeMetrics checks and report results
    Check {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(short, long, default_value = "json")]
        format: String,
        #[arg(long)]
        coverage: Option<String>,
        #[arg(long, default_value = "30")]
        max_crap: f64,
        #[arg(long, default_value = "50")]
        min_doc: f64,
        #[arg(long, default_value = "100")]
        max_debt: usize,
        #[arg(long, default_value = "0")]
        max_complexity_violations: usize,
        #[arg(long, default_value = "0")]
        max_taint: usize,
        #[arg(long, default_value = "5.0")]
        max_duplication: f64,
        #[arg(long, default_value = "10.0")]
        max_risk: f64,
        #[arg(long, default_value = "5")]
        max_coupling: usize,
        #[arg(long, default_value = "0.0")]
        min_propcov: f64,
        #[arg(long, default_value = "0")]
        max_fuzz_risk: usize,
        #[arg(long, default_value = "0")]
        max_linelen: usize,
        #[arg(long, default_value = "2.0")]
        max_halstead_bugs: f64,
        #[arg(long, default_value = "0")]
        max_secrets: usize,
        #[arg(long, default_value = "10")]
        max_deadcode: usize,
        #[arg(long, default_value = "5")]
        max_cohesion: usize,
        #[arg(long, default_value = "0.05")]
        min_comment_ratio: f64,
        #[arg(long, default_value = "50")]
        max_errhandle: usize,
        #[arg(long, default_value = "0.0")]
        min_typecov: f64,
        #[arg(long, default_value = "0")]
        max_vuln_critical: usize,
        #[arg(long, default_value = "0")]
        max_vuln_high: usize,
        #[arg(long, default_value = "0")]
        max_sast: usize,
        #[arg(long, default_value = "0")]
        max_crypto: usize,
        #[arg(long, default_value = "0")]
        max_license_violations: usize,
        #[arg(long, default_value = "0")]
        max_outdated: usize,
        #[arg(long)]
        skip: Option<String>,
        #[arg(long)]
        only: Option<String>,
        #[arg(long)]
        ci: bool,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(long)]
        fix: bool,
    },

    /// Verify environment dependencies (doctor)
    Setup,

    /// CRAP metric only
    Crap {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long)]
        coverage: Option<String>,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Technical debt only
    Debt {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long)]
        marker: Option<String>,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Documentation coverage only
    Doccov {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Code duplication only
    Dupfind {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long, default_value = "5")]
        min_lines: usize,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Cyclomatic complexity report
    Complexity {
        path: String,
        #[arg(short, long)]
        recursive: bool,
        #[arg(long, default_value = "5")]
        min_complexity: u32,
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Generate default config file
    Init {
        #[arg(long, default_value = ".quality.toml")]
        output: String,
        #[arg(long)]
        ci: bool,
    },

    /// Run all CodeMetrics tools in batch mode using .quality.toml config
    Run {
        path: String,
        #[arg(long, default_value = ".quality.toml")]
        config: String,
        #[arg(short, long, default_value = "table")]
        format: String,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(long)]
        no_fail_on_regression: bool,
    },

    /// Record or display CodeMetrics history
    History {
        #[arg(default_value = "show")]
        action: String,
        #[arg(long, default_value = ".codemetrics-history")]
        dir: String,
        #[arg(long, default_value = "10")]
        last: usize,
        #[arg(long)]
        report: Option<String>,
    },

    /// Install a CodeMetrics pre-commit git hook
    InstallHooks {
        #[arg(default_value = ".")]
        repo: String,
        #[arg(long)]
        fast: bool,
    },

    /// Remove the CodeMetrics pre-commit git hook
    UninstallHooks {
        #[arg(default_value = ".")]
        repo: String,
    },

    /// Watch for file changes and re-run relevant checks
    Watch {
        #[arg(default_value = ".")]
        path: String,
        #[arg(long, default_value = "debt,doc,crap")]
        checks: String,
        #[arg(long, default_value = "500")]
        debounce_ms: u64,
        #[arg(long)]
        no_tests: bool,
        #[arg(long)]
        full: bool,
    },

    /// Discover available CodeMetrics tools and their capabilities
    Discover {
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Generate a human-readable audit report (HTML or Markdown) from a check run
    Report {
        #[arg(default_value = ".")]
        path: String,
        #[arg(short, long, default_value = "html")]
        format: String,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        from_json: Option<String>,
        #[arg(long)]
        skip: Option<String>,
        #[arg(long)]
        open: bool,
    },

    /// Compare two check JSON snapshots and show regressions or improvements
    Diff {
        before: String,
        after: String,
    },
}
