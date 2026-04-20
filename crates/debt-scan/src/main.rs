use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Parser)]
#[command(name = "debt", about = "Technical debt scanner -- track TODO/FIXME/HACK/XXX markers")]
struct Cli {
    /// Path to scan (file or directory)
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Only show markers of this type (comma-separated: todo,fixme,hack,xxx)
    #[arg(long)]
    marker: Option<String>,

    /// Sort by: age (default), file, type, author
    #[arg(short, long, default_value = "age")]
    sort: String,
}

#[derive(Debug, Clone, Serialize)]
struct DebtItem {
    file: String,
    line: usize,
    marker_type: String,
    text: String,
    author: Option<String>,
    date: Option<String>,
}

#[derive(Serialize)]
struct DebtReport {
    items: Vec<DebtItem>,
    summary: DebtSummary,
}

#[derive(Serialize)]
struct DebtSummary {
    total: usize,
    todo: usize,
    fixme: usize,
    hack: usize,
    xxx: usize,
    by_author: HashMap<String, usize>,
}

const MARKERS: &[(&str, &str)] = &[
    ("TODO", "todo"),
    ("FIXME", "fixme"),
    ("HACK", "hack"),
    ("XXX", "xxx"),
    ("WARN", "warn"),
    ("BUG", "bug"),
    ("OPTIMIZE", "optimize"),
];

fn main() {
    let cli = Cli::parse();

    let files = find_source_files(&cli.path, cli.recursive);
    if files.is_empty() {
        eprintln!("No source files found at {}", cli.path);
        std::process::exit(1);
    }

    let marker_filter: Option<Vec<String>> = cli.marker.as_ref().map(|m| {
        m.split(',').map(|s| s.trim().to_lowercase()).collect()
    });

    let mut items = Vec::new();

    for file_path in &files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            for (marker_name, marker_type) in MARKERS {
                // Match "TODO:", "TODO(", "FIXME:", "HACK:", etc.
                let patterns = [
                    format!("{}:", marker_name),
                    format!("{}(", marker_name),
                    format!("{} ", marker_name),
                ];

                for pattern in &patterns {
                    if trimmed.contains(pattern) {
                        // Filter out if not requested
                        if let Some(ref filter) = marker_filter {
                            if !filter.contains(&marker_type.to_string()) {
                                continue;
                            }
                        }

                        // Extract the comment text after the marker
                        let text = extract_comment_text(line, marker_name);

                        // Try to get git blame info
                        let (author, date) = get_git_blame(file_path, line_num + 1);

                        items.push(DebtItem {
                            file: file_path.clone(),
                            line: line_num + 1,
                            marker_type: marker_type.to_string(),
                            text,
                            author,
                            date,
                        });
                        break; // Only one marker per line
                    }
                }
            }
        }
    }

    // Sort
    match cli.sort.as_str() {
        "file" => items.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line))),
        "type" => items.sort_by(|a, b| a.marker_type.cmp(&b.marker_type)),
        "author" => items.sort_by(|a, b| a.author.cmp(&b.author)),
        _ => {} // age = git blame date order, already chronological if git available
    }

    match cli.format.as_str() {
        "json" => output_json(&items),
        _ => output_table(&items),
    }
}

fn extract_comment_text(line: &str, marker: &str) -> String {
    // Find the marker and extract text after it
    if let Some(pos) = line.find(marker) {
        let after = &line[pos + marker.len()..];
        // Remove leading :, (, whitespace
        let after = after.trim_start_matches(':').trim_start_matches('(').trim();
        // Remove trailing */
        let after = after.trim_end_matches("*/").trim_end_matches(')').trim();
        after.to_string()
    } else {
        line.trim().to_string()
    }
}

fn get_git_blame(file_path: &str, line: usize) -> (Option<String>, Option<String>) {
    use std::process::Command;

    let output = Command::new("git")
        .args(["blame", "-L", &format!("{},{}", line, line), "--porcelain", file_path])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut author = None;
            let mut date = None;

            for line in text.lines() {
                if let Some(name) = line.strip_prefix("author ") {
                    author = Some(name.to_string());
                }
                if let Some(d) = line.strip_prefix("author-time ") {
                    if let Ok(ts) = d.parse::<i64>() {
                        date = Some(format_timestamp(ts));
                    }
                }
            }

            (author, date)
        }
        _ => (None, None),
    }
}

