// ═══════════════════════════════════════════
// IGNORE — .codemetricsignore file support
// ═══════════════════════════════════════════

use std::path::Path;

/// Load ignore patterns from `.codemetricsignore` in the given directory.
/// Returns a Vec of glob-like patterns (one per line, '#' comments supported).
pub fn load_ignore_patterns(dir: &str) -> Vec<String> {
    let ignore_path = Path::new(dir).join(".codemetricsignore");
    if !ignore_path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&ignore_path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a file path matches any ignore pattern.
/// Supports simple glob patterns: `*.ext`, `dir/`, `**/dir/`, exact matches.
pub fn is_ignored(file_path: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let path = Path::new(file_path);
    let file_name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    for pattern in patterns {
        let pat = pattern.trim();
        if pat.is_empty() {
            continue;
        }

        // **/ prefix — match any directory depth (check BEFORE directory pattern)
        if pat.starts_with("**/") {
            let suffix = &pat[3..];
            // Check if any path component or the file itself matches
            for component in path.components() {
                let comp = component.as_os_str().to_string_lossy().to_string();
                if matches_glob(&comp, suffix) {
                    return true;
                }
            }
            // Also check if the path contains the suffix as a substring after a /
            if suffix.ends_with('/') {
                let dir_suffix = &suffix[..suffix.len() - 1];
                for component in path.components() {
                    if component.as_os_str().to_string_lossy() == dir_suffix {
                        return true;
                    }
                }
            } else if file_path.contains(suffix) {
                return true;
            }
            continue;
        }

        // Directory pattern (ends with /)
        if pat.ends_with('/') {
            let dir_name = &pat[..pat.len() - 1];
            // Check if any single component matches (e.g., "generated/" matches any "generated" dir)
            for component in path.components() {
                if component.as_os_str().to_string_lossy() == dir_name {
                    return true;
                }
            }
            // Check if the path contains the full directory sequence (e.g., "src/generated/")
            if dir_name.contains('/') {
                if file_path.contains(dir_name) {
                    return true;
                }
            }
            continue;
        }

        // **/ prefix — match any directory depth
        if pat.starts_with("**/") {
            let suffix = &pat[3..];
            // Check if any path component or the file itself matches
            for component in path.components() {
                let comp = component.as_os_str().to_string_lossy().to_string();
                if matches_glob(&comp, suffix) {
                    return true;
                }
            }
            // Also check if the path contains the suffix as a substring after a /
            if suffix.ends_with('/') {
                let dir_suffix = &suffix[..suffix.len() - 1];
                for component in path.components() {
                    if component.as_os_str().to_string_lossy() == dir_suffix {
                        return true;
                    }
                }
            } else if file_path.contains(suffix) {
                return true;
            }
            continue;
        }

        // *.ext pattern
        if pat.starts_with("*.") {
            let ext = &pat[1..]; // ".ext"
            if file_name.ends_with(ext) {
                return true;
            }
            continue;
        }

        // Exact filename match
        if file_name == pat {
            return true;
        }

        // Check if any path component matches exactly
        for component in path.components() {
            let comp = component.as_os_str().to_string_lossy();
            if comp == pat {
                return true;
            }
        }

        // Full path suffix match (e.g., "src/generated/foo.rs")
        if file_path.ends_with(pat) {
            return true;
        }
    }
    false
}

/// Simple glob matching — supports `*` (any chars) and `?` (single char).
fn matches_glob(s: &str, pattern: &str) -> bool {
    // Fast path: exact match
    if s == pattern {
        return true;
    }
    // Fast path: no wildcards
    if !pattern.contains('*') && !pattern.contains('?') {
        return false;
    }
    // Convert glob to a simple check
    // For our use cases, we mainly need `*.ext` and exact matches
    if pattern == "*" {
        return true;
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        return s.ends_with(ext);
    }
    if pattern.ends_with(".*") {
        let prefix = &pattern[..pattern.len() - 1];
        return s.starts_with(prefix);
    }
    // Fallback: check if the pattern appears anywhere
    s.contains(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_patterns() {
        assert!(!is_ignored("src/main.rs", &[]));
    }

    #[test]
    fn test_exact_filename() {
        let patterns = vec!["generated.rs".to_string()];
        assert!(is_ignored("src/generated.rs", &patterns));
        assert!(!is_ignored("src/main.rs", &patterns));
    }

    #[test]
    fn test_extension_pattern() {
        let patterns = vec!["*.generated.rs".to_string()];
        assert!(is_ignored("src/foo.generated.rs", &patterns));
        assert!(!is_ignored("src/foo.rs", &patterns));
    }

    #[test]
    fn test_directory_pattern() {
        let patterns = vec!["generated/".to_string()];
        assert!(is_ignored("src/generated/foo.rs", &patterns));
        assert!(!is_ignored("src/main.rs", &patterns));
    }

    #[test]
    fn test_double_star_pattern() {
        let patterns = vec!["**/target/".to_string()];
        assert!(is_ignored("project/target/debug/foo", &patterns));
    }

    #[test]
    fn test_full_path_suffix() {
        let patterns = vec!["src/generated/".to_string()];
        // Should match files under src/generated/
        assert!(is_ignored("project/src/generated/foo.rs", &patterns));
        // Should NOT match files outside src/generated/
        assert!(!is_ignored("project/src/main.rs", &patterns));
    }
}
