/// Universal AST layer backed by tree-sitter.
/// Supports Rust, Python, JavaScript, TypeScript, Go.
use std::cell::RefCell;
use tree_sitter::{Language as TsLanguage, Node, Parser};

// ═══════════════════════════════════════════
// LANGUAGE ENUM
// ═══════════════════════════════════════════

/// Supported source languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Unknown,
}

impl Language {
    /// Detect language from file extension.
    pub fn from_extension(path: &str) -> Self {
        let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
        match ext.as_str() {
            "rs" => Language::Rust,
            "py" | "pyi" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" | "mts" => Language::TypeScript,
            "go" => Language::Go,
            _ => Language::Unknown,
        }
    }

    fn ts_language(self) -> Option<TsLanguage> {
        match self {
            Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
            Language::Unknown => None,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

// ═══════════════════════════════════════════
// PUBLIC DATA TYPES
// ═══════════════════════════════════════════

/// A parsed function with its complexity.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub complexity: u32,
    pub language: Language,
}

/// Documentation coverage statistics for one file.
#[derive(Debug, Clone, Default)]
pub struct DocStats {
    /// Total public functions/methods/classes found.
    pub total_public: usize,
    /// How many of those have a doc comment.
    pub documented: usize,
}

impl DocStats {
    pub fn coverage_pct(&self) -> f64 {
        if self.total_public == 0 {
            100.0
        } else {
            self.documented as f64 / self.total_public as f64 * 100.0
        }
    }
}

/// A structural block fingerprint used for duplication detection.
#[derive(Debug, Clone)]
pub struct BlockFingerprint {
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub fingerprint: String,
}

/// An import/dependency found in a source file.
#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub source_module: String,
    pub imported_module: String,
    pub line: usize,
}

// ═══════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════

// ═══════════════════════════════════════════
// THREAD-LOCAL PARSER POOL
// ═══════════════════════════════════════════

// Per-thread parser cache: one Parser per Language, stored as Option for take().
// `Parser` is !Send, so we store it in thread_local storage.
thread_local! {
    static PARSER_POOL: RefCell<std::collections::HashMap<Language, Option<Parser>>> = RefCell::new(std::collections::HashMap::new());
}

fn init_parser_for_lang(lang: Language) -> Option<Parser> {
    let ts_lang = lang.ts_language()?;
    let mut p = Parser::new();
    p.set_language(&ts_lang).ok()?;
    Some(p)
}

/// Checkout a parser for `lang` from the thread-local pool, creating one if needed.
/// The parser is returned to the pool via the closure so it can be reused.
pub fn with_pooled_parser<T>(lang: Language, f: impl FnOnce(&mut Parser) -> T) -> T {
    PARSER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        let entry = pool.entry(lang).or_insert_with(|| init_parser_for_lang(lang));
        if entry.is_none() {
            *entry = init_parser_for_lang(lang);
        }
        let mut owned = entry.take();
        let result = if let Some(ref mut p) = owned {
            f(p)
        } else {
            // Fallback: create transient parser (shouldn't happen after first success)
            let mut p = Parser::new();
            f(&mut p)
        };
        // Return parser to pool
        *pool.get_mut(&lang).unwrap() = owned;
        result
    })
}

fn node_text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn node_start_line(node: Node<'_>) -> usize {
    node.start_position().row + 1
}

fn node_end_line(node: Node<'_>) -> usize {
    node.end_position().row + 1
}

// ═══════════════════════════════════════════
// COMPLEXITY
// ═══════════════════════════════════════════

/// Complexity-branching node kinds per language.
fn complexity_branch_kinds(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &[
            "if_expression",
            "else_clause",
            "match_arm",
            "while_expression",
            "loop_expression",
            "for_expression",
            "closure_expression",
        ],
        Language::Python => &[
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "with_statement",
            "try_statement",
            "except_clause",
            "lambda",
        ],
        Language::JavaScript | Language::TypeScript => &[
            "if_statement",
            "else_clause",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_case",
            "catch_clause",
            "ternary_expression",
            "arrow_function",
        ],
        Language::Go => &[
            "if_statement",
            "else_clause",
            "for_statement",
            "switch_statement",
            "case_clause",
            "type_switch_statement",
            "select_statement",
            "communication_case",
        ],
        Language::Unknown => &[],
    }
}

