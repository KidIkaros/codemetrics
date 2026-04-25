use clap::Parser;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use ast_parse_ts::{parse_imports_file, Language};

#[derive(Parser)]
#[command(name = "coupling", about = "Coupling analysis -- module dependency graphs, fan-in/fan-out")]
struct Cli {
    /// Path to scan (directory with src/)
    path: String,

    /// Output format: table (default), json, or dot (Graphviz)
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Show only tightly coupled modules (fan-in + fan-out > threshold)
    #[arg(long, default_value = "0")]
    min_coupling: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ModuleInfo {
    name: String,
    imports: Vec<String>,     // modules this one depends on
    imported_by: Vec<String>, // modules that depend on this one
    fan_out: usize,           // how many modules I depend on
    fan_in: usize,            // how many modules depend on me
    instability: f64,         // fan_out / (fan_in + fan_out)
    implicit_deps: Vec<String>, // detected module references without explicit use statements
}

#[derive(Serialize)]
struct CouplingReport {
    modules: Vec<ModuleInfo>,
    summary: CouplingSummary,
}

#[derive(Serialize)]
struct CouplingSummary {
    total_modules: usize,
    total_dependencies: usize,
    avg_fan_in: f64,
    avg_fan_out: f64,
    most_coupled: Vec<String>,
    modules_with_implicit: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    let src_dir = Path::new(&cli.path);
    let src_path = if src_dir.join("src").is_dir() {
        src_dir.join("src")
    } else if src_dir.is_dir() {
        src_dir.to_path_buf()
    } else {
        eprintln!("No source directory found at {}", cli.path);
        std::process::exit(1);
    };

    // Find all .rs files and map them to module names
    let mut file_modules: HashMap<String, String> = HashMap::new();
    let mut module_files: HashMap<String, String> = HashMap::new();
    find_modules(&src_path, &src_path, &mut file_modules, &mut module_files);

    // Parse workspace Cargo.toml to identify internal crates
    let internal_crates = find_workspace_crates(&cli.path);

    // Parse imports from each module
    let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();
    let mut implicit_refs: HashMap<String, HashSet<String>> = HashMap::new();

    // Collect all known module names for implicit reference detection
    let known_modules: HashSet<String> = module_files.keys().cloned().collect();

    for (module_name, file_path) in &module_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let imports = extract_imports(&source, module_name, &internal_crates);
        dependencies.insert(module_name.clone(), imports.clone());

        let implicit = extract_implicit_refs(&source, module_name, &known_modules, &imports);
        implicit_refs.insert(module_name.clone(), implicit);
    }

    // Also scan non-Rust source files via tree-sitter
    scan_multilang_imports(&src_path, &mut dependencies);

    // Build module info
    let modules = build_module_info(&dependencies, &implicit_refs);

    // Filter by minimum coupling
    let filtered: Vec<_> = if cli.min_coupling > 0 {
        modules
            .into_iter()
            .filter(|m| (m.fan_in + m.fan_out) >= cli.min_coupling)
            .collect()
    } else {
        modules
    };

    match cli.format.as_str() {
        "json" => output_json(&filtered),
        "dot" => output_dot(&filtered),
        _ => output_table(&filtered),
    }
}

/// Scan non-Rust source files via tree-sitter and inject their imports into the dependency graph.
fn scan_multilang_imports(dir: &Path, deps: &mut HashMap<String, HashSet<String>>) {
    const EXTS: &[&str] = &["py", "pyi", "js", "mjs", "ts", "tsx", "go"];
    collect_multilang_files(dir, EXTS, deps);
}

fn collect_multilang_files(dir: &Path, exts: &[&str], deps: &mut HashMap<String, HashSet<String>>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if exts.contains(&ext) {
                let file_str = path.to_string_lossy().to_string();
                let lang = Language::from_extension(&file_str);
                if lang == Language::Unknown { continue; }
                // Use file path as the module key
                let module_key = file_str.clone();
                let import_info = parse_imports_file(&file_str);
                let targets: HashSet<String> = import_info
                    .into_iter()
                    .map(|i| i.imported_module)
                    .filter(|m| !m.is_empty())
                    .collect();
                if !targets.is_empty() {
                    deps.entry(module_key).or_default().extend(targets);
                }
            }
        } else if path.is_dir() && should_traverse_dir(&path) {
            collect_multilang_files(&path, exts, deps);
        }
    }
}

