// ═══════════════════════════════════════════
// REPORT GENERATION — HTML/Markdown/PDF + diff + setup
// ═══════════════════════════════════════════

use colored::Colorize;
use crate::health::health_score;
use crate::types::CheckReport;

pub fn open_in_browser(path: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", path])
        .spawn();
}

pub fn report_command(
    path: &str,
    format: &str,
    output: Option<&str>,
    project: Option<&str>,
    from_json: Option<&str>,
    skip: Option<&str>,
    open: bool,
) -> i32 {
    let check_report: CheckReport = if let Some(json_path) = from_json {
        let content = match std::fs::read_to_string(json_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading {}: {}", json_path, e);
                return 2;
            }
        };
        match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error parsing JSON: {}", e);
                return 2;
            }
        }
    } else {
        eprintln!("  {} Running checks…", "·".bright_black());
        let mut args = vec![path, "--format", "json"];
        if let Some(s) = skip {
            args.push("--skip");
            args.push(s);
        }
        let output_bytes = std::process::Command::new(
            std::env::current_exe().unwrap_or_else(|_| "codemetrics".into()),
        )
        .arg("check")
        .args(&args)
        .output();
        match output_bytes {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                match serde_json::from_str(&stdout) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to parse check output: {}", e);
                        return 2;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to run checks: {}", e);
                return 2;
            }
        }
    };

    let project_name = project
        .map(|s| s.to_string())
        .or_else(|| {
            std::fs::canonicalize(path)
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        })
        .unwrap_or_else(|| path.to_string());

    let now = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let hh = (secs % 86400) / 3600;
        let mm = (secs % 3600) / 60;
        let mut days = (secs / 86400) as i64;
        let mut year = 1970i64;
        loop {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            let y_days = if leap { 366 } else { 365 };
            if days < y_days {
                break;
            }
            days -= y_days;
            year += 1;
        }
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let month_days = [
            31i64,
            if leap { 29 } else { 28 },
            31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
        ];
        let mut month = 0usize;
        for &md in &month_days {
            if days < md {
                break;
            }
            days -= md;
            month += 1;
        }
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} UTC",
            year,
            month + 1,
            days + 1,
            hh,
            mm
        )
    };

    let passed = check_report.passed;
    let total = check_report.summary.total_checks;
    let passed_n = check_report.summary.passed_checks;
    let failed_n = check_report.summary.failed_checks;

    let security_tools = [
        "secrets", "vulnscan", "taint", "errhandle", "sast", "crypto",
    ];
    let compliance_tools = ["licenses", "sbom"];
    let quality_tools = [
        "crap", "debt", "doc_coverage", "complexity", "duplication", "cohesion", "coupling",
        "riskmap", "linelen", "halstead", "deadcode", "comments", "propcov", "fuzz", "typecov",
    ];

    match format {
        "markdown" | "md" => {
            let md = render_markdown_report(
                &check_report,
                &project_name,
                &now,
                &security_tools,
                &quality_tools,
                &compliance_tools,
            );
            let out_path = output.unwrap_or("codemetrics-report.md");
            std::fs::write(out_path, &md).expect("Failed to write report");
            eprintln!("  {} Report written to {}", "✓".green().bold(), out_path);
            if open {
                open_in_browser(out_path);
            }
        }
        "pdf" => {
            let html = render_html_report(
                &check_report,
                &project_name,
                &now,
                &security_tools,
                &quality_tools,
                &compliance_tools,
            );
            let html_tmp = "/tmp/codemetrics-report-tmp.html";
            std::fs::write(html_tmp, &html).expect("Failed to write temp HTML");
            let pdf_path = output.unwrap_or("codemetrics-report.pdf");
            let browser = [
                "chromium",
                "chromium-browser",
                "google-chrome",
                "google-chrome-stable",
            ]
            .iter()
            .find(|b| {
                std::process::Command::new(b)
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            })
            .copied();
            match browser {
                Some(bin) => {
                    let abs_html = std::fs::canonicalize(html_tmp)
                        .map(|p| format!("file://{}", p.display()))
                        .unwrap_or_else(|_| format!("file://{}", html_tmp));
                    let result = std::process::Command::new(bin)
                        .args([
                            "--headless",
                            "--disable-gpu",
                            "--no-sandbox",
                            &format!("--print-to-pdf={}", pdf_path),
                            &abs_html,
                        ])
                        .output();
                    match result {
                        Ok(o) if o.status.success() => {
                            eprintln!("  {} Report written to {}", "✓".green().bold(), pdf_path);
                            if open {
                                open_in_browser(pdf_path);
                            }
                        }
                        Ok(o) => {
                            let err = String::from_utf8_lossy(&o.stderr);
                            eprintln!(
                                "  {} PDF conversion failed: {}",
                                "✗".red().bold(),
                                err.lines().next().unwrap_or("unknown error")
                            );
                            eprintln!("  {} HTML saved to {}", "ℹ".cyan(), html_tmp);
                        }
                        Err(e) => eprintln!("  {} Could not run {}: {}", "✗".red().bold(), bin, e),
                    }
                }
                None => {
                    eprintln!(
                        "  {} No Chromium/Chrome found — falling back to HTML",
                        "!".yellow().bold()
                    );
                    let out_path = output.unwrap_or("codemetrics-report.html");
                    std::fs::write(out_path, &html).expect("Failed to write HTML report");
                    eprintln!("  {} Report written to {}", "✓".green().bold(), out_path);
                    if open {
                        open_in_browser(out_path);
                    }
                }
            }
        }
        _ => {
            let html = render_html_report(
                &check_report,
                &project_name,
                &now,
                &security_tools,
                &quality_tools,
                &compliance_tools,
            );
            let out_path = output.unwrap_or("codemetrics-report.html");
            std::fs::write(out_path, &html).expect("Failed to write report");
            eprintln!("  {} Report written to {}", "✓".green().bold(), out_path);
            eprintln!(
                "  {} {} checks: {}/{} passed",
                if passed { "✓".green().bold() } else { "✗".red().bold() },
                total,
                passed_n,
                total
            );
            if open {
                open_in_browser(out_path);
            }
        }
    }

    let _ = (passed_n, failed_n);
    if passed { 0 } else { 1 }
}