/// Function/method node kinds per language.
fn function_node_kinds(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &["function_item", "impl_item"],
        Language::Python => &["function_definition", "decorated_definition"],
        Language::JavaScript => &[
            "function_declaration",
            "function",
            "arrow_function",
            "method_definition",
        ],
        Language::TypeScript => &[
            "function_declaration",
            "function",
            "arrow_function",
            "method_definition",
            "method_signature",
        ],
        Language::Go => &["function_declaration", "method_declaration"],
        Language::Unknown => &[],
    }
}

/// Name-extracting child field per language.
fn function_name_field(lang: Language) -> &'static str {
    match lang {
        Language::Rust | Language::Python | Language::JavaScript | Language::TypeScript => "name",
        Language::Go => "name",
        Language::Unknown => "name",
    }
}

/// Count branches within a subtree (recursive).
fn count_branches(node: Node<'_>, branch_kinds: &[&str]) -> u32 {
    let mut count = 0u32;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if branch_kinds.contains(&child.kind()) {
            count += 1;
        }
        count += count_branches(child, branch_kinds);
    }
    count
}

/// Collect all function nodes from a tree, recursively.
fn collect_functions<'a>(node: Node<'a>, func_kinds: &[&str], out: &mut Vec<Node<'a>>) {
    if func_kinds.contains(&node.kind()) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, func_kinds, out);
    }
}

/// Parse a source string and return function complexity for all functions.
pub fn parse_complexity(source: &str, file: &str, lang: Language) -> Vec<FunctionInfo> {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let source_bytes = source.as_bytes();
    let func_kinds = function_node_kinds(lang);
    let branch_kinds = complexity_branch_kinds(lang);
    let name_field = function_name_field(lang);

    let mut functions = Vec::new();
    let mut func_nodes = Vec::new();
    collect_functions(tree.root_node(), func_kinds, &mut func_nodes);

    for func_node in func_nodes {
        // For Rust impl_item, skip — we only want nested fn items inside
        if lang == Language::Rust && func_node.kind() == "impl_item" {
            continue;
        }

        let name = func_node
            .child_by_field_name(name_field)
            .map(|n| node_text(n, source_bytes).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

        let complexity = 1 + count_branches(func_node, branch_kinds);
        let line = node_start_line(func_node);
        let end_line = node_end_line(func_node);

        functions.push(FunctionInfo {
            name,
            file: file.to_string(),
            line,
            end_line,
            complexity,
            language: lang,
        });
    }

    functions
    })
}

/// Parse a source file on disk and return complexity.
pub fn parse_complexity_file(path: &str) -> Vec<FunctionInfo> {
    let lang = Language::from_extension(path);
    if lang == Language::Unknown {
        return vec![];
    }
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    parse_complexity(&source, path, lang)
}

// ═══════════════════════════════════════════
// DOC COVERAGE
// ═══════════════════════════════════════════

/// Detect whether a function/class node at `line` has a doc comment in `source`.
fn has_doc_comment_before(source: &str, line: usize, lang: Language) -> bool {
    if line == 0 {
        return false;
    }
    let lines: Vec<&str> = source.lines().collect();
    // Look at up to 3 lines above the node start
    let start = if line >= 3 { line - 3 } else { 0 };
    for prev in (start..line - 1).rev() {
        let trimmed = lines.get(prev).map(|l| l.trim()).unwrap_or("");
        if trimmed.is_empty() {
            continue;
        }
        let is_doc = match lang {
            Language::Rust => trimmed.starts_with("///") || trimmed.starts_with("/**") || trimmed.starts_with("//!"),
            Language::Python => {
                // Python docstrings appear as the first statement inside the function body —
                // handled separately via AST; here we check for a comment above.
                trimmed.starts_with('#')
            }
            Language::JavaScript | Language::TypeScript => {
                trimmed.starts_with("/**") || trimmed.starts_with("* ") || trimmed.starts_with("*/")
                    || trimmed.starts_with("//")
            }
            Language::Go => trimmed.starts_with("//"),
            Language::Unknown => false,
        };
        return is_doc;
    }
    false
}

