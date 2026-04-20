use clap::Parser;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

    // Parse imports from each module
    let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();

    for (module_name, file_path) in &module_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let imports = extract_imports(&source, module_name);
        dependencies.insert(module_name.clone(), imports);
    }

    // Build module info
    let modules = build_module_info(&dependencies);

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

fn find_modules(dir: &Path, base: &Path, file_map: &mut HashMap<String, String>, module_map: &mut HashMap<String, String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
            if let Ok(rel) = path.strip_prefix(base) {
                let module_name = rel
                    .with_extension("")
                    .to_string_lossy()
                    .replace('/', "::")
                    .replace('\\', "::")
                    .replace("mod", "")
                    .trim_end_matches("::")
                    .to_string();

                if !module_name.is_empty() {
                    file_map.insert(path.to_string_lossy().to_string(), module_name.clone());
                    module_map.insert(module_name, path.to_string_lossy().to_string());
                }
            }
        } else if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && name != ".git" && !name.starts_with('.') {
                find_modules(&path, base, file_map, module_map);
            }
        }
    }
}

fn extract_imports(source: &str, current_module: &str) -> HashSet<String> {
    let mut imports = HashSet::new();
    let crate_prefix = current_module.split("::").next().unwrap_or(current_module);

    for line in source.lines() {
        let trimmed = line.trim();

        // Match `use crate::xxx`, `use super::xxx`, `use self::xxx`
        if let Some(use_path) = trimmed.strip_prefix("use ") {
            let use_path = use_path.trim_end_matches(';').trim();

            if use_path.starts_with("crate::") {
                // Convert to module name
                let module = use_path
                    .strip_prefix("crate::")
                    .unwrap_or(use_path)
                    .split("::")
                    .next()
                    .unwrap_or(use_path);
                if module != crate_prefix {
                    imports.insert(format!("{}::{}", crate_prefix, module));
                }
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

fn build_module_info(dependencies: &HashMap<String, HashSet<String>>) -> Vec<ModuleInfo> {
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
}

fn output_json(modules: &[ModuleInfo]) {
    let total_fan_in: usize = modules.iter().map(|m| m.fan_in).sum();
    let total_fan_out: usize = modules.iter().map(|m| m.fan_out).sum();
    let n = modules.len();

    let most_coupled: Vec<String> = modules.iter()
        .filter(|m| m.fan_in + m.fan_out > 5)
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
