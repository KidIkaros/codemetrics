// ═══════════════════════════════════════════
// HEALTH SCORING & SUMMARY DISPLAY
// ═══════════════════════════════════════════

use colored::Colorize;
use crate::types::CheckResult;

/// Strip ANSI escape sequences to measure true visible character width.
pub fn visible_len(s: &str) -> usize {
    let plain = strip_ansi(s);
    plain.chars().count()
}

/// Remove ANSI CSI escape sequences (ESC [ ... m) from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Print a box row padding `content` to `inner_width` visible chars.
pub fn box_row(content: &str, inner_width: usize) {
    let vlen = visible_len(content);
    let padding = if inner_width > vlen {
        inner_width - vlen
    } else {
        0
    };
    eprintln!("  ║  {}{}║", content, " ".repeat(padding));
}

/// Compute a weighted health score 0–100 and letter grade.
/// Security failures penalise harder (×3), compliance (×2), quality (×1).
pub fn health_score(checks: &[CheckResult]) -> (u32, char) {
    let security = [
        "secrets", "vulnscan", "taint", "errhandle", "sast", "crypto",
    ];
    let compliance = ["licenses", "sbom"];
    if checks.is_empty() {
        return (100, 'A');
    }
    let mut weighted_pass = 0u32;
    let mut weighted_total = 0u32;
    for c in checks {
        let w = if security.contains(&c.name.as_str()) {
            3
        } else if compliance.contains(&c.name.as_str()) {
            2
        } else {
            1
        };
        weighted_total += w;
        if c.passed {
            weighted_pass += w;
        }
    }
    let score = if weighted_total == 0 {
        100
    } else {
        weighted_pass * 100 / weighted_total
    };
    let grade = match score {
        90..=100 => 'A',
        80..=89 => 'B',
        65..=79 => 'C',
        50..=64 => 'D',
        _ => 'F',
    };
    (score, grade)
}

/// Extract up to `limit` top offenders from a CheckResult's details JSON.
/// Returns (file, line, description) tuples.
pub fn extract_offenders(
    check: &CheckResult,
    limit: usize,
) -> Vec<(String, Option<u64>, String)> {
    let mut out = Vec::new();
    let arrays = [
        "items", "functions", "findings", "violations", "secrets", "duplicates",
    ];
    for key in &arrays {
        if let Some(arr) = check.details.get(key).and_then(|v| v.as_array()) {
            for item in arr.iter().take(limit) {
                let file = item
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = item.get("line").and_then(|v| v.as_u64());
                let desc = item
                    .get("context")
                    .or_else(|| item.get("kind"))
                    .or_else(|| item.get("name"))
                    .or_else(|| item.get("type"))
                    .or_else(|| item.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !file.is_empty() || !desc.is_empty() {
                    out.push((file, line, desc));
                }
            }
            if !out.is_empty() {
                break;
            }
        }
    }
    out
}

/// Print inline offenders under a check line (used by run_check! for failures).
pub fn print_offenders(check: &CheckResult) {
    let offenders = extract_offenders(check, 5);
    if offenders.is_empty() {
        return;
    }
    for (file, line, desc) in &offenders {
        let loc = match line {
            Some(l) => format!("{}:{}", file, l),
            None if file.is_empty() => String::new(),
            None => file.clone(),
        };
        if loc.is_empty() && desc.is_empty() {
            continue;
        }
        let truncated_desc = if desc.len() > 60 {
            format!("{}…", &desc[..60])
        } else {
            desc.clone()
        };
        if loc.is_empty() {
            eprintln!("      {}", truncated_desc.bright_black());
        } else {
            eprintln!("      {}  {}", loc.cyan(), truncated_desc.bright_black());
        }
    }
    let arrays = [
        "items", "functions", "findings", "violations", "secrets", "duplicates",
    ];
    for key in &arrays {
        if let Some(arr) = check.details.get(key).and_then(|v| v.as_array()) {
            if arr.len() > 5 {
                eprintln!(
                    "      {}",
                    format!("… {} more", arr.len() - 5).bright_black()
                );
            }
            break;
        }
    }
}

pub fn print_summary_box(
    kind: &str,
    passed: bool,
    path: &str,
    passed_count: usize,
    total: usize,
    elapsed: std::time::Duration,
    checks: &[CheckResult],
) {
    use crate::progress::format_elapsed;
    let (score, grade) = health_score(checks);
    let status_plain = if passed { "PASSED ✓" } else { "FAILED ✗" };
    let status = if passed {
        status_plain.green().bold().to_string()
    } else {
        status_plain.red().bold().to_string()
    };
    let grade_col = match grade {
        'A' => grade.to_string().green().bold().to_string(),
        'B' => grade.to_string().cyan().bold().to_string(),
        'C' => grade.to_string().yellow().bold().to_string(),
        _ => grade.to_string().red().bold().to_string(),
    };
    let score_str = format!("Score: {}/100  {}", score, grade_col);
    let checks_str = format!(
        "{}/{} checks passed  ·  {} total",
        passed_count,
        total,
        format_elapsed(elapsed)
    );
    let checks_col = if passed {
        checks_str.green().to_string()
    } else {
        checks_str.red().to_string()
    };
    let inner = 50usize;
    let border = "═".repeat(inner + 2);
    let title = format!("{}  ·  {}", kind, status);
    eprintln!();
    eprintln!("  ╔{}╗", border);
    box_row(&title, inner);
    eprintln!("  ╠{}╣", border);
    box_row(&checks_col, inner);
    box_row(&score_str, inner);
    box_row(&format!("Path: {}", path), inner);
    eprintln!("  ╚{}╝", border);
    eprintln!();
}