/// Check if a Python function has a docstring as its first body statement.
fn python_fn_has_docstring(func_node: Node<'_>, _source_bytes: &[u8]) -> bool {
    // body is a block; first statement should be expression_statement containing string
    let body = func_node.child_by_field_name("body");
    if let Some(body) = body {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                let mut c2 = child.walk();
                for sub in child.children(&mut c2) {
                    if sub.kind() == "string" {
                        return true;
                    }
                }
            }
            // Only look at the first non-trivial statement
            if child.kind() != "comment" && !child.kind().contains("newline") && !child.is_extra() {
                break;
            }
        }
    }
    false
}

/// Node kinds that represent documentable public items.
fn public_item_kinds(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &["function_item", "struct_item", "enum_item", "trait_item"],
        Language::Python => &["function_definition", "class_definition"],
        Language::JavaScript | Language::TypeScript => &[
            "function_declaration",
            "class_declaration",
            "method_definition",
            "export_statement",
        ],
        Language::Go => &["function_declaration", "method_declaration", "type_declaration"],
        Language::Unknown => &[],
    }
}

/// Whether a Rust item is `pub`.
fn rust_is_public(node: Node<'_>, source_bytes: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = node_text(child, source_bytes);
            return text.starts_with("pub");
        }
    }
    false
}

fn collect_public_items<'a>(node: Node<'a>, lang: Language, source_bytes: &[u8], out: &mut Vec<Node<'a>>) {
    let kinds = public_item_kinds(lang);
    if kinds.contains(&node.kind()) {
        let include = match lang {
            Language::Rust => rust_is_public(node, source_bytes),
            // Python/JS/Go: all top-level functions are considered public
            _ => node.parent().map_or(true, |p| {
                p.kind() == "module"
                    || p.kind() == "program"
                    || p.kind() == "source_file"
                    || p.kind() == "block"
            }),
        };
        if include {
            out.push(node);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_public_items(child, lang, source_bytes, out);
    }
}

/// Parse doc coverage from source string.
pub fn parse_doc_coverage(source: &str, lang: Language) -> DocStats {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return DocStats::default(),
    };

    let source_bytes = source.as_bytes();
    let mut items = Vec::new();
    collect_public_items(tree.root_node(), lang, source_bytes, &mut items);

    let mut stats = DocStats::default();
    for item in items {
        stats.total_public += 1;
        let line = node_start_line(item);
        let documented = match lang {
            Language::Python => {
                python_fn_has_docstring(item, source_bytes)
                    || has_doc_comment_before(source, line, lang)
            }
            _ => has_doc_comment_before(source, line, lang),
        };
        if documented {
            stats.documented += 1;
        }
    }
    stats
    })
}

/// Parse doc coverage from a file on disk.
pub fn parse_doc_coverage_file(path: &str) -> DocStats {
    let lang = Language::from_extension(path);
    if lang == Language::Unknown {
        return DocStats::default();
    }
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return DocStats::default(),
    };
    parse_doc_coverage(&source, lang)
}

// ═══════════════════════════════════════════
// STRUCTURAL FINGERPRINTING (for duplication)
// ═══════════════════════════════════════════

/// Normalize a node kind to a language-independent token.
fn normalize_kind(kind: &str) -> Option<&'static str> {
    static MAP: &[(&[&str], &str)] = &[
        (&["if_expression", "if_statement"], "IF"),
        (&["else_clause", "elif_clause", "else"], "ELSE"),
        (&["while_expression", "while_statement"], "WHILE"),
        (&["for_expression", "for_statement", "for_in_statement"], "FOR"),
        (&["loop_expression"], "LOOP"),
        (&["match_expression", "switch_statement", "switch_case"], "MATCH"),
        (&["match_arm", "case_clause", "default_case"], "ARM"),
        (&["let_declaration", "variable_declaration", "short_var_declaration", "assignment"], "LET"),
        (&["return_expression", "return_statement"], "RET"),
        (&["call_expression", "function_call", "method_call", "call"], "CALL"),
        (&["closure_expression", "arrow_function", "lambda"], "LAMBDA"),
        (&["block", "statement_block", "body"], "BLOCK"),
        (&["try_expression", "try_statement", "except_clause", "catch_clause"], "TRY"),
    ];
    MAP.iter()
        .find(|(kinds, _)| kinds.contains(&kind))
        .map(|(_, token)| *token)
}

