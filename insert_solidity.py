#!/usr/bin/env python3

import sys

# Read file
with open("crates/fuzz-surface/src/main.rs", "r") as f:
    content = f.read()

# Find line 636 (0-indexed)
lines = content.split("\n")
target_index = 635

# Build new content
new_content = []
for i, line in enumerate(lines):
    if i == target_index:
        # Add the new function
        new_content.append("")
        new_content.append("// Solidity function analysis (simplified)")
        new_content.append(
            "fn analyze_solidity_file(source: &str, file: &str) -> Vec<FuzzableFunction> {"
        )
        new_content.append("    let mut functions = Vec::new();")
        new_content.append("    let mut line_num = 0;")
        new_content.append("")
        new_content.append("    for line in source.lines() {")
        new_content.append("        line_num += 1;")
        new_content.append("        let trimmed = line.trim();")
        new_content.append("")
        new_content.append("        // Detect Solidity functions")
        new_content.append(
            "        if trimmed.starts_with(\"function \") && trimmed.contains('(') {"
        )
        new_content.append("            let name = trimmed")
        new_content.append('                .strip_prefix("function ")')
        new_content.append('                .unwrap_or("")')
        new_content.append('                .split("(")')
        new_content.append("                .next()")
        new_content.append('                .unwrap_or("")')
        new_content.append("                .trim()")
        new_content.append("                .to_string();")
        new_content.append("")
        new_content.append(
            '            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == "_") {'
        )
        new_content.append("                functions.push(FuzzableFunction {")
        new_content.append("                    name,")
        new_content.append("                    file: file.to_string(),")
        new_content.append("                    line: line_num,")
        new_content.append("                    params: vec![],")
        new_content.append("                    score: 10,")
        new_content.append(
            '                    is_public: trimmed.starts_with("public ") || !trimmed.starts_with("private ") || trimmed.starts_with("internal "),'
        )
        new_content.append("                    complexity: 1,")
        new_content.append("                    has_harness: false,")
        new_content.append("                });")
        new_content.append("        }")
        new_content.append("    }")
        new_content.append("")
        new_content.append("    functions")
        new_content.extend(lines[i + 1 :])  # Keep rest of file after insertion

# Write back
with open("crates/fuzz-surface/src/main.rs", "w") as f:
    f.write("\n".join(new_content))

print(f"Inserted at line {target_index + 1}, file now has {len(new_content)} lines")
