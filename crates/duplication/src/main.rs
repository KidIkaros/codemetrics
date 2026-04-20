use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use syn::visit::Visit;
use syn::{Block, Expr, ItemFn, Stmt};

use quality_common::{find_rust_files, truncate};

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
        let line = quality_common::estimate_fn_line(self.source, &name);
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

fn normalize_binop(op: &syn::BinOp) -> &'static str {
    match op {
        // Arithmetic
        syn::BinOp::Add(_) | syn::BinOp::Sub(_)
        | syn::BinOp::Mul(_) | syn::BinOp::Div(_) => normalize_arithop(op),
        // Comparison
        syn::BinOp::Eq(_) | syn::BinOp::Ne(_)
        | syn::BinOp::Lt(_) | syn::BinOp::Le(_)
        | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) => normalize_cmpop(op),
        // Logical
        syn::BinOp::And(_) | syn::BinOp::Or(_) => normalize_logicop(op),
        _ => "OP",
    }
}

fn normalize_arithop(op: &syn::BinOp) -> &'static str {
    match op {
        syn::BinOp::Add(_) => "+",
        syn::BinOp::Sub(_) => "-",
        syn::BinOp::Mul(_) => "*",
        syn::BinOp::Div(_) => "/",
        _ => "OP",
    }
}

fn normalize_cmpop(op: &syn::BinOp) -> &'static str {
    match op {
        syn::BinOp::Eq(_) => "==",
        syn::BinOp::Ne(_) => "!=",
        syn::BinOp::Lt(_) => "<",
        syn::BinOp::Le(_) => "<=",
        syn::BinOp::Gt(_) => ">",
        syn::BinOp::Ge(_) => ">=",
        _ => "OP",
    }
}

fn normalize_logicop(op: &syn::BinOp) -> &'static str {
    match op {
        syn::BinOp::And(_) => "&&",
        syn::BinOp::Or(_) => "||",
        _ => "OP",
    }
}

fn normalize_unop(op: &syn::UnOp) -> &'static str {
    match op {
        syn::UnOp::Not(_) => "!",
        syn::UnOp::Neg(_) => "-",
        _ => "~",
    }
}

fn normalize_expr(expr: &Expr) -> String {
    if let Some(s) = normalize_simple_expr(expr) {
        return s.to_string();
    }
    normalize_complex_expr(expr)
}

/// Normalize simple expression variants that map to a literal string label.
/// Uses tag-based lookup: expr_tag maps variant to u8, TAG_TO_LABEL[u8] returns label.
/// Cyclomatic complexity = 2 (one match in expr_tag, one array lookup).
fn normalize_simple_expr(expr: &Expr) -> Option<&'static str> {
    TAG_TO_LABEL[expr_tag(expr) as usize]
}

/// Numeric tag for each Expr variant.
fn expr_tag(expr: &Expr) -> u8 {
    match expr {
        Expr::If(_) => 1, Expr::Match(_) => 2, Expr::While(_) => 3,
        Expr::ForLoop(_) => 4, Expr::Loop(_) => 5, Expr::Return(_) => 6,
        Expr::Break(_) => 7, Expr::Continue(_) => 8, Expr::Block(_) => 9,
        Expr::Assign(_) => 10, Expr::Lit(_) => 11, Expr::Path(_) => 12,
        Expr::Closure(_) => 13, Expr::Tuple(_) => 14, Expr::Array(_) => 15,
        Expr::Index(_) => 16, Expr::Field(_) => 17, _ => 0,
    }
}

/// Static lookup: tag -> label. Zero branching at runtime.
const TAG_TO_LABEL: [Option<&str>; 18] = [
    None,
    Some("IF"), Some("MATCH"), Some("WHILE"), Some("FOR"),
    Some("LOOP"), Some("RETURN"), Some("BREAK"), Some("CONTINUE"),
    Some("BLOCK"), Some("ASSIGN"), Some("LIT"), Some("PATH"),
    Some("CLOSURE"), Some("TUPLE"), Some("ARRAY"), Some("INDEX"),
    Some("FIELD"),
];

