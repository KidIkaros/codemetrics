use clap::Parser;
use serde::Serialize;
use syn::visit::Visit;
use syn::{
    ImplItemFn, ItemEnum, ItemFn, ItemStruct, ItemTrait, Visibility,
};

use ast_parse_ts::{parse_doc_coverage_file, Language};
use quality_common::{find_source_files, Column, print_table_header, print_table_row, separator};

#[derive(Parser)]
#[command(name = "doccov", about = "Documentation coverage -- measure public API doc comment percentage")]
struct Cli {
    /// Path to scan (file or directory)
    path: String,

    /// Recursive scan
    #[arg(short, long)]
    recursive: bool,

    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Minimum coverage threshold (exit code 1 if below)
    #[arg(long)]
    min: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct DocItem {
    kind: String,     // fn, struct, enum, trait, impl_fn
    name: String,
    public: bool,
    documented: bool,
    file: String,
    line: usize,
}

#[derive(Serialize)]
struct DocReport {
    items: Vec<DocItem>,
    summary: DocSummary,
}

#[derive(Serialize)]
struct DocSummary {
    total_public: usize,
    documented: usize,
    undocumented: usize,
    coverage_pct: f64,
    by_kind: KindBreakdown,
}

#[derive(Serialize)]
struct KindBreakdown {
    functions: (usize, usize),    // (total_public, documented)
    structs: (usize, usize),
    enums: (usize, usize),
    traits: (usize, usize),
    impl_fns: (usize, usize),
}

const ALL_EXTS: &[&str] = &["rs", "py", "pyi", "js", "mjs", "ts", "tsx", "go"];

fn main() {
    let cli = Cli::parse();

    // Collect all supported source files
    let all_files = find_source_files(&cli.path, cli.recursive, ALL_EXTS);
    if all_files.is_empty() {
        eprintln!("No supported source files found at {}", cli.path);
        std::process::exit(1);
    }

    let mut all_items: Vec<DocItem> = Vec::new();

    // Process Rust files with the high-fidelity syn visitor
    let rust_files: Vec<_> = all_files.iter().filter(|f| f.ends_with(".rs")).cloned().collect();
    for file_path in &rust_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        match syn::parse_file(&source) {
            Ok(ast) => {
                let mut visitor = DocVisitor {
                    file: file_path.clone(),
                    source: &source,
                    items: Vec::new(),
                };
                visitor.visit_file(&ast);
                all_items.extend(visitor.items);
            }
            Err(e) => eprintln!("Warning: parse error in {}: {}", file_path, e),
        }
    }

    // Process non-Rust files via tree-sitter
    let other_files: Vec<_> = all_files.iter().filter(|f| !f.ends_with(".rs")).cloned().collect();
    for file_path in &other_files {
        let lang = Language::from_extension(file_path);
        if lang == Language::Unknown { continue; }
        let stats = parse_doc_coverage_file(file_path);
        for _ in 0..stats.total_public {
            all_items.push(DocItem {
                kind: lang.to_string(),
                name: String::new(),
                public: true,
                documented: false,
                file: file_path.clone(),
                line: 0,
            });
        }
        // Mark `documented` count items as documented
        for item in all_items.iter_mut()
            .filter(|i| i.file == *file_path)
            .take(stats.documented)
        {
            item.documented = true;
        }
    }

    match cli.format.as_str() {
        "json" => output_json(&all_items),
        _ => {
            let coverage = output_table(&all_items);
            if let Some(min) = cli.min {
                if coverage < min {
                    eprintln!("\nFAILED: Coverage {:.0}% is below minimum {:.0}%", coverage, min);
                    std::process::exit(1);
                }
            }
        }
    }
}

struct DocVisitor<'a> {
    file: String,
    source: &'a str,
    items: Vec<DocItem>,
}

impl<'a> DocVisitor<'a> {
    fn is_public(vis: &Visibility) -> bool {
        matches!(vis, Visibility::Public(_))
    }