/// Convert a file path to a module name relative to the base directory.
fn module_name_from_path(path: &Path, base: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    let module_name = rel
        .with_extension("")
        .to_string_lossy()
        .replace('/', "::")
        .replace('\\', "::")
        .replace("mod", "")
        .trim_end_matches("::")
        .to_string();
    if module_name.is_empty() {
        None
    } else {
        Some(module_name)
    }
}

/// Returns true if a directory should be traversed (not a build/git/hidden dir).
fn should_traverse_dir(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name != "target" && name != ".git" && !name.starts_with('.')
}

fn find_modules(dir: &Path, base: &Path, file_map: &mut HashMap<String, String>, module_map: &mut HashMap<String, String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            if let Some(module_name) = module_name_from_path(&path, base) {
                file_map.insert(path.to_string_lossy().to_string(), module_name.clone());
                module_map.insert(module_name, path.to_string_lossy().to_string());
            }
        } else if path.is_dir() && should_traverse_dir(&path) {
            find_modules(&path, base, file_map, module_map);
        }
    }
}

/// Parse workspace member crate names from Cargo.toml content.
fn parse_workspace_members(content: &str) -> HashSet<String> {
    let mut crates = HashSet::new();
    if !content.contains("[workspace]") {
        return crates;
    }
    let mut in_members = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") {
            in_members = true;
            continue;
        }
        if in_members {
            if trimmed == "]" {
                break;
            }
            if let Some(path_str) = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"').or(Some(s)))
            {
                let path_str = path_str.trim_end_matches('"').trim_end_matches(',');
                let crate_name = Path::new(path_str)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if !crate_name.is_empty() {
                    crates.insert(crate_name);
                }
            }
        }
    }
    crates
}

fn find_workspace_crates(start_path: &str) -> HashSet<String> {
    let mut current = PathBuf::from(start_path);

    // Walk up to find workspace Cargo.toml
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                let crates = parse_workspace_members(&content);
                if !crates.is_empty() {
                    return crates;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }

    HashSet::new()
}

/// Attempt to extract a cross-crate import from a `use` path.
/// Returns `Some("crate::<name>")` for external crate imports, or `None`.
fn extract_cross_crate_import(use_path: &str) -> Option<String> {
    let first_segment = use_path.split("::").next().unwrap_or("");
    if !first_segment.is_empty()
        && !first_segment.starts_with("std")
        && !first_segment.starts_with("core")
        && !first_segment.starts_with("alloc")
        && first_segment != "self"
        && first_segment != "super"
    {
        Some(format!("crate::{}", first_segment))
    } else {
        None
    }
}

