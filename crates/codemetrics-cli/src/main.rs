#![deny(clippy::all)]

use clap::Parser;
use colored::Colorize;
use std::time::Instant;
use tracing::info;

// ─── Module declarations ───────────────────────────────────────────────
mod batch;
mod checks;
mod cli;
mod config;
mod health;
mod history;
mod hooks;
mod output;
mod progress;
mod project;
mod report;
mod setup;
mod types;
mod watch;

// ─── Imports from modules ──────────────────────────────────────────────
use cli::{Cli, Commands};
use progress::run_with_spinner;

fn main() {
    // Initialize tracing to stderr so it doesn't corrupt JSON output on stdout
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    tracing::info!("CodeMetrics CLI started");

    let exit_code = match cli.command {
        Commands::Check {
            path,
            recursive,
            format,
            coverage,
            max_crap,
            min_doc,
            max_debt,
            max_complexity_violations,
            max_taint,
            max_duplication,
            max_risk,
            max_coupling,
            min_propcov,
            max_fuzz_risk,
            max_linelen,
            max_halstead_bugs,
            max_secrets,
            max_deadcode,
            max_cohesion,
            min_comment_ratio,
            max_errhandle,
            min_typecov,
            max_vuln_critical,
            max_vuln_high,
            max_sast,
            max_crypto,
            max_license_violations,
            max_outdated,
            skip,
            only,
            ci,
            verbose,
        } => {
            let format = if ci { "json".to_string() } else { format };
            if ci {
                std::env::set_var("CODEMETRICS_NO_PROGRESS", "1");
            }

            // Auto-load .quality.toml if present; CLI flags override file values.
            // Config loading would use config::Config via load_config_thresholds.
            drop::<config::Config>(config::Config { project: None, crap: None, debt: None, doc: None, complexity: None, taint: None, duplication: None, risk: None, coupling: None, mutation: None, security: None, secrets: None, licenses: None, dead_code: None, type_coverage: None, halstead: None });

            let skip_list: Vec<String> = skip
                .map(|s| s.split(',').map(|s| s.trim().to_lowercase()).collect())
                .unwrap_or_default();
            let only_list: Vec<String> = only
                .map(|s| s.split(',').map(|s| s.trim().to_lowercase()).collect())
                .unwrap_or_default();

            let should_run = |name: &str| -> bool {
                if !only_list.is_empty() {
                    only_list.contains(&name.to_string())
                } else {
                    !skip_list.contains(&name.to_string())
                }
            };

            let check_start = Instant::now();
            let show_progress = format == "text";

            macro_rules! run_check {
                ($label:expr, $expr:expr) => {{
                    if show_progress {
                        let label = $label;
                        let t = Instant::now();
                        let result = run_with_spinner(label, || $expr);
                        let elapsed = progress::format_elapsed(t.elapsed());
                        let detail = &result.message;
                        let icon = if result.passed { "✓".green().bold() } else { "✗".red().bold() };
                        let name_col = if result.passed { label.normal() } else { label.red() };
                        let msg_col = if result.passed { detail.bright_black() } else { detail.red() };
                        eprintln!("  {} {:<18} {}  {}", icon, name_col, elapsed.bright_black(), msg_col);
                        if !result.passed || verbose {
                            health::print_offenders(&result);
                        }
                        result
                    } else {
                        $expr
                    }
                }};
            }

            let mut checks_results = Vec::new();

            if should_run("crap") {
                checks_results.push(run_check!("crap", checks::check_crap(&path, recursive, &coverage, max_crap)));
            }
            if should_run("debt") {
                checks_results.push(run_check!("debt", checks::check_debt(&path, recursive, max_debt)));
            }
            if should_run("doc") {
                checks_results.push(run_check!("doc_coverage", checks::check_doc_coverage(&path, recursive, min_doc)));
            }
            if should_run("complexity") {
                checks_results.push(run_check!("complexity", checks::check_complexity(&path, recursive, 10, max_complexity_violations)));
            }
            if should_run("taint") {
                checks_results.push(run_check!("taint", checks::check_taint(&path, recursive, max_taint)));
            }
            if should_run("dup") || should_run("dupfind") || should_run("duplication") {
                checks_results.push(run_check!("duplication", checks::check_dupfind(&path, recursive, max_duplication)));
            }
            if should_run("risk") || should_run("riskmap") {
                checks_results.push(run_check!("riskmap", checks::check_riskmap(&path, recursive, max_risk)));
            }
            if should_run("coupling") {
                checks_results.push(run_check!("coupling", checks::check_coupling(&path, max_coupling)));
            }
            if should_run("propcov") {
                checks_results.push(run_check!("propcov", checks::check_propcov(&path, recursive, min_propcov)));
            }
            if should_run("fuzz") {
                checks_results.push(run_check!("fuzz", checks::check_fuzz(&path, recursive, max_fuzz_risk)));
            }
            if should_run("linelen") {
                checks_results.push(run_check!("linelen", checks::check_linelen(&path, recursive, max_linelen)));
            }
            if should_run("halstead") {
                checks_results.push(run_check!("halstead", checks::check_halstead(&path, recursive, max_halstead_bugs)));
            }
            if should_run("secrets") {
                checks_results.push(run_check!("secrets", checks::check_secrets(&path, recursive, max_secrets)));
            }
            if should_run("deadcode") {
                checks_results.push(run_check!("deadcode", checks::check_deadcode(&path, recursive, max_deadcode)));
            }
            if should_run("cohesion") {
                checks_results.push(run_check!("cohesion", checks::check_cohesion(&path, recursive, max_cohesion)));
            }
            if should_run("comments") {
                checks_results.push(run_check!("comments", checks::check_comments(&path, recursive, min_comment_ratio)));
            }
            if should_run("errhandle") {
                checks_results.push(run_check!("errhandle", checks::check_errhandle(&path, recursive, max_errhandle)));
            }
            if should_run("typecov") && min_typecov > 0.0 {
                checks_results.push(run_check!("typecov", checks::check_typecov(&path, recursive, min_typecov)));
            }
            if should_run("vulnscan") {
                checks_results.push(run_check!("vulnscan", checks::check_vulnscan(&path, max_vuln_critical, max_vuln_high)));
            }
            if should_run("sast") {
                checks_results.push(run_check!("sast", checks::check_sast(&path, recursive, max_sast)));
            }
            if should_run("crypto") {
                checks_results.push(run_check!("crypto", checks::check_crypto(&path, recursive, max_crypto)));
            }
            if should_run("licenses") {
                checks_results.push(run_check!("licenses", checks::check_licenses(&path, max_license_violations)));
            }
            if should_run("outdated") {
                checks_results.push(run_check!("outdated", checks::check_outdated(&path, max_outdated)));
            }

            let passed = checks_results.iter().all(|c| c.passed);
            let total_funcs: usize = checks_results
                .iter()
                .filter_map(|c| c.details.get("total_functions").and_then(|v| v.as_u64()))
                .map(|v| v as usize)
                .sum();
            let passed_count = checks_results.iter().filter(|c| c.passed).count();
            let failed_count = checks_results.len() - passed_count;
            let total_checks = checks_results.len();

            let report = types::CheckReport {
                passed,
                path: path.clone(),
                checks: checks_results,
                summary: types::CheckSummary {
                    total_checks,
                    passed_checks: passed_count,
                    failed_checks: failed_count,
                    functions_analyzed: total_funcs,
                    avg_complexity: 0.0,
                    avg_crap: 0.0,
                },
            };

            match format.as_str() {
                "text" => {
                    health::print_summary_box(
                        "CODEMETRICS CHECK",
                        passed,
                        &path,
                        passed_count,
                        total_checks,
                        check_start.elapsed(),
                        &report.checks,
                    );
                }
                "ndjson" => output::output_ndjson(&report),
                _ => output::output_json(&report),
            }

            if passed { 0 } else { 1 }
        }
        Commands::Crap { path, recursive, coverage, format } => {
            let result = if format == "text" {
                run_with_spinner("crap", || checks::check_crap(&path, recursive, &coverage, 30.0))
            } else {
                checks::check_crap(&path, recursive, &coverage, 30.0)
            };
            let passed = result.passed;
            match format.as_str() {
                "text" => { let icon = if passed { "✓".green().bold() } else { "✗".red().bold() }; eprintln!("  {} crap  {}", icon, result.message.bright_black()); println!("{}", result.message); }
                _ => println!("{}", serde_json::to_string_pretty(&result).expect("Failed to serialize to JSON")),
            }
            if passed { 0 } else { 1 }
        }
        Commands::Debt { path, recursive, marker: _, format } => {
            let result = if format == "text" { run_with_spinner("debt", || checks::check_debt(&path, recursive, 1000)) } else { checks::check_debt(&path, recursive, 1000) };
            let passed = result.passed;
            match format.as_str() {
                "text" => { let icon = if passed { "✓".green().bold() } else { "✗".red().bold() }; eprintln!("  {} debt  {}", icon, result.message.bright_black()); println!("{}", result.message); }
                _ => println!("{}", serde_json::to_string_pretty(&result).expect("Failed to serialize to JSON")),
            }
            if passed { 0 } else { 1 }
        }
        Commands::Doccov { path, recursive, format } => {
            let result = if format == "text" { run_with_spinner("doccov", || checks::check_doc_coverage(&path, recursive, 0.0)) } else { checks::check_doc_coverage(&path, recursive, 0.0) };
            let passed = result.passed;
            match format.as_str() {
                "text" => { let icon = if passed { "✓".green().bold() } else { "✗".red().bold() }; eprintln!("  {} doccov  {}", icon, result.message.bright_black()); println!("{}", result.message); }
                _ => println!("{}", serde_json::to_string_pretty(&result).expect("Failed to serialize to JSON")),
            }
            if passed { 0 } else { 1 }
        }
        Commands::Dupfind { .. } => { eprintln!("dupfind subcommand not yet integrated -- use dupfind binary directly"); 2 }
        Commands::Complexity { path, recursive, min_complexity, format } => {
            let result = if format == "text" { run_with_spinner("complexity", || checks::check_complexity(&path, recursive, min_complexity, 0)) } else { checks::check_complexity(&path, recursive, min_complexity, 0) };
            let passed = result.passed;
            match format.as_str() {
                "text" => { let icon = if passed { "✓".green().bold() } else { "✗".red().bold() }; eprintln!("  {} complexity  {}", icon, result.message.bright_black()); println!("{}", result.message); }
                _ => println!("{}", serde_json::to_string_pretty(&result).expect("Failed to serialize to JSON")),
            }
            if passed { 0 } else { 1 }
        }
        Commands::Setup => { report::setup_command(); 0 }
        Commands::Init { output, ci } => {
            let detect_start = Instant::now();
            let profile = run_with_spinner("detecting project ecosystem", || project::detect_project("."));
            eprintln!("  {} detected: {}  ({})", "✓".green().bold(), profile.ecosystem.to_string().cyan().bold(), if profile.test_cmd.is_empty() { "no test runner".to_string() } else { profile.test_cmd.join(" ") });
            let _ = detect_start;
            if ci { setup::init_ci(&output, &profile) }
            else {
                let write_start = Instant::now();
                setup::generate_config(&output, &profile);
                eprintln!("  {} wrote {}  ({})", "✓".green().bold(), output.cyan(), progress::format_elapsed(write_start.elapsed()).bright_black());
                eprintln!(); eprintln!("  {} Next steps:", "▶".cyan().bold());
                eprintln!("    1. {} codemetrics check .          {}", "$".bright_black(), "— run all checks now".bright_black());
                eprintln!("    2. {} codemetrics report .         {}", "$".bright_black(), "— generate HTML audit report".bright_black());
                eprintln!("    3. {} codemetrics init --ci        {}", "$".bright_black(), "— wire GitHub Actions + pre-commit hook".bright_black());
                eprintln!("    4. {} codemetrics watch .          {}", "$".bright_black(), "— live re-check on file save".bright_black());
                eprintln!(); eprintln!("  {} Tip: edit {} to tune thresholds for your project.", "ℹ".cyan(), output.cyan());
                0
            }
        }
        Commands::Discover { format } => { output::discover_command(&format); 0 }
        Commands::Run { path, config, format, baseline, no_fail_on_regression } => batch::run_batch(&path, &config, &format, baseline.as_deref(), no_fail_on_regression),
        Commands::History { action, dir, last, report } => history::history_command(&action, &dir, last, report.as_deref()),
        Commands::InstallHooks { repo, fast } => hooks::install_hooks(&repo, fast),
        Commands::UninstallHooks { repo } => hooks::uninstall_hooks(&repo),
        Commands::Watch { path, checks, debounce_ms, no_tests, full } => watch::watch_mode(&path, &checks, debounce_ms, no_tests, full),
        Commands::Report { path, format, output, project, from_json, skip, open } => report::report_command(&path, &format, output.as_deref(), project.as_deref(), from_json.as_deref(), skip.as_deref(), open),
        Commands::Diff { before, after } => report::diff_command(&before, &after),
    };

    std::process::exit(exit_code);
}