    fn has_doc_comment(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            attr.path().is_ident("doc")
        })
    }

    fn estimate_line(&self, name: &str) -> usize {
        let pattern = format!("fn {}", name);
        for (i, line) in self.source.lines().enumerate() {
            if line.contains(&pattern) {
                return i + 1;
            }
        }
        let pattern = format!("struct {}", name);
        for (i, line) in self.source.lines().enumerate() {
            if line.contains(&pattern) {
                return i + 1;
            }
        }
        let pattern = format!("enum {}", name);
        for (i, line) in self.source.lines().enumerate() {
            if line.contains(&pattern) {
                return i + 1;
            }
        }
        1
    }
}

impl<'a> Visit<'a> for DocVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        let name = node.sig.ident.to_string();
        let public = Self::is_public(&node.vis);
        let documented = Self::has_doc_comment(&node.attrs);
        let line = self.estimate_line(&name);

        self.items.push(DocItem {
            kind: "fn".to_string(),
            name,
            public,
            documented,
            file: self.file.clone(),
            line,
        });

        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'a ItemStruct) {
        let name = node.ident.to_string();
        let public = Self::is_public(&node.vis);
        let documented = Self::has_doc_comment(&node.attrs);
        let line = self.estimate_line(&name);

        self.items.push(DocItem {
            kind: "struct".to_string(),
            name,
            public,
            documented,
            file: self.file.clone(),
            line,
        });

        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'a ItemEnum) {
        let name = node.ident.to_string();
        let public = Self::is_public(&node.vis);
        let documented = Self::has_doc_comment(&node.attrs);
        let line = self.estimate_line(&name);

        self.items.push(DocItem {
            kind: "enum".to_string(),
            name,
            public,
            documented,
            file: self.file.clone(),
            line,
        });

        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'a ItemTrait) {
        let name = node.ident.to_string();
        let public = Self::is_public(&node.vis);
        let documented = Self::has_doc_comment(&node.attrs);
        let line = self.estimate_line(&name);

        self.items.push(DocItem {
            kind: "trait".to_string(),
            name,
            public,
            documented,
            file: self.file.clone(),
            line,
        });

        syn::visit::visit_item_trait(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'a ImplItemFn) {
        let name = node.sig.ident.to_string();
        let public = Self::is_public(&node.vis);
        let documented = Self::has_doc_comment(&node.attrs);
        let line = self.estimate_line(&name);

        self.items.push(DocItem {
            kind: "impl_fn".to_string(),
            name,
            public,
            documented,
            file: self.file.clone(),
            line,
        });

        syn::visit::visit_impl_item_fn(self, node);
    }
}

fn output_table(items: &[DocItem]) -> f64 {
    let public_items: Vec<_> = items.iter().filter(|i| i.public).collect();
    let documented = public_items.iter().filter(|i| i.documented).count();
    let total = public_items.len();
    let coverage = if total > 0 { documented as f64 / total as f64 * 100.0 } else { 100.0 };

    // By kind breakdown
    let count_kind = |kind: &str| -> (usize, usize) {
        let items: Vec<_> = public_items.iter().filter(|i| i.kind == kind).collect();
        let doc = items.iter().filter(|i| i.documented).count();
        (items.len(), doc)
    };

    let fns = count_kind("fn");
    let structs = count_kind("struct");
    let enums = count_kind("enum");
    let traits = count_kind("trait");
    let impl_fns = count_kind("impl_fn");

    // Undocumented items
    let undocumented: Vec<_> = public_items.iter().filter(|i| !i.documented).collect();

    println!("DOCUMENTATION COVERAGE");
    println!("{}", separator(70));
    println!();
    println!("  Public items:     {}", total);
    println!("  Documented:       {} ({:.0}%)", documented, coverage);
    println!("  Undocumented:     {}", total - documented);
    println!();
    println!("  By kind:");
    println!("    Functions:      {}/{} ({:.0}%)", fns.1, fns.0, pct(fns));
    println!("    Structs:        {}/{} ({:.0}%)", structs.1, structs.0, pct(structs));
    println!("    Enums:          {}/{} ({:.0}%)", enums.1, enums.0, pct(enums));
    println!("    Traits:         {}/{} ({:.0}%)", traits.1, traits.0, pct(traits));
    println!("    Impl fns:       {}/{} ({:.0}%)", impl_fns.1, impl_fns.0, pct(impl_fns));

    if !undocumented.is_empty() {
        println!();
        println!("  UNDOCUMENTED PUBLIC ITEMS:");

        let columns = [
            Column::left("KIND", 10),
            Column::left("NAME", 30),
            Column::left("FILE", 30),
            Column::right("LINE", 5),
        ];
        print_table_header(&columns);

        for item in undocumented.iter().take(20) {
            let kind_str = format!("{} {}", icon(&item.kind), item.kind);
            let line_str = item.line.to_string();
            print_table_row(&columns, &[&kind_str, &item.name, &item.file, &line_str]);
        }
        if undocumented.len() > 20 {
            println!("    ... and {} more", undocumented.len() - 20);
        }
    }

    println!();
    let verdict = if coverage >= 90.0 {
        "Excellent"
    } else if coverage >= 70.0 {
        "Good"
    } else if coverage >= 50.0 {
        "Needs work"
    } else {
        "Poor"
    };
    println!("  Coverage: {:.0}% -- {}", coverage, verdict);

    coverage
}