/// Detect implicit module references in source code that aren't covered by `use` statements.
/// These are direct path references like `super::foo::bar()`, `crate::bar::baz`, or `mod_name::Type`.
fn extract_implicit_refs(
    source: &str,
    current_module: &str,
    known_modules: &HashSet<String>,
    explicit_imports: &HashSet<String>,
) -> HashSet<String> {
    let mut implicit = HashSet::new();
    let crate_prefix = current_module.split("::").next().unwrap_or(current_module);

    for line in source.lines() {
        let trimmed = line.trim();
        // Skip comments and use statements (already covered)
        if trimmed.starts_with("//") || trimmed.starts_with("use ") {
            continue;
        }

        // Pattern 1: `crate::xxx::` or `crate::xxx` references
        if let Some(start) = line.find("crate::") {
            let after = &line[start + 7..];
            // Extract the next path component: alphanumeric, underscore, colon (for :: separators)
            let end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
                .unwrap_or(after.len());
            let path = &after[..end];
            let parts: Vec<&str> = path.split("::").filter(|s| !s.is_empty()).collect();
            if !parts.is_empty() {
                let dep = format!("{}::{}::...", crate_prefix, parts[0]);
                if path != crate_prefix && !explicit_imports.contains(&dep.replace("::...", "")) {
                    implicit.insert(dep);
                }
            }
        }

        // Pattern 2: `super::xxx::` references
        if let Some(start) = line.find("super::") {
            let after = &line[start + 7..];
            let end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
                .unwrap_or(after.len());
            let path = &after[..end];
            let parts: Vec<&str> = path.split("::").filter(|s| !s.is_empty()).collect();
            if !parts.is_empty() {
                let dep = format!("{}::super::{}::...", crate_prefix, parts[0]);
                implicit.insert(dep);
            }
        }

        // Pattern 3: Direct `xxx::yyy` where xxx is another known module
        for module in known_modules {
            let short = module.split("::").last().unwrap_or(module);
            let pattern = format!("{}::", short);
            if line.contains(&pattern) && short != crate_prefix {
                // Don't flag if it's part of a use statement or already imported
                let dep_short = format!("{}::{}::...", crate_prefix, short);
                if !explicit_imports.contains(&dep_short.replace("::...", "")) {
                    implicit.insert(dep_short);
                }
            }
        }
    }

    implicit
}

fn extract_imports(source: &str, current_module: &str, _internal_crates: &HashSet<String>) -> HashSet<String> {
    let mut imports = HashSet::new();
    let crate_prefix = current_module.split("::").next().unwrap_or(current_module);

    for line in source.lines() {
        let trimmed = line.trim();

        // Match `use crate::xxx`, `use super::xxx`, `use self::xxx`
        if let Some(use_path) = trimmed.strip_prefix("use ") {
            let use_path = use_path.trim_end_matches(';').trim();

            if use_path.starts_with("crate::") {
                let module = use_path
                    .strip_prefix("crate::")
                    .unwrap_or(use_path)
                    .split("::")
                    .next()
                    .unwrap_or(use_path);
                if module != crate_prefix {
                    imports.insert(format!("{}::{}", crate_prefix, module));
                }
            } else if let Some(dep) = extract_cross_crate_import(use_path) {
                imports.insert(dep);
            }
        }

        // Match `mod xxx;`
        if let Some(mod_name) = trimmed.strip_prefix("mod ") {
            let mod_name = mod_name.trim_end_matches(';').trim();
            if !mod_name.is_empty() && !mod_name.contains('{') {
                imports.insert(format!("{}::{}", current_module, mod_name));
            }
        }
    }

    imports
}

fn build_module_info(
    dependencies: &HashMap<String, HashSet<String>>,
    implicit_refs: &HashMap<String, HashSet<String>>,
) -> Vec<ModuleInfo> {
    // Build reverse dependency map
    let mut reverse: HashMap<String, HashSet<String>> = HashMap::new();
    for (module, deps) in dependencies {
        for dep in deps {
            reverse.entry(dep.clone()).or_default().insert(module.clone());
        }
    }

    let mut all_modules: HashSet<String> = dependencies.keys().cloned().collect();
    all_modules.extend(reverse.keys().cloned());

    let mut modules: Vec<ModuleInfo> = all_modules
        .iter()
        .map(|name| {
            let imports: Vec<String> = dependencies
                .get(name)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            let imported_by: Vec<String> = reverse
                .get(name)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            let implicit_deps: Vec<String> = implicit_refs
                .get(name)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            let fan_out = imports.len();
            let fan_in = imported_by.len();
            let total = fan_in + fan_out;
            let instability = if total > 0 {
                fan_out as f64 / total as f64
            } else {
                0.0
            };

            ModuleInfo {
                name: name.clone(),
                imports,
                imported_by,
                fan_out,
                fan_in,
                instability,
                implicit_deps,
            }
        })
        .collect();

    modules.sort_by(|a, b| (b.fan_in + b.fan_out).cmp(&(a.fan_in + a.fan_out)));
    modules
}

