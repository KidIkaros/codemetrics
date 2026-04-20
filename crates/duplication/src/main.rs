use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use syn::visit::Visit;
use syn::{Block, Expr, ItemFn, Stmt};

#[derive(Parser)]
#[command(name = "dupfind", about = "Code duplication detection -- find copy-pasted blocks via structural similarity")]
struct Cli {
    /// Path to scan (file or directory)
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Minimum block size (lines) to consider
    #[arg(short, long, default_value = "5")]
    min_lines: usize,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,
}

#[derive(Debug, Clone, Serialize)]
struct DuplicateGroup {
    fingerprint: String,
    instances: Vec<DuplicateInstance>,
    similarity: f64,
}

#[derive(Debug, Clone, Serialize)]
struct DuplicateInstance {
    file: String,
    function: String,
    line: usize,
}

#[derive(Serialize)]
struct DupReport {
    groups: Vec<DuplicateGroup>,
    summary: DupSummary,
}

#[derive(Serialize)]
struct DupSummary {
    total_groups: usize,
    total_instances: usize,
    files_affected: usize,
}

/// A normalized function skeleton for comparison
#[derive(Debug, Clone)]
struct FunctionSkeleton {
    name: String,
    file: String,
    line: usize,
    /// Normalized statement pattern (structure without identifiers)
    pattern: String,
    /// Statement count
    stmt_count: usize,
}

fn main() {
    let cli = Cli::parse();

    let files = find_rust_files(&cli.path, cli.recursive);
    if files.is_empty() {
        eprintln!("No .rs files found at {}", cli.path);
        std::process::exit(1);
    }

    // Extract function skeletons
    let mut skeletons = Vec::new();

    for file_path in &files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match syn::parse_file(&source) {
            Ok(ast) => {
                let mut visitor = SkeletonVisitor {
                    file: file_path.clone(),
                    source: &source,
                    skeletons: Vec::new(),
                };
                visitor.visit_file(&ast);
                skeletons.extend(visitor.skeletons);
            }
            Err(e) => eprintln!("Warning: parse error in {}: {}", file_path, e),
        }
    }

    // Group by pattern similarity
    let groups = find_duplicates(&skeletons, cli.min_lines);

    match cli.format.as_str() {
        "json" => output_json(&groups),
        _ => output_table(&groups),
    }
}

struct SkeletonVisitor<'a> {
    file: String,
    source: &'a str,
    skeletons: Vec<FunctionSkeleton>,
}

impl<'a> Visit<'a> for SkeletonVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        let name = node.sig.ident.to_string();
        let line = estimate_line(self.source, &name);
        let pattern = normalize_block(&node.block);
        let stmt_count = node.block.stmts.len();

        self.skeletons.push(FunctionSkeleton {
            name,
            file: self.file.clone(),
            line,
            pattern,
            stmt_count,
        });

        // Don't recurse into nested functions
    }
}

/// Normalize a block to a structural pattern
fn normalize_block(block: &Block) -> String {
    let mut pattern = Vec::new();
    for stmt in &block.stmts {
        pattern.push(normalize_stmt(stmt));
    }
    pattern.join(";")
}

fn normalize_stmt(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Local(local) => {
            let mut s = "LET".to_string();
            if local.init.is_some() {
                s.push_str("=EXPR");
            }
            s
        }
        Stmt::Item(item) => normalize_item(item),
        Stmt::Expr(expr, _) => normalize_expr(expr),
        Stmt::Macro(_) => "MACRO".to_string(),
    }
}

fn normalize_item(item: &syn::Item) -> String {
    match item {
        syn::Item::Fn(_) => "FN".to_string(),
        syn::Item::Struct(_) => "STRUCT".to_string(),
        _ => "ITEM".to_string(),
    }
}

fn normalize_expr(expr: &Expr) -> String {
    match expr {
        Expr::If(_) => "IF".to_string(),
        Expr::Match(_) => "MATCH".to_string(),
        Expr::While(_) => "WHILE".to_string(),
        Expr::ForLoop(_) => "FOR".to_string(),
        Expr::Loop(_) => "LOOP".to_string(),
        Expr::Return(_) => "RETURN".to_string(),
        Expr::Break(_) => "BREAK".to_string(),
        Expr::Continue(_) => "CONTINUE".to_string(),
        Expr::Block(_) => "BLOCK".to_string(),
        Expr::Call(call) => {
            let func = normalize_expr(&call.func);
            format!("CALL({})", func)
        }
        Expr::MethodCall(mc) => {
            format!("METHOD({})", mc.method)
        }
        Expr::Assign(_) => "ASSIGN".to_string(),
        Expr::Binary(bin) => {
            let op = match &bin.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                syn::BinOp::Div(_) => "/",
                syn::BinOp::And(_) => "&&",
                syn::BinOp::Or(_) => "||",
                syn::BinOp::Eq(_) => "==",
                syn::BinOp::Ne(_) => "!=",
                syn::BinOp::Lt(_) => "<",
                syn::BinOp::Le(_) => "<=",
                syn::BinOp::Gt(_) => ">",
                syn::BinOp::Ge(_) => ">=",
                _ => "OP",
            };
            format!("BIN({})", op)
        }
        Expr::Unary(un) => {
            let op = match &un.op {
                syn::UnOp::Not(_) => "!",
                syn::UnOp::Neg(_) => "-",
                _ => "~",
            };
            format!("UNARY({})", op)
        }
        Expr::Lit(_) => "LIT".to_string(),
        Expr::Path(_) => "PATH".to_string(),
        Expr::Closure(_) => "CLOSURE".to_string(),
        Expr::Tuple(_) => "TUPLE".to_string(),
        Expr::Array(_) => "ARRAY".to_string(),
        Expr::Index(_) => "INDEX".to_string(),
        Expr::Field(_) => "FIELD".to_string(),
        _ => "EXPR".to_string(),
    }
}