// ─── HTML helpers ───────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn severity_color_html(sev: &str) -> &'static str {
    match sev {
        "high" | "critical" | "error" => "#ef4444",
        "medium" | "warning" => "#f59e0b",
        "low" => "#3b82f6",
        _ => "#6b7280",
    }
}

fn severity_badge(sev: &str) -> String {
    let color = severity_color_html(sev);
    format!(
        r#"<span style="background:{c};color:#fff;padding:2px 8px;border-radius:12px;font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.03em">{s}</span>"#,
        c = color, s = sev
    )
}

fn offender_rows_html(c: &crate::types::CheckResult) -> String {
    let arrays = [
        "items", "functions", "findings", "violations", "secrets", "duplicates",
    ];
    for key in &arrays {
        if let Some(arr) = c.details.get(key).and_then(|v| v.as_array()) {
            if arr.is_empty() {
                continue;
            }
            let mut rows = String::new();
            for item in arr.iter().take(10) {
                let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let line = item
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .map(|l| format!(":{}", l))
                    .unwrap_or_default();
                let desc = item
                    .get("context")
                    .or_else(|| item.get("kind"))
                    .or_else(|| item.get("name"))
                    .or_else(|| item.get("type"))
                    .or_else(|| item.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let loc = if file.is_empty() {
                    String::new()
                } else {
                    format!("{}{}", file, line)
                };
                let desc_trunc = if desc.len() > 80 {
                    format!("{}…", &desc[..80])
                } else {
                    desc.to_string()
                };
                rows.push_str(&format!(
                    r#"<div style="display:flex;gap:12px;padding:4px 0;border-bottom:1px solid #f3f4f6;font-size:12px">
  <span style="color:#6366f1;font-family:monospace;white-space:nowrap;min-width:180px">{}</span>
  <span style="color:#6b7280">{}</span>
</div>"#,
                    html_escape(&loc),
                    html_escape(&desc_trunc)
                ));
            }
            let more = if arr.len() > 10 {
                format!(
                    r#"<div style="font-size:12px;color:#9ca3af;padding-top:6px">… {} more findings</div>"#,
                    arr.len() - 10
                )
            } else {
                String::new()
            };
            return format!(
                r#"<details style="margin-top:8px">
<summary style="font-size:12px;color:#6366f1;cursor:pointer;user-select:none;padding:4px 0">▶ Show {} finding{}</summary>
<div style="margin-top:8px;padding:8px 12px;background:#f9fafb;border-radius:6px;border-left:3px solid #6366f1">
{}{}
</div>
</details>"#,
                arr.len().min(10),
                if arr.len() == 1 { "" } else { "s" },
                rows,
                more
            );
        }
    }
    String::new()
}

fn check_row_html(c: &crate::types::CheckResult) -> String {
    let icon = if c.passed { "&#10003;" } else { "&#10007;" };
    let icon_color = if c.passed { "#22c55e" } else { "#ef4444" };
    let row_bg = if c.passed { "#fff" } else { "#fef2f2" };
    let name_color = if c.passed { "#111827" } else { "#ef4444" };
    let sev = c.severity.as_deref().unwrap_or("info");
    let help = c.help.as_deref().unwrap_or("");
    let score_str = match (c.score, c.threshold) {
        (Some(s), Some(t)) => format!("{:.1} / {:.1}", s, t),
        (Some(s), None) => format!("{:.1}", s),
        _ => "&#8212;".to_string(),
    };
    let offenders = if !c.passed {
        offender_rows_html(c)
    } else {
        String::new()
    };
    let msg_cell = format!(
        "<div style=\"font-size:13px;color:#374151\">{msg}</div><div style=\"font-size:11px;color:#9ca3af;margin-top:3px\">{help}</div>{off}",
        msg = html_escape(&c.message),
        help = html_escape(help),
        off = offenders
    );
    format!(
        "<tr style=\"background:{rb};border-bottom:1px solid #f3f4f6;vertical-align:top\">
  <td style=\"padding:12px 14px;font-size:18px;color:{ic};text-align:center;width:40px;font-weight:700\">{icon}</td>
  <td style=\"padding:12px 14px;font-weight:600;font-size:13px;color:{nc};white-space:nowrap\">{name}</td>
  <td style=\"padding:12px 14px\">{mc}</td>
  <td style=\"padding:12px 14px;font-size:12px;color:#6b7280;white-space:nowrap\">{score}</td>
  <td style=\"padding:12px 14px;white-space:nowrap\">{sb}</td>
</tr>",
        rb = row_bg,
        ic = icon_color,
        icon = icon,
        nc = name_color,
        name = c.name,
        mc = msg_cell,
        score = score_str,
        sb = severity_badge(sev),
    )
}

fn donut_svg(pct: f64, color: &str) -> String {
    let circ = 276.46f64;
    let dash = circ * pct / 100.0;
    let gap = circ - dash;
    let pct_int = pct as u32;
    let mut s = String::from(
        r#"<svg viewBox="0 0 100 100" width="120" height="120" style="display:block">"#,
    );
    s.push_str(&format!(
        "\n  <circle cx=\"50\" cy=\"50\" r=\"44\" fill=\"none\" stroke=\"#e5e7eb\" stroke-width=\"10\"/>\n"
    ));
    s.push_str(&format!(
        "  <circle cx=\"50\" cy=\"50\" r=\"44\" fill=\"none\" stroke=\"{}\" stroke-width=\"10\"\n",
        color
    ));
    s.push_str(&format!(
        "    stroke-dasharray=\"{:.2} {:.2}\" stroke-dashoffset=\"69.12\"\n",
        dash, gap
    ));
    s.push_str("    stroke-linecap=\"round\" transform=\"rotate(-90 50 50)\"/>\n");
    s.push_str(&format!("  <text x=\"50\" y=\"46\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"800\" fill=\"{}\" font-family=\"system-ui\">{}%</text>\n", color, pct_int));
    s.push_str("  <text x=\"50\" y=\"60\" text-anchor=\"middle\" font-size=\"9\" fill=\"#9ca3af\" font-family=\"system-ui\">pass rate</text>\n");
    s.push_str("</svg>");
    s
}

fn mini_bar(pass: usize, total: usize, color: &str) -> String {
    if total == 0 {
        return String::new();
    }
    let filled = (pass * 12) / total;
    let bar: String = "█".repeat(filled) + &"░".repeat(12 - filled);
    let pct = pass * 100 / total;
    format!(
        "<div style=\"display:flex;align-items:center;gap:8px;font-size:12px\">
  <span style=\"font-family:monospace;color:{color};letter-spacing:.1em\">{bar}</span>
  <span style=\"color:#6b7280\">{pass}/{total} ({pct}%)</span>
</div>",
        color = color, bar = bar, pass = pass, total = total, pct = pct
    )
}

// ─── HTML report renderer ───────────────────────────────────────────────

fn render_html_report(
    report: &CheckReport,
    project: &str,
    date: &str,
    security_tools: &[&str],
    quality_tools: &[&str],
    compliance_tools: &[&str],
) -> String {
    let (health, grade) = health_score(&report.checks);
    let overall_color = if report.passed { "#22c55e" } else { "#ef4444" };
    let overall_label = if report.passed { "PASSED" } else { "FAILED" };
    let pct = if report.summary.total_checks == 0 {
        100.0
    } else {
        report.summary.passed_checks as f64 / report.summary.total_checks as f64 * 100.0
    };
    let grade_color = match grade {
        'A' => "#22c55e",
        'B' => "#06b6d4",
        'C' => "#f59e0b",
        _ => "#ef4444",
    };

    let mut sec_checks: Vec<&crate::types::CheckResult> = Vec::new();
    let mut qual_checks: Vec<&crate::types::CheckResult> = Vec::new();
    let mut comp_checks: Vec<&crate::types::CheckResult> = Vec::new();
    let mut other_checks: Vec<&crate::types::CheckResult> = Vec::new();
    for c in &report.checks {
        if security_tools.contains(&c.name.as_str()) {
            sec_checks.push(c);
        } else if compliance_tools.contains(&c.name.as_str()) {
            comp_checks.push(c);
        } else if quality_tools.contains(&c.name.as_str()) {
            qual_checks.push(c);
        } else {
            other_checks.push(c);
        }
    }
    qual_checks.extend(other_checks);

    let sec_pass = sec_checks.iter().filter(|c| c.passed).count();
    let qual_pass = qual_checks.iter().filter(|c| c.passed).count();
    let comp_pass = comp_checks.iter().filter(|c| c.passed).count();
    let sec_col = if sec_pass == sec_checks.len() { "#22c55e" } else { "#ef4444" };
    let qual_col = if qual_pass == qual_checks.len() { "#22c55e" } else { "#ef4444" };
    let comp_col = if comp_pass == comp_checks.len() { "#22c55e" } else { "#ef4444" };

    let failed_checks: Vec<&crate::types::CheckResult> =
        report.checks.iter().filter(|c| !c.passed).collect();

    let risk_domain = if sec_checks.iter().any(|c| !c.passed) {
        "security"
    } else if comp_checks.iter().any(|c| !c.passed) {
        "compliance"
    } else if qual_checks.iter().any(|c| !c.passed) {
        "code quality"
    } else {
        "none"
    };
    let exec_verdict = if report.passed {
        format!("This codebase passed all {} checks with a health score of {}/100 (grade {}). No critical findings were detected across security, quality, or compliance domains.", report.summary.total_checks, health, grade)
    } else {
        let high_count = failed_checks
            .iter()
            .filter(|c| matches!(c.severity.as_deref(), Some("high") | Some("critical") | Some("error")))
            .count();
        format!(
            "{} of {} checks failed, concentrated in {}. {} finding{} rated high/critical severity require immediate attention before the next release.",
            failed_checks.len(), report.summary.total_checks, risk_domain,
            high_count, if high_count == 1 { "" } else { "s" }
        )
    };
    let top3: Vec<&crate::types::CheckResult> = {
        let mut sorted = failed_checks.clone();
        sorted.sort_by_key(|c| match c.severity.as_deref() {
            Some("critical") => 0,
            Some("high") | Some("error") => 1,
            Some("medium") | Some("warning") => 2,
            _ => 3,
        });
        sorted.into_iter().take(3).collect()
    };
    let top3_html = if top3.is_empty() {
        r#"<p style="color:#22c55e;font-size:14px">✓ No action items — all checks passed.</p>"#.to_string()
    } else {
        let mut h = String::new();
        for (i, c) in top3.iter().enumerate() {
            let sev = c.severity.as_deref().unwrap_or("medium");
            let effort = match sev {
                "critical" | "high" | "error" => "High effort",
                "medium" | "warning" => "Medium effort",
                _ => "Low effort",
            };
            let help = c.help.as_deref().unwrap_or("Review and fix flagged items.");
            h.push_str(&format!(
                r#"<div style="display:flex;gap:14px;padding:12px 0;border-bottom:1px solid #f3f4f6;align-items:flex-start">
  <div style="font-size:20px;font-weight:800;color:#d1d5db;min-width:24px">{}</div>
  <div style="flex:1">
    <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px">
      <span style="font-weight:700;font-size:14px">{}</span>{}
      <span style="font-size:11px;color:#9ca3af;margin-left:auto">{}</span>
    </div>
    <div style="font-size:13px;color:#6b7280">{}</div>
  </div>
</div>"#,
                i + 1,
                html_escape(&c.name),
                severity_badge(sev),
                effort,
                html_escape(help)
            ));
        }
        h
    };

    let remediation_html = if failed_checks.is_empty() {
        r#"<p style="color:#22c55e;font-weight:600;font-size:14px">✓ No findings — all checks passed.</p>"#.to_string()
    } else {
        let mut rows = String::new();
        let mut sorted_failed = failed_checks.clone();
        sorted_failed.sort_by_key(|c| match c.severity.as_deref() {
            Some("critical") => 0,
            Some("high") | Some("error") => 1,
            Some("medium") | Some("warning") => 2,
            _ => 3,
        });
        for (i, c) in sorted_failed.iter().enumerate() {
            let sev = c.severity.as_deref().unwrap_or("medium");
            let effort = match sev {
                "critical" | "high" | "error" => "High",
                "medium" | "warning" => "Medium",
                _ => "Low",
            };
            let help = c
                .help
                .as_deref()
                .unwrap_or("Review and fix the flagged items.");
            rows.push_str(&format!(
                r#"<tr style="border-bottom:1px solid #f3f4f6">
  <td style="padding:10px 14px;font-weight:700;color:#9ca3af">{}</td>
  <td style="padding:10px 14px;font-weight:600">{}</td>
  <td style="padding:10px 14px">{}</td>
  <td style="padding:10px 14px;font-size:12px;color:#6b7280">{}</td>
  <td style="padding:10px 14px;font-size:12px;color:#6b7280">{}</td>
</tr>"#,
                i + 1,
                html_escape(&c.name),
                severity_badge(sev),
                effort,
                html_escape(help),
            ));
        }
        format!(
            r#"<table style="width:100%;border-collapse:collapse;font-size:13px">
<thead><tr style="background:#f9fafb;border-bottom:2px solid #e5e7eb">
  <th style="padding:8px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600">#</th>
  <th style="padding:8px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600">Check</th>
  <th style="padding:8px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600">Severity</th>
  <th style="padding:8px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600">Effort</th>
  <th style="padding:8px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600">Action</th>
</tr></thead><tbody>{rows}</tbody></table>"#,
            rows = rows
        )
    };

    fn section_html(
        title: &str,
        icon: &str,
        anchor: &str,
        checks: &[&crate::types::CheckResult],
    ) -> String {
        if checks.is_empty() {
            return String::new();
        }
        let rows: String = checks.iter().map(|c| check_row_html(c)).collect();
        let pass_c = checks.iter().filter(|c| c.passed).count();
        let fail_c = checks.len() - pass_c;
        let status_color = if fail_c == 0 { "#22c55e" } else { "#ef4444" };
        let status_pill = if fail_c == 0 {
            r#"<span style="background:#dcfce7;color:#16a34a;padding:2px 10px;border-radius:12px;font-size:11px;font-weight:600">ALL PASSED</span>"#.to_string()
        } else {
            format!(
                r#"<span style="background:#fee2e2;color:#ef4444;padding:2px 10px;border-radius:12px;font-size:11px;font-weight:600">{} FAILED</span>"#,
                fail_c
            )
        };
        format!(
            "<section id=\"{anch}\" style=\"margin-bottom:40px\">
<div style=\"display:flex;align-items:center;gap:12px;margin-bottom:16px;padding-bottom:12px;border-bottom:2px solid #f3f4f6\">
  <span style=\"font-size:22px\">{icn}</span>
  <h2 style=\"font-size:18px;font-weight:800;color:#111827;margin:0\">{ttl}</h2>
  <span style=\"font-size:13px;color:{sc};font-weight:600;margin-left:4px\">{ps}/{tot}</span>
  <div style=\"margin-left:auto\">{pill}</div>
</div>
<div style=\"border-radius:10px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,.08)\">
<table style=\"width:100%;border-collapse:collapse;font-size:13px\">
<thead><tr style=\"background:#f9fafb;border-bottom:2px solid #e5e7eb\">
  <th style=\"padding:9px 14px;width:42px\"></th>
  <th style=\"padding:9px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600\">Check</th>
  <th style=\"padding:9px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600\">Result / Details</th>
  <th style=\"padding:9px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600\">Score</th>
  <th style=\"padding:9px 14px;text-align:left;font-size:11px;text-transform:uppercase;color:#9ca3af;font-weight:600\">Sev</th>
</tr></thead>
<tbody>{rows}</tbody>
</table></div></section>",
            anch = anchor, icn = icon, ttl = title, sc = status_color,
            ps = pass_c, tot = checks.len(), pill = status_pill, rows = rows,
        )
    }

    let sec_section = section_html("Security Checks", "🔒", "security", &sec_checks);
    let qual_section = section_html("Code Quality Checks", "📊", "quality", &qual_checks);
    let comp_section = section_html("Compliance Checks", "📋", "compliance", &comp_checks);

    let donut = donut_svg(pct, overall_color);
    let sec_bar = mini_bar(sec_pass, sec_checks.len(), sec_col);
    let qual_bar = mini_bar(qual_pass, qual_checks.len(), qual_col);
    let comp_bar = mini_bar(comp_pass, comp_checks.len(), comp_col);

    let tmpl = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>CodeMetrics Audit — __PROJECT__</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#f1f5f9;color:#1e293b;line-height:1.5}
a{color:#6366f1;text-decoration:none}
.layout{display:grid;grid-template-columns:220px 1fr;min-height:100vh}
.sidebar{background:#0f172a;color:#94a3b8;padding:28px 0;position:sticky;top:0;height:100vh;overflow-y:auto}
.sidebar .brand{padding:0 20px 24px;border-bottom:1px solid #1e293b;margin-bottom:20px}
.sidebar .brand-name{font-size:13px;font-weight:700;color:#e2e8f0;text-transform:uppercase;letter-spacing:.08em}
.sidebar .brand-sub{font-size:11px;color:#475569;margin-top:2px}
.sidebar nav a{display:flex;align-items:center;gap:10px;padding:9px 20px;font-size:13px;color:#94a3b8;transition:background .15s}
.sidebar nav a:hover,.sidebar nav a.active{background:#1e293b;color:#e2e8f0}
.sidebar .nav-icon{font-size:15px;width:20px;text-align:center}
.sidebar .cat-score{margin-left:auto;font-size:11px;font-weight:600;padding:1px 7px;border-radius:10px}
.sidebar .score-pass{background:#14532d;color:#86efac}
.sidebar .score-fail{background:#7f1d1d;color:#fca5a5}
.sidebar .divider{border:none;border-top:1px solid #1e293b;margin:12px 20px}
.main{padding:36px 40px 60px;max-width:1000px}
.page-header{background:linear-gradient(135deg,#1e293b 0%,#0f172a 100%);color:#fff;padding:32px 36px;border-radius:14px;margin-bottom:32px;display:flex;gap:32px;align-items:center}
.page-header-text{flex:1}
.page-header .eyebrow{font-size:11px;color:#64748b;text-transform:uppercase;letter-spacing:.1em;margin-bottom:8px}
.page-header h1{font-size:26px;font-weight:800;letter-spacing:-.3px;color:#f8fafc}
.page-header .meta{font-size:12px;color:#64748b;margin-top:6px}
.verdict-pill{display:inline-flex;align-items:center;gap:8px;background:rgba(255,255,255,.08);border:1px solid rgba(255,255,255,.12);padding:8px 18px;border-radius:8px;margin-top:14px}
.verdict-pill .v-label{font-size:18px;font-weight:800;color:__OC__}
.verdict-pill .v-sub{font-size:13px;color:#94a3b8}
.score-ring{display:flex;flex-direction:column;align-items:center;gap:4px}
.grade-badge{font-size:40px;font-weight:900;color:__GC__;line-height:1}
.grade-label{font-size:11px;color:#64748b;text-transform:uppercase;letter-spacing:.06em}
.stat-grid{display:grid;grid-template-columns:repeat(4,1fr);gap:16px;margin-bottom:32px}
.stat-card{background:#fff;border-radius:12px;padding:20px;box-shadow:0 1px 3px rgba(0,0,0,.06);text-align:center;border:1px solid #f1f5f9}
.stat-card .n{font-size:34px;font-weight:900;line-height:1}
.stat-card .lbl{font-size:11px;color:#94a3b8;margin-top:6px;text-transform:uppercase;letter-spacing:.06em;font-weight:500}
.card{background:#fff;border-radius:12px;padding:28px;box-shadow:0 1px 3px rgba(0,0,0,.06);margin-bottom:28px;border:1px solid #f1f5f9}
.card-title{font-size:15px;font-weight:700;color:#0f172a;margin-bottom:16px;display:flex;align-items:center;gap:8px}
.cat-bars{display:flex;flex-direction:column;gap:14px}
.cat-bar-row{display:flex;align-items:center;gap:12px}
.cat-bar-label{font-size:13px;color:#475569;min-width:120px;font-weight:500}
.footer{font-size:12px;color:#94a3b8;text-align:center;margin-top:48px;padding-top:24px;border-top:1px solid #e2e8f0}
@media(max-width:768px){.layout{grid-template-columns:1fr}.sidebar{position:static;height:auto}.main{padding:20px}.stat-grid{grid-template-columns:repeat(2,1fr)}.page-header{flex-direction:column}}
@media print{.sidebar{display:none}.layout{grid-template-columns:1fr}.main{padding:20px}body{background:#fff}}
</style>
</head>
<body>
<div class="layout">
<aside class="sidebar">
  <div class="brand">
    <div class="brand-name">CodeMetrics</div>
    <div class="brand-sub">Audit Report</div>
  </div>
  <nav>
    <a href="#overview" class="active"><span class="nav-icon">&#128203;</span>Overview
      <span class="cat-score __OS__">__OL__</span>
    </a>
    <hr class="divider">
    <a href="#security"><span class="nav-icon">&#128274;</span>Security
      <span class="cat-score __SS__">__SEC_PASS__/__SEC_TOTAL__</span>
    </a>
    <a href="#quality"><span class="nav-icon">&#128202;</span>Quality
      <span class="cat-score __QS__">__QUAL_PASS__/__QUAL_TOTAL__</span>
    </a>
    <a href="#compliance"><span class="nav-icon">&#128203;</span>Compliance
      <span class="cat-score __CS__">__COMP_PASS__/__COMP_TOTAL__</span>
    </a>
    <hr class="divider">
    <a href="#remediation"><span class="nav-icon">&#128295;</span>Remediation</a>
  </nav>
</aside>
<main class="main">
<div id="overview">
<div class="page-header">
  <div class="page-header-text">
    <div class="eyebrow">Automated Code Audit Report</div>
    <h1>__PROJECT__</h1>
    <div class="meta">Generated __DATE__ &nbsp;&middot;&nbsp; Path: __PATH__ &nbsp;&middot;&nbsp; CodeMetrics v__VERSION__</div>
    <div class="verdict-pill">
      <span class="v-label">__OVERALL_LABEL__</span>
      <span class="v-sub">__PCT__% of __TOTAL__ checks passed</span>
    </div>
  </div>
  <div class="score-ring">
    __DONUT__
    <div class="grade-badge" style="color:__GC__">__GRADE__</div>
    <div class="grade-label">Health Grade</div>
  </div>
</div>
<div class="stat-grid">
  <div class="stat-card"><div class="n" style="color:#1e293b">__TOTAL__</div><div class="lbl">Total Checks</div></div>
  <div class="stat-card"><div class="n" style="color:#22c55e">__PASSED_N__</div><div class="lbl">Passed</div></div>
  <div class="stat-card"><div class="n" style="color:#ef4444">__FAILED_N__</div><div class="lbl">Failed</div></div>
  <div class="stat-card"><div class="n" style="color:__GC__">__HEALTH__/100</div><div class="lbl">Health Score</div></div>
</div>
<div class="card">
  <div class="card-title">&#128200; Category Breakdown</div>
  <div class="cat-bars">
    <div class="cat-bar-row"><span class="cat-bar-label">&#128274; Security</span>__SEC_BAR__</div>
    <div class="cat-bar-row"><span class="cat-bar-label">&#128202; Quality</span>__QUAL_BAR__</div>
    <div class="cat-bar-row"><span class="cat-bar-label">&#128203; Compliance</span>__COMP_BAR__</div>
  </div>
</div>
<div class="card">
  <div class="card-title">&#127919; Executive Summary</div>
  <p style="font-size:14px;color:#475569;line-height:1.6;margin-bottom:20px">__EXEC_VERDICT__</p>
  <div style="font-weight:700;font-size:13px;color:#0f172a;margin-bottom:12px">Top Priority Actions</div>
  __TOP3_HTML__
</div>
</div>
<div class="card" id="remediation" style="scroll-margin-top:80px">
  <div class="card-title">&#128295; Remediation Checklist
    <span style="font-size:12px;font-weight:400;color:#9ca3af;margin-left:4px">&#8212; ranked by severity</span>
  </div>
  <p style="font-size:13px;color:#9ca3af;margin-bottom:16px">Address Critical and High items before any release.</p>
  __REMEDIATION_HTML__
</div>
__QUAL_SECTION__
__SEC_SECTION__
__COMP_SECTION__
<div class="footer">
  Generated by <strong>CodeMetrics</strong> &#8212; automated code quality &amp; security auditing &nbsp;&middot;&nbsp; __DATE__<br>
  <span style="color:#cbd5e1">This report is machine-generated. Results should be reviewed by a qualified engineer before use in compliance filings.</span>
</div>
</main>
</div>
<script>
const sections = document.querySelectorAll('[id]');
const links = document.querySelectorAll('.sidebar nav a');
const obs = new IntersectionObserver(entries => {
  entries.forEach(e => {
    if(e.isIntersecting){
      links.forEach(l=>l.classList.remove('active'));
      const a = document.querySelector('.sidebar nav a[href="#'+e.target.id+'"]');
      if(a) a.classList.add('active');
    }
  });
},{threshold:0.3});
sections.forEach(s=>obs.observe(s));
</script>
</body>
</html>"##;

    tmpl.replace("__PROJECT__", &html_escape(project))
        .replace("__DATE__", date)
        .replace("__PATH__", &html_escape(&report.path))
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
        .replace("__OC__", overall_color)
        .replace("__GC__", grade_color)
        .replace("__GRADE__", &grade.to_string())
        .replace("__OVERALL_LABEL__", overall_label)
        .replace("__PCT__", &format!("{:.0}", pct))
        .replace("__TOTAL__", &report.summary.total_checks.to_string())
        .replace("__PASSED_N__", &report.summary.passed_checks.to_string())
        .replace("__FAILED_N__", &report.summary.failed_checks.to_string())
        .replace("__HEALTH__", &health.to_string())
        .replace("__DONUT__", &donut)
        .replace("__SEC_BAR__", &sec_bar)
        .replace("__QUAL_BAR__", &qual_bar)
        .replace("__COMP_BAR__", &comp_bar)
        .replace("__SEC_PASS__", &sec_pass.to_string())
        .replace("__SEC_TOTAL__", &sec_checks.len().to_string())
        .replace("__QUAL_PASS__", &qual_pass.to_string())
        .replace("__QUAL_TOTAL__", &qual_checks.len().to_string())
        .replace("__COMP_PASS__", &comp_pass.to_string())
        .replace("__COMP_TOTAL__", &comp_checks.len().to_string())
        .replace("__SS__", if sec_pass == sec_checks.len() { "score-pass" } else { "score-fail" })
        .replace("__QS__", if qual_pass == qual_checks.len() { "score-pass" } else { "score-fail" })
        .replace("__CS__", if comp_pass == comp_checks.len() { "score-pass" } else { "score-fail" })
        .replace("__OS__", if report.passed { "score-pass" } else { "score-fail" })
        .replace("__OL__", if report.passed { "PASS" } else { "FAIL" })
        .replace("__EXEC_VERDICT__", &html_escape(&exec_verdict))
        .replace("__TOP3_HTML__", &top3_html)
        .replace("__REMEDIATION_HTML__", &remediation_html)
        .replace("__QUAL_SECTION__", &qual_section)
        .replace("__SEC_SECTION__", &sec_section)
        .replace("__COMP_SECTION__", &comp_section)
}

// ─── Markdown report renderer ───────────────────────────────────────────

fn render_markdown_report(
    report: &CheckReport,
    project: &str,
    date: &str,
    security_tools: &[&str],
    quality_tools: &[&str],
    compliance_tools: &[&str],
) -> String {
    let overall = if report.passed { "✅ PASSED" } else { "❌ FAILED" };
    let pct = if report.summary.total_checks == 0 {
        100.0
    } else {
        report.summary.passed_checks as f64 / report.summary.total_checks as f64 * 100.0
    };

    let mut md = format!(
        "# CodeMetrics Audit Report — {}\n\n\
         **Status:** {}  \n\
         **Generated:** {}  \n\
         **Path:** `{}`  \n\
         **Version:** CodeMetrics v{}\n\n\
         ---\n\n\
         ## Summary\n\n\
         | Metric | Value |\n|---|---|\n\
         | Total Checks | {} |\n\
         | Passed | {} |\n\
         | Failed | {} |\n\
         | Pass Rate | {:.0}% |\n\n",
        project, overall, date, report.path, env!("CARGO_PKG_VERSION"),
        report.summary.total_checks, report.summary.passed_checks,
        report.summary.failed_checks, pct,
    );

    let failed: Vec<&crate::types::CheckResult> = report.checks.iter().filter(|c| !c.passed).collect();
    if !failed.is_empty() {
        md.push_str("## Remediation Checklist\n\n");
        md.push_str("| # | Check | Severity | Effort | Action |\n|---|---|---|---|---|\n");
        for (i, c) in failed.iter().enumerate() {
            let sev = c.severity.as_deref().unwrap_or("medium");
            let effort = match sev {
                "critical" | "high" => "High",
                "medium" => "Medium",
                _ => "Low",
            };
            let help = c.help.as_deref().unwrap_or("Review and fix.");
            md.push_str(&format!(
                "| {} | `{}` | {} | {} | {} |\n",
                i + 1, c.name, sev, effort, help
            ));
        }
        md.push('\n');
    }

    let categories: &[(&str, &str, &[&str])] = &[
        ("Code Quality Checks", "📊", quality_tools),
        ("Security Checks", "🔒", security_tools),
        ("Compliance Checks", "📋", compliance_tools),
    ];
    for (title, icon, tools) in categories {
        let checks: Vec<&crate::types::CheckResult> = report
            .checks
            .iter()
            .filter(|c| tools.contains(&c.name.as_str()))
            .collect();
        if checks.is_empty() {
            continue;
        }
        md.push_str(&format!("## {} {}\n\n", icon, title));
        md.push_str("| Check | Status | Score | Severity | Message |\n|---|---|---|---|---|\n");
        for c in &checks {
            let status = if c.passed { "✅" } else { "❌" };
            let sev = c.severity.as_deref().unwrap_or("info");
            let score = match (c.score, c.threshold) {
                (Some(s), Some(t)) => format!("{:.1}/{:.1}", s, t),
                (Some(s), None) => format!("{:.1}", s),
                _ => "—".to_string(),
            };
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                c.name, status, score, sev, c.message
            ));
        }
        md.push('\n');
    }

    md.push_str("---\n\n*Generated by CodeMetrics — automated code quality & security auditing.*  \n");
    md.push_str("*This report is machine-generated. Results should be reviewed by a qualified engineer before use in compliance filings.*\n");
    md
}

// ─── diff_command ───────────────────────────────────────────────────────

pub fn diff_command(before_path: &str, after_path: &str) -> i32 {
    let load = |p: &str| -> Option<CheckReport> {
        let content = std::fs::read_to_string(p)
            .map_err(|e| eprintln!("Error reading {}: {}", p, e))
            .ok()?;
        serde_json::from_str(&content)
            .map_err(|e| eprintln!("Error parsing {}: {}", p, e))
            .ok()
    };

    let before = match load(before_path) {
        Some(r) => r,
        None => return 2,
    };
    let after = match load(after_path) {
        Some(r) => r,
        None => return 2,
    };

    let mut regressions: Vec<&str> = Vec::new();
    let mut fixes: Vec<&str> = Vec::new();
    let mut unchanged_pass = 0usize;
    let mut unchanged_fail = 0usize;

    for ac in &after.checks {
        if let Some(bc) = before.checks.iter().find(|b| b.name == ac.name) {
            match (bc.passed, ac.passed) {
                (true, false) => regressions.push(&ac.name),
                (false, true) => fixes.push(&ac.name),
                (true, true) => unchanged_pass += 1,
                (false, false) => unchanged_fail += 1,
            }
        }
    }

    let new_checks: Vec<&str> = after
        .checks
        .iter()
        .filter(|ac| !before.checks.iter().any(|bc| bc.name == ac.name))
        .map(|ac| ac.name.as_str())
        .collect();

    eprintln!();
    eprintln!(
        "  {} {} → {}",
        "diff".bright_black(),
        before_path.cyan(),
        after_path.cyan()
    );
    eprintln!();

    if regressions.is_empty() && fixes.is_empty() && new_checks.is_empty() {
        eprintln!(
            "  {} No changes — {} pass, {} fail (unchanged)",
            "◉".bright_black(),
            unchanged_pass,
            unchanged_fail
        );
    } else {
        for name in &fixes {
            eprintln!(
                "  {} {} {}",
                "↑".green().bold(),
                name.green().bold(),
                "now passing".green()
            );
        }
        for name in &regressions {
            eprintln!(
                "  {} {} {}",
                "↓".red().bold(),
                name.red().bold(),
                "now failing".red()
            );
        }
        for name in &new_checks {
            let status = after
                .checks
                .iter()
                .find(|c| c.name == *name)
                .map(|c| c.passed)
                .unwrap_or(false);
            let icon = if status {
                "✓".green().bold().to_string()
            } else {
                "✗".red().bold().to_string()
            };
            eprintln!("  {} {} {}", icon, name, "(new check)".bright_black());
        }
        eprintln!();
        if unchanged_pass > 0 || unchanged_fail > 0 {
            eprintln!(
                "  {} {} unchanged passing, {} unchanged failing",
                "◉".bright_black(),
                unchanged_pass,
                unchanged_fail
            );
        }
    }

    let score_before = {
        let (s, g) = health_score(&before.checks);
        format!("{}/100 ({})", s, g)
    };
    let score_after = {
        let (s, g) = health_score(&after.checks);
        format!("{}/100 ({})", s, g)
    };
    eprintln!();
    eprintln!(
        "  {} Health: {} → {}",
        "▶".cyan(),
        score_before.bright_black(),
        score_after.cyan().bold()
    );
    eprintln!();

    if regressions.is_empty() { 0 } else { 1 }
}

// ─── setup_command ──────────────────────────────────────────────────────

pub fn setup_command() {
    let ascii_art = r#"
   ____          _      __  __      _        _
  / ___|___   __| | ___|  \/  | ___| |_ _ __(_) ___ ___
 | |   / _ \ / _` |/ _ \ |\/| |/ _ \ __| '__| |/ __/ __|
 | |__| (_) | (_| |  __/ |  | |  __/ |_| |  | | (__\__ \
  \____\___/ \__,_|\___|_|  |_|\___|\__|_|  |_||\___|___/
"#;
    println!("{}", ascii_art.cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());
    println!("{}", "  CodeMetrics Doctor & Setup".cyan().bold());
    println!("{}\n", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());

    let mut all_passed = true;

    if std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok()
    {
        println!("  {} cargo installed", "[✓]".green().bold());
    } else {
        println!("  {} cargo NOT installed", "[✗]".red().bold());
        println!("      => {}", "Install Rust: https://rustup.rs/".yellow());
        all_passed = false;
    }

    if std::process::Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .output()
        .is_ok()
    {
        println!("  {} cargo-llvm-cov installed", "[✓]".green().bold());
    } else {
        println!("  {} cargo-llvm-cov NOT installed", "[✗]".red().bold());
        println!("      => {}", "Run: cargo install cargo-llvm-cov".yellow());
        all_passed = false;
    }

    let rustup_out = std::process::Command::new("rustup")
        .args(["component", "list"])
        .output()
        .ok();
    if let Some(out) = rustup_out {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("llvm-tools-preview (installed)")
            || stdout.contains("llvm-tools (installed)")
        {
            println!("  {} llvm-tools installed", "[✓]".green().bold());
        } else {
            println!("  {} llvm-tools NOT installed", "[✗]".red().bold());
            println!(
                "      => {}",
                "Run: rustup component add llvm-tools-preview".yellow()
            );
            all_passed = false;
        }
    } else {
        println!(
            "  {} rustup not found, could not verify llvm-tools",
            "[?]".yellow().bold()
        );
    }

    if std::path::Path::new(".quality.toml").exists() {
        println!(
            "  {} .quality.toml configuration found",
            "[✓]".green().bold()
        );
    } else {
        println!("  {} .quality.toml NOT found", "[✗]".red().bold());
        println!("      => {}", "Run: codemetrics init".yellow());
        all_passed = false;
    }

    println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());
    if all_passed {
        println!(
            "  {}",
            "Everything looks good! Your codebase is ready.".green().bold()
        );
    } else {
        println!(
            "  {}",
            "Please resolve the missing requirements above.".red().bold()
        );
    }
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());
}