fn output_table(modules: &[ModuleInfo]) {
    if modules.is_empty() {
        println!("No modules found.");
        return;
    }

    println!("MODULE COUPLING ANALYSIS");
    println!("{}", "─".repeat(80));
    println!(
        "\n{:<40} {:>8} {:>8} {:>12} {}",
        "MODULE", "FAN-IN", "FAN-OUT", "INSTABILITY", "STATUS"
    );
    println!("{}", "─".repeat(80));

    let total_fan_in: usize = modules.iter().map(|m| m.fan_in).sum();
    let total_fan_out: usize = modules.iter().map(|m| m.fan_out).sum();

    for m in modules {
        let total = m.fan_in + m.fan_out;
        let status = if total > 10 {
            "⚠ high"
        } else if total > 5 {
            "○ moderate"
        } else {
            "✓ low"
        };

        println!(
            "{:<40} {:>8} {:>8} {:>11.2} {}",
            truncate(&m.name, 38),
            m.fan_in,
            m.fan_out,
            m.instability,
            status,
        );
    }

    println!("{}", "─".repeat(80));
    println!();
    println!("  Total modules:       {}", modules.len());
    println!("  Total dependencies:  {}", modules.iter().map(|m| m.fan_out).sum::<usize>());
    println!("  Avg fan-in:          {:.1}", total_fan_in as f64 / modules.len() as f64);
    println!("  Avg fan-out:         {:.1}", total_fan_out as f64 / modules.len() as f64);

    // Most coupled
    let coupled: Vec<_> = modules.iter()
        .filter(|m| m.fan_in + m.fan_out > 5)
        .collect();

    if !coupled.is_empty() {
        println!();
        println!("  MOST COUPLED:");
        for m in coupled.iter().take(5) {
            println!("    {} (fan-in: {}, fan-out: {})", m.name, m.fan_in, m.fan_out);
        }
    }

    // Implicit dependencies
    let with_implicit: Vec<_> = modules.iter()
        .filter(|m| !m.implicit_deps.is_empty())
        .collect();

    if !with_implicit.is_empty() {
        println!();
        println!("  IMPLICIT DEPENDENCIES (no explicit use statement):");
        for m in with_implicit.iter().take(5) {
            println!("    {} has {} implicit reference(s): {}",
                m.name,
                m.implicit_deps.len(),
                m.implicit_deps.join(", "));
        }
        let total_implicit: usize = with_implicit.iter().map(|m| m.implicit_deps.len()).sum();
        println!();
        println!("    {} modules have {} total implicit dependencies.",
            with_implicit.len(), total_implicit);
        println!("    Consider adding explicit use statements for clarity.");
    }
}

fn output_json(modules: &[ModuleInfo]) {
    let total_fan_in: usize = modules.iter().map(|m| m.fan_in).sum();
    let total_fan_out: usize = modules.iter().map(|m| m.fan_out).sum();
    let n = modules.len();

    let most_coupled: Vec<String> = modules.iter()
        .filter(|m| m.fan_in + m.fan_out > 5)
        .map(|m| m.name.clone())
        .collect();

    let modules_with_implicit: Vec<String> = modules.iter()
        .filter(|m| !m.implicit_deps.is_empty())
        .map(|m| m.name.clone())
        .collect();

    let report = CouplingReport {
        modules: modules.to_vec(),
        summary: CouplingSummary {
            total_modules: n,
            total_dependencies: modules.iter().map(|m| m.fan_out).sum(),
            avg_fan_in: if n > 0 { total_fan_in as f64 / n as f64 } else { 0.0 },
            avg_fan_out: if n > 0 { total_fan_out as f64 / n as f64 } else { 0.0 },
            most_coupled,
            modules_with_implicit,
        },
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn output_dot(modules: &[ModuleInfo]) {
    println!("digraph coupling {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box, style=filled, fillcolor=lightblue];");
    println!();

    for m in modules {
        let short_name = m.name.split("::").last().unwrap_or(&m.name);
        for dep in &m.imports {
            let dep_short = dep.split("::").last().unwrap_or(dep);
            println!("  \"{}\" -> \"{}\";", short_name, dep_short);
        }
    }

    println!("}}");
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - max + 1..])
    }
}