/// Build a fingerprint string from a function body node by walking its statements.
fn fingerprint_node(node: Node<'_>) -> String {
    let mut tokens = Vec::new();
    fingerprint_recurse(node, &mut tokens, 0);
    tokens.join(";")
}

fn fingerprint_recurse(node: Node<'_>, tokens: &mut Vec<&'static str>, depth: u32) {
    if depth > 20 {
        return;
    }
    if let Some(tok) = normalize_kind(node.kind()) {
        tokens.push(tok);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        fingerprint_recurse(child, tokens, depth + 1);
    }
}

/// Extract structural fingerprints of all functions in a source file.
pub fn parse_fingerprints(source: &str, file: &str, lang: Language) -> Vec<BlockFingerprint> {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let func_kinds = function_node_kinds(lang);
    let mut func_nodes = Vec::new();
    collect_functions(tree.root_node(), func_kinds, &mut func_nodes);

    func_nodes
        .into_iter()
        .filter(|n| !(lang == Language::Rust && n.kind() == "impl_item"))
        .map(|n| BlockFingerprint {
            file: file.to_string(),
            line: node_start_line(n),
            end_line: node_end_line(n),
            fingerprint: fingerprint_node(n),
        })
        .collect()
    })
}

/// Parse fingerprints from a file on disk.
pub fn parse_fingerprints_file(path: &str) -> Vec<BlockFingerprint> {
    let lang = Language::from_extension(path);
    if lang == Language::Unknown {
        return vec![];
    }
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    parse_fingerprints(&source, path, lang)
}

// ═══════════════════════════════════════════
// IDENTIFIERS & STRING LITERALS (for taint)
// ═══════════════════════════════════════════

/// Extract all identifier names from source (for taint variable detection).
pub fn parse_identifiers(source: &str, lang: Language) -> Vec<String> {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let source_bytes = source.as_bytes();
    let mut ids = Vec::new();
    collect_node_kind(tree.root_node(), "identifier", source_bytes, &mut ids);
    ids.dedup();
    ids
    })
}

/// Extract all string literal values from source (for taint sink detection).
pub fn parse_string_literals(source: &str, lang: Language) -> Vec<String> {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };
    let source_bytes = source.as_bytes();
    let mut strings = Vec::new();
    for kind in &["string_literal", "string", "interpreted_string_literal", "raw_string_literal"] {
        collect_node_kind(tree.root_node(), kind, source_bytes, &mut strings);
    }
    strings
    })
}

fn collect_node_kind(node: Node<'_>, kind: &str, source: &[u8], out: &mut Vec<String>) {
    if node.kind() == kind {
        out.push(node_text(node, source).to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_kind(child, kind, source, out);
    }
}

// ═══════════════════════════════════════════
// IMPORT EXTRACTION (for coupling)
// ═══════════════════════════════════════════

/// Import/dependency kinds per language.
fn import_node_kinds(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &["use_declaration", "extern_crate_declaration"],
        Language::Python => &["import_statement", "import_from_statement"],
        Language::JavaScript | Language::TypeScript => &["import_statement", "call_expression"],
        Language::Go => &["import_declaration", "import_spec"],
        Language::Unknown => &[],
    }
}

/// Collect import nodes recursively.
fn collect_import_nodes<'a>(node: Node<'a>, import_kinds: &[&str], out: &mut Vec<Node<'a>>) {
    if import_kinds.contains(&node.kind()) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_import_nodes(child, import_kinds, out);
    }
}