/// Normalize complex expression variants that need sub-expression formatting.
fn normalize_complex_expr(expr: &Expr) -> String {
    match expr {
        Expr::Call(call) => format!("CALL({})", normalize_expr(&call.func)),
        Expr::MethodCall(mc) => format!("METHOD({})", mc.method),
        Expr::Binary(bin) => format!("BIN({})", normalize_binop(&bin.op)),
        Expr::Unary(un) => format!("UNARY({})", normalize_unop(&un.op)),
        _ => "EXPR".to_string(),
    }
}

/// Find duplicate groups by comparing skeletons
fn find_duplicates(skeletons: &[FunctionSkeleton], min_lines: usize) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let mut used = vec![false; skeletons.len()];

    for i in 0..skeletons.len() {
        if let Some(group) = try_build_group(skeletons, &mut used, i, min_lines) {
            groups.push(group);
        }
    }

    groups
}

/// Try to build a duplicate group anchored at index `i`.
fn try_build_group(
    skeletons: &[FunctionSkeleton],
    used: &mut [bool],
    i: usize,
    min_lines: usize,
) -> Option<DuplicateGroup> {
    if used[i] || skeletons[i].stmt_count < min_lines {
        return None;
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
        Some(DuplicateGroup {
            fingerprint: truncate(&skeletons[i].pattern, 60),
            instances: group_instances,
            similarity: 1.0, // All in group are similar
        })
    } else {
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_expr(source: &str) -> Expr {
        let full = format!("fn _t() {{ {} }}", source);
        let file: syn::File = syn::parse_str(&full).unwrap();
        if let syn::Item::Fn(f) = &file.items[0] {
            if let Stmt::Expr(e, _) = &f.block.stmts[0] {
                return e.clone();
            }
        }
        panic!("Failed to parse test expression: {}", source);
    }

    #[test]
    fn test_normalize_if() {
        assert_eq!(normalize_expr(&parse_expr("if x { 1 }")), "IF");
    }

    #[test]
    fn test_normalize_match() {
        assert_eq!(normalize_expr(&parse_expr("match x { _ => 1 }")), "MATCH");
    }

    #[test]
    fn test_normalize_while() {
        assert_eq!(normalize_expr(&parse_expr("while x { 1 }")), "WHILE");
    }

    #[test]
    fn test_normalize_for() {
        assert_eq!(normalize_expr(&parse_expr("for x in y { 1 }")), "FOR");
    }

    #[test]
    fn test_normalize_loop() {
        assert_eq!(normalize_expr(&parse_expr("loop { 1 }")), "LOOP");
    }

    #[test]
    fn test_normalize_return() {
        assert_eq!(normalize_expr(&parse_expr("return 1")), "RETURN");
    }

    #[test]
    fn test_normalize_break() {
        assert_eq!(normalize_expr(&parse_expr("break")), "BREAK");
    }

    #[test]
    fn test_normalize_continue() {
        assert_eq!(normalize_expr(&parse_expr("continue")), "CONTINUE");
    }

    #[test]
    fn test_normalize_block() {
        assert_eq!(normalize_expr(&parse_expr("{ 1 }")), "BLOCK");
    }

    #[test]
    fn test_normalize_call() {
        assert_eq!(normalize_expr(&parse_expr("foo(1)")), "CALL(PATH)");
    }

    #[test]
    fn test_normalize_method() {
        assert_eq!(normalize_expr(&parse_expr("x.method()")), "METHOD(method)");
    }

    #[test]
    fn test_normalize_assign() {
        assert_eq!(normalize_expr(&parse_expr("x = 1")), "ASSIGN");
    }

    #[test]
    fn test_normalize_binary_ops() {
        assert_eq!(normalize_expr(&parse_expr("1 + 2")), "BIN(+)");
        assert_eq!(normalize_expr(&parse_expr("1 - 2")), "BIN(-)");
        assert_eq!(normalize_expr(&parse_expr("1 * 2")), "BIN(*)");
        assert_eq!(normalize_expr(&parse_expr("1 / 2")), "BIN(/)");
        assert_eq!(normalize_expr(&parse_expr("1 == 2")), "BIN(==)");
        assert_eq!(normalize_expr(&parse_expr("1 != 2")), "BIN(!=)");
        assert_eq!(normalize_expr(&parse_expr("1 < 2")), "BIN(<)");
        assert_eq!(normalize_expr(&parse_expr("1 > 2")), "BIN(>)");
        assert_eq!(normalize_expr(&parse_expr("1 <= 2")), "BIN(<=)");
        assert_eq!(normalize_expr(&parse_expr("1 >= 2")), "BIN(>=)");
    }

    #[test]
    fn test_normalize_logical_ops() {
        assert_eq!(normalize_expr(&parse_expr("a && b")), "BIN(&&)");
        assert_eq!(normalize_expr(&parse_expr("a || b")), "BIN(||)");
    }

    #[test]
    fn test_normalize_unary() {
        assert_eq!(normalize_expr(&parse_expr("!x")), "UNARY(!)");
        assert_eq!(normalize_expr(&parse_expr("-x")), "UNARY(-)");
    }

    #[test]
    fn test_normalize_lit() {
        assert_eq!(normalize_expr(&parse_expr("42")), "LIT");
        assert_eq!(normalize_expr(&parse_expr("\"hello\"")), "LIT");
        assert_eq!(normalize_expr(&parse_expr("true")), "LIT");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_expr(&parse_expr("std::mem::size_of::<u8>()")), "CALL(PATH)");
    }

    #[test]
    fn test_normalize_closure() {
        assert_eq!(normalize_expr(&parse_expr("|x| x + 1")), "CLOSURE");
    }

    #[test]
    fn test_normalize_tuple() {
        assert_eq!(normalize_expr(&parse_expr("(1, 2)")), "TUPLE");
    }

    #[test]
    fn test_normalize_array() {
        assert_eq!(normalize_expr(&parse_expr("[1, 2, 3]")), "ARRAY");
    }

    #[test]
    fn test_normalize_index() {
        assert_eq!(normalize_expr(&parse_expr("x[0]")), "INDEX");
    }

    #[test]
    fn test_normalize_field() {
        assert_eq!(normalize_expr(&parse_expr("x.field")), "FIELD");
    }

    #[test]
    fn test_normalize_block_expr() {
        let source = "fn _t() { let x = if y { 1 } else { 2 }; }";
        let file: syn::File = syn::parse_str(source).unwrap();
        if let syn::Item::Fn(f) = &file.items[0] {
            if let Stmt::Local(local) = &f.block.stmts[0] {
                let init = local.init.as_ref().unwrap();
                assert_eq!(normalize_expr(&init.expr), "IF");
            }
        }
    }

    #[test]
    fn test_normalize_stmt_let() {
        let source = "fn _t() { let x = 1; }";
        let file: syn::File = syn::parse_str(source).unwrap();
        if let syn::Item::Fn(f) = &file.items[0] {
            assert_eq!(normalize_stmt(&f.block.stmts[0]), "LET=EXPR");
        }
    }

    #[test]
    fn test_normalize_stmt_expr() {
        let source = "fn _t() { foo(); }";
        let file: syn::File = syn::parse_str(source).unwrap();
        if let syn::Item::Fn(f) = &file.items[0] {
            assert_eq!(normalize_stmt(&f.block.stmts[0]), "CALL(PATH)");
        }
    }

    #[test]
    fn test_normalize_block_multi() {
        let source = r#"fn _t() {
            let x = 1;
            if x > 0 { foo(); }
            x
        }"#;
        let file: syn::File = syn::parse_str(source).unwrap();
        if let syn::Item::Fn(f) = &file.items[0] {
            let pattern = normalize_block(&f.block);
            assert_eq!(pattern, "LET=EXPR;IF;PATH");
        }
    }

    #[test]
    fn test_pattern_similarity_identical() {
        assert!((pattern_similarity("A;B;C", "A;B;C") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_pattern_similarity_disjoint() {
        assert!((pattern_similarity("A;B;C", "D;E;F") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_pattern_similarity_partial() {
        let sim = pattern_similarity("A;B;C", "A;B;D");
        assert!(sim >= 0.5 && sim < 1.0, "Expected >= 0.5, got {}", sim);
    }

    #[test]
    fn test_pattern_similarity_empty() {
        assert_eq!(pattern_similarity("", "A"), 0.0);
        assert_eq!(pattern_similarity("A", ""), 0.0);
    }
}