fn format_timestamp(ts: i64) -> String {
    // Simple date formatting without chrono dependency for the display
    let days = ts / 86400;
    let year = 1970 + days / 365;
    let remaining = days % 365;
    let month = remaining / 30 + 1;
    let day = remaining % 30 + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn find_source_files(path: &str, recursive: bool) -> Vec<String> {
    let path = Path::new(path);
    let mut files = Vec::new();
    let extensions = ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "java", "rb", "php"];

    if path.is_file() {
        files.push(path.to_string_lossy().to_string());
    } else if path.is_dir() {
        scan_dir(path, recursive, &extensions, &mut files);
    }

    files.sort();
    files
}

fn scan_dir(dir: &Path, recursive: bool, extensions: &[&str], files: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if extensions.contains(&ext.to_string_lossy().as_ref()) {
                    files.push(path.to_string_lossy().to_string());
                }
            }
        } else if recursive && path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && name != ".git" && name != "node_modules" && !name.starts_with('.') {
                scan_dir(&path, recursive, extensions, files);
            }
        }
    }
}

fn output_table(items: &[DebtItem]) {
    if items.is_empty() {
        println!("No technical debt markers found. Clean code!");
        return;
    }

    println!(
        "{:<8} {:<50} {:>4} {:<15} {}",
        "TYPE", "FILE", "LINE", "AUTHOR", "TEXT"
    );
    println!("{}", "─".repeat(110));

    let mut todo = 0;
    let mut fixme = 0;
    let mut hack = 0;
    let mut xxx = 0;
    let mut by_author: HashMap<String, usize> = HashMap::new();

    for item in items {
        let icon = match item.marker_type.as_str() {
            "todo" => { todo += 1; "○" }
            "fixme" => { fixme += 1; "⚠" }
            "hack" => { hack += 1; "✗" }
            "xxx" => { xxx += 1; "!" }
            _ => "?"
        };

        let author = item.author.as_deref().unwrap_or("unknown");
        *by_author.entry(author.to_string()).or_insert(0) += 1;

        println!(
            "{} {:<6} {:<50} {:>4} {:<15} {}",
            icon,
            item.marker_type.to_uppercase(),
            truncate(&item.file, 48),
            item.line,
            truncate(author, 13),
            truncate(&item.text, 60),
        );
    }

    println!("{}", "─".repeat(110));
    println!();
    println!("TECHNICAL DEBT SUMMARY");
    println!("  Total markers:  {}", items.len());
    println!("  TODO:           {} (can wait)", todo);
    println!("  FIXME:          {} (should fix)", fixme);
    println!("  HACK:           {} (needs refactor)", hack);
    if xxx > 0 {
        println!("  XXX:            {} (DANGER)", xxx);
    }

    if !by_author.is_empty() {
        println!();
        println!("  By author:");
        let mut authors: Vec<_> = by_author.iter().collect();
        authors.sort_by(|a, b| b.1.cmp(a.1));
        for (author, count) in authors.iter().take(5) {
            println!("    {}: {}", author, count);
        }
    }

    let debt_ratio = (fixme + hack + xxx) as f64 / items.len() as f64 * 100.0;
    println!();
    if debt_ratio > 50.0 {
        println!("  ⚠ {:.0}% of markers are actionable (FIXME/HACK/XXX). High debt.", debt_ratio);
    } else if debt_ratio > 20.0 {
        println!("  ○ {:.0}% of markers are actionable. Moderate debt.", debt_ratio);
    } else {
        println!("  ✓ {:.0}% of markers are actionable. Low debt.", debt_ratio);
    }
}

fn output_json(items: &[DebtItem]) {
    let mut todo = 0;
    let mut fixme = 0;
    let mut hack = 0;
    let mut xxx = 0;
    let mut by_author: HashMap<String, usize> = HashMap::new();

    for item in items {
        match item.marker_type.as_str() {
            "todo" => todo += 1,
            "fixme" => fixme += 1,
            "hack" => hack += 1,
            "xxx" => xxx += 1,
            _ => {}
        }
        let author = item.author.as_deref().unwrap_or("unknown");
        *by_author.entry(author.to_string()).or_insert(0) += 1;
    }

    let report = DebtReport {
        items: items.to_vec(),
        summary: DebtSummary {
            total: items.len(),
            todo,
            fixme,
            hack,
            xxx,
            by_author,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