/// Extract what module each import node points to.
fn extract_import_target(node: Node<'_>, source_bytes: &[u8], lang: Language) -> Option<String> {
    match lang {
        Language::Python => {
            // `import foo` or `from foo import bar`
            if let Some(n) = node.child_by_field_name("name") {
                return Some(node_text(n, source_bytes).to_string());
            }
            if let Some(n) = node.child_by_field_name("module_name") {
                return Some(node_text(n, source_bytes).to_string());
            }
            Some(node_text(node, source_bytes).trim().to_string())
        }
        Language::JavaScript | Language::TypeScript => {
            // `import ... from "path"` or `require("path")`
            if let Some(src) = node.child_by_field_name("source") {
                let raw = node_text(src, source_bytes).trim_matches('"').trim_matches('\'').to_string();
                return Some(raw);
            }
            // require() call
            if node.kind() == "call_expression" {
                let func = node.child_by_field_name("function");
                if func.map(|n| node_text(n, source_bytes)) == Some("require") {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        let mut cursor = args.walk();
                        for arg in args.children(&mut cursor) {
                            if arg.kind().contains("string") {
                                let raw = node_text(arg, source_bytes)
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .trim_matches('`')
                                    .to_string();
                                return Some(raw);
                            }
                        }
                    }
                }
            }
            None
        }
        Language::Go => {
            // import_spec has `path` field
            if let Some(p) = node.child_by_field_name("path") {
                let raw = node_text(p, source_bytes).trim_matches('"').to_string();
                return Some(raw);
            }
            None
        }
        Language::Rust => {
            // `use crate::foo::bar` → extract the path text
            let text = node_text(node, source_bytes)
                .trim_start_matches("use ")
                .trim_end_matches(';')
                .trim()
                .to_string();
            if !text.is_empty() {
                return Some(text);
            }
            None
        }
        Language::Unknown => None,
    }
}

/// Parse imports from a source string.
pub fn parse_imports(source: &str, file: &str, lang: Language) -> Vec<ImportInfo> {
    with_pooled_parser(lang, |parser| {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let source_bytes = source.as_bytes();
    let import_kinds = import_node_kinds(lang);
    let mut import_nodes = Vec::new();
    collect_import_nodes(tree.root_node(), import_kinds, &mut import_nodes);

    let source_module = file.to_string();
    let mut results = Vec::new();

    for node in import_nodes {
        if let Some(target) = extract_import_target(node, source_bytes, lang) {
            if !target.is_empty() {
                results.push(ImportInfo {
                    source_module: source_module.clone(),
                    imported_module: target,
                    line: node_start_line(node),
                });
            }
        }
    }

    results
    })
}

/// Parse imports from a file on disk.
pub fn parse_imports_file(path: &str) -> Vec<ImportInfo> {
    let lang = Language::from_extension(path);
    if lang == Language::Unknown {
        return vec![];
    }
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    parse_imports(&source, path, lang)
}

// ═══════════════════════════════════════════
// JACCARD SIMILARITY (for duplication)
// ═══════════════════════════════════════════

/// Compute token-set Jaccard similarity between two fingerprint strings.
pub fn fingerprint_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let set_a: std::collections::HashSet<&str> = a.split(';').collect();
    let set_b: std::collections::HashSet<&str> = b.split(';').collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