/// Find duplicate groups by comparing skeletons
fn find_duplicates(skeletons: &[FunctionSkeleton], min_lines: usize) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let mut used = vec![false; skeletons.len()];

    for i in 0..skeletons.len() {
        if used[i] || skeletons[i].stmt_count < min_lines {
            continue;
        }

        let mut group_instances = vec![DuplicateInstance {
            file: skeletons[i].file.clone(),
            function: skeletons[i].name.clone(),
            line: skeletons[i].line,
        }];

        for j in (i + 1)..skeletons.len() {
            if used[j] || skeletons[j].stmt_count < min_lines {
                continue;
            }

            let similarity = pattern_similarity(&skeletons[i].pattern, &skeletons[j].pattern);
            if similarity >= 0.7 {
                group_instances.push(DuplicateInstance {
                    file: skeletons[j].file.clone(),
                    function: skeletons[j].name.clone(),
                    line: skeletons[j].line,
                });
                used[j] = true;
            }
        }

        if group_instances.len() > 1 {
            used[i] = true;
            groups.push(DuplicateGroup {
                fingerprint: truncate(&skeletons[i].pattern, 60),
                instances: group_instances,
                similarity: 1.0, // All in group are similar
            });
        }
    }

    groups
}

/// Calculate similarity between two patterns (0.0 to 1.0)
fn pattern_similarity(a: &str, b: &str) -> f64 {
    let tokens_a: Vec<&str> = a.split(';').collect();
    let tokens_b: Vec<&str> = b.split(';').collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    // Count matching tokens in order (longest common subsequence ratio)
    let max_len = tokens_a.len().max(tokens_b.len());
    let mut matches = 0;

    // Simple approach: count tokens that appear in both
    let set_a: std::collections::HashSet<&str> = tokens_a.iter().cloned().collect();
    let set_b: std::collections::HashSet<&str> = tokens_b.iter().cloned().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        return 0.0;
    }

    // Jaccard similarity on token sets
    intersection as f64 / union as f64
}

fn estimate_line(source: &str, fn_name: &str) -> usize {
    let pattern = format!("fn {}", fn_name);
    for (i, line) in source.lines().enumerate() {
        if line.contains(&pattern) {
            return i + 1;
        }
    }
    1
}

fn find_rust_files(path: &str, recursive: bool) -> Vec<String> {
    let path = Path::new(path);
    let mut files = Vec::new();

    if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
        files.push(path.to_string_lossy().to_string());
    } else if path.is_dir() {
        scan_dir(path, recursive, &mut files);
    }

    files.sort();
    files
}

fn scan_dir(dir: &Path, recursive: bool, files: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            files.push(path.to_string_lossy().to_string());
        } else if recursive && path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && name != ".git" && !name.starts_with('.') {
                scan_dir(&path, recursive, files);
            }
        }
    }
}

fn output_table(groups: &[DuplicateGroup]) {
    if groups.is_empty() {
        println!("No code duplication found. Clean code!");
        return;
    }

    let total_instances: usize = groups.iter().map(|g| g.instances.len()).sum();
    let files: std::collections::HashSet<&str> = groups
        .iter()
        .flat_map(|g| g.instances.iter().map(|i| i.file.as_str()))
        .collect();

    println!("CODE DUPLICATION");
    println!("{}", "─".repeat(70));
    println!();

    for (i, group) in groups.iter().enumerate() {
        println!("  Group {} ({} instances):", i + 1, group.instances.len());
        println!("    Pattern: {}", group.fingerprint);
        for inst in &group.instances {
            println!("      - {} ({}:{})", inst.function, inst.file, inst.line);
        }
        println!();
    }

    println!("{}", "─".repeat(70));
    println!("  Duplicate groups:    {}", groups.len());
    println!("  Total instances:     {}", total_instances);
    println!("  Files affected:      {}", files.len());

    let dup_ratio = total_instances as f64 / (total_instances + files.len()) as f64 * 100.0;
    if dup_ratio > 20.0 {
        println!();
        println!("  ⚠ Significant duplication detected. Consider refactoring.");
    }
}

fn output_json(groups: &[DuplicateGroup]) {
    let total_instances: usize = groups.iter().map(|g| g.instances.len()).sum();
    let files: std::collections::HashSet<&str> = groups
        .iter()
        .flat_map(|g| g.instances.iter().map(|i| i.file.as_str()))
        .collect();

    let report = DupReport {
        groups: groups.to_vec(),
        summary: DupSummary {
            total_groups: groups.len(),
            total_instances,
            files_affected: files.len(),
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