fn pct((doc, total): (usize, usize)) -> f64 {
    if total > 0 { doc as f64 / total as f64 * 100.0 } else { 0.0 }
}

fn icon(kind: &str) -> &'static str {
    match kind {
        "fn" => "fn",
        "struct" => "▓",
        "enum" => "◇",
        "trait" => "△",
        "impl_fn" => "→",
        _ => "?",
    }
}

fn output_json(items: &[DocItem]) {
    let public_items: Vec<_> = items.iter().filter(|i| i.public).collect();
    let documented = public_items.iter().filter(|i| i.documented).count();
    let total = public_items.len();
    let coverage = if total > 0 { documented as f64 / total as f64 * 100.0 } else { 100.0 };

    let count_kind = |kind: &str| -> (usize, usize) {
        let items: Vec<_> = public_items.iter().filter(|i| i.kind == kind).collect();
        let doc = items.iter().filter(|i| i.documented).count();
        (items.len(), doc)
    };

    let report = DocReport {
        items: items.to_vec(),
        summary: DocSummary {
            total_public: total,
            documented,
            undocumented: total - documented,
            coverage_pct: coverage,
            by_kind: KindBreakdown {
                functions: count_kind("fn"),
                structs: count_kind("struct"),
                enums: count_kind("enum"),
                traits: count_kind("trait"),
                impl_fns: count_kind("impl_fn"),
            },
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

#[cfg(test)]
mod tests {
    use ast_parse_ts::Language;

    #[test]
    fn test_python_doc_coverage() {
        let source = r#"
def documented_func():
    """This has a docstring."""
    pass

def undocumented_func():
    pass
"#;
        let stats = ast_parse_ts::parse_doc_coverage(source, Language::Python);
        assert_eq!(stats.total_public, 2, "Should find 2 Python functions");
        assert_eq!(stats.documented, 1, "Only 1 should have docstring");
    }

    #[test]
    fn test_js_doc_coverage() {
        let source = r#"
/** Documented function */
function documentedFunc() {
    return 1;
}

function undocumentedFunc() {
    return 2;
}
"#;
        let stats = ast_parse_ts::parse_doc_coverage(source, Language::JavaScript);
        assert_eq!(stats.total_public, 2, "Should find 2 JS functions");
        assert_eq!(stats.documented, 1, "Only 1 should have JSDoc");
    }

    #[test]
    fn test_go_doc_coverage() {
        let source = r#"
// DocumentedFunc does something.
func DocumentedFunc() int {
    return 1
}

func UndocumentedFunc() int {
    return 2
}
"#;
        let stats = ast_parse_ts::parse_doc_coverage(source, Language::Go);
        assert_eq!(stats.total_public, 2, "Should find 2 Go functions");
        assert_eq!(stats.documented, 1, "Only 1 should have Go doc comment");
    }
}