// ═══════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Language detection ───────────────────

    #[test]
    fn test_detect_rust() {
        assert_eq!(Language::from_extension("src/main.rs"), Language::Rust);
    }

    #[test]
    fn test_detect_python() {
        assert_eq!(Language::from_extension("app.py"), Language::Python);
    }

    #[test]
    fn test_detect_js() {
        assert_eq!(Language::from_extension("index.js"), Language::JavaScript);
    }

    #[test]
    fn test_detect_ts() {
        assert_eq!(Language::from_extension("app.ts"), Language::TypeScript);
    }

    #[test]
    fn test_detect_go() {
        assert_eq!(Language::from_extension("main.go"), Language::Go);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(Language::from_extension("Makefile"), Language::Unknown);
    }

    // ── Complexity — Rust ────────────────────

    #[test]
    fn test_rust_complexity_simple() {
        let src = r#"fn add(a: i32, b: i32) -> i32 { a + b }"#;
        let funcs = parse_complexity(src, "test.rs", Language::Rust);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "add");
        assert_eq!(funcs[0].complexity, 1);
    }

    #[test]
    fn test_rust_complexity_branching() {
        let src = r#"
fn classify(x: i32) -> &'static str {
    if x > 0 {
        "positive"
    } else if x < 0 {
        "negative"
    } else {
        "zero"
    }
}
"#;
        let funcs = parse_complexity(src, "test.rs", Language::Rust);
        assert!(!funcs.is_empty());
        assert!(funcs[0].complexity >= 2);
    }

    // ── Complexity — Python ──────────────────

    #[test]
    fn test_python_complexity_simple() {
        let src = "def hello():\n    print('hi')\n";
        let funcs = parse_complexity(src, "test.py", Language::Python);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "hello");
        assert_eq!(funcs[0].complexity, 1);
    }

    #[test]
    fn test_python_complexity_branching() {
        let src = "def f(x):\n    if x > 0:\n        return 1\n    elif x < 0:\n        return -1\n    return 0\n";
        let funcs = parse_complexity(src, "test.py", Language::Python);
        assert!(!funcs.is_empty());
        assert!(funcs[0].complexity >= 2);
    }

    // ── Complexity — JavaScript ──────────────

    #[test]
    fn test_js_complexity() {
        let src = "function greet(name) { if (name) { return 'hi ' + name; } return 'hi'; }";
        let funcs = parse_complexity(src, "test.js", Language::JavaScript);
        assert!(!funcs.is_empty());
        assert!(funcs[0].complexity >= 2);
    }

    // ── Complexity — Go ──────────────────────

    #[test]
    fn test_go_complexity() {
        let src = "package main\nfunc add(a, b int) int { return a + b }\n";
        let funcs = parse_complexity(src, "test.go", Language::Go);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "add");
        assert_eq!(funcs[0].complexity, 1);
    }

    // ── Doc coverage — Rust ──────────────────

    #[test]
    fn test_rust_doc_coverage() {
        let src = r#"
/// Documented function.
pub fn good() {}

pub fn bad() {}
"#;
        let stats = parse_doc_coverage(src, Language::Rust);
        assert_eq!(stats.total_public, 2);
        assert_eq!(stats.documented, 1);
    }

    // ── Doc coverage — Python ────────────────

    #[test]
    fn test_python_docstring() {
        let src = "def documented():\n    \"\"\"Does something.\"\"\"\n    pass\n\ndef undocumented():\n    pass\n";
        let stats = parse_doc_coverage(src, Language::Python);
        assert!(stats.total_public >= 2);
        assert!(stats.documented >= 1);
    }

    // ── Fingerprints / Duplication ───────────

    #[test]
    fn test_fingerprint_identical() {
        let a = "IF;BLOCK;RET";
        let b = "IF;BLOCK;RET";
        assert!((fingerprint_similarity(a, b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fingerprint_different() {
        let a = "IF;BLOCK;RET";
        let b = "FOR;CALL;LET";
        assert!(fingerprint_similarity(a, b) < 0.1);
    }

    #[test]
    fn test_rust_fingerprints() {
        let src = r#"
fn foo(x: i32) -> i32 {
    if x > 0 { return x; }
    0
}

fn bar(x: i32) -> i32 {
    if x > 0 { return x; }
    0
}
"#;
        let prints = parse_fingerprints(src, "test.rs", Language::Rust);
        assert_eq!(prints.len(), 2);
        let sim = fingerprint_similarity(&prints[0].fingerprint, &prints[1].fingerprint);
        assert!(sim > 0.7, "identical-logic functions should be similar, got {:.2}", sim);
    }

    // ── Imports — Python ─────────────────────

    #[test]
    fn test_python_imports() {
        let src = "import os\nfrom pathlib import Path\n";
        let imports = parse_imports(src, "test.py", Language::Python);
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.imported_module.contains("os") || i.imported_module.contains("pathlib")));
    }

    // ── Imports — JS ─────────────────────────

    #[test]
    fn test_js_imports() {
        let src = r#"import React from 'react';
import { useState } from 'react';
"#;
        let imports = parse_imports(src, "test.js", Language::JavaScript);
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.imported_module == "react"));
    }

    // ── Imports — Go ─────────────────────────

    #[test]
    fn test_go_imports() {
        let src = "package main\nimport \"fmt\"\nfunc main() { fmt.Println(\"hi\") }\n";
        let imports = parse_imports(src, "test.go", Language::Go);
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.imported_module == "fmt"));
    }
}
