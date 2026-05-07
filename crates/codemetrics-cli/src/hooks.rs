// ═══════════════════════════════════════════
// GIT HOOKS — install/uninstall pre-commit
// ═══════════════════════════════════════════

use crate::project::ProjectProfile;

pub fn install_hooks(repo: &str, fast: bool) -> i32 {
    let profile = crate::project::detect_project(repo);
    install_hooks_impl(repo, fast, &profile)
}

pub fn install_hooks_impl(repo: &str, fast: bool, profile: &ProjectProfile) -> i32 {
    let hook_dir = format!("{}/.git/hooks", repo);
    let hook_path = format!("{}/pre-commit", hook_dir);

    if !std::path::Path::new(&hook_dir).exists() {
        eprintln!(
            "install-hooks: {} is not a git repository (no .git/hooks directory)",
            repo
        );
        return 1;
    }

    if std::path::Path::new(&hook_path).exists() {
        eprintln!(
            "install-hooks: hook already exists at {} -- remove it first or use uninstall-hooks",
            hook_path
        );
        return 1;
    }

    let hook_script = build_hook_script(fast, profile);

    match std::fs::write(&hook_path, hook_script) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("install-hooks: write failed: {}", e);
            return 1;
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .expect("Failed to get file metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).ok();
    }

    println!("Installed pre-commit hook at {}", hook_path);
    if fast {
        println!("Mode: fast (metrics only, no tests)");
    } else {
        println!(
            "Mode: full (runs tests + coverage for {} before checking)",
            profile.ecosystem
        );
    }
    println!("To bypass: git commit --no-verify");
    println!("To remove: codemetrics uninstall-hooks {}", repo);
    0
}

pub fn build_hook_script(fast: bool, profile: &ProjectProfile) -> String {
    let cm_bin = r#"CM_BIN=""
if command -v codemetrics &>/dev/null; then
    CM_BIN="codemetrics"
elif [ -f target/release/codemetrics ]; then
    CM_BIN="./target/release/codemetrics"
else
    echo "codemetrics: binary not found, skipping pre-commit check" >&2
    exit 0
fi"#;

    if fast || !profile.is_coverage_available() {
        format!(
            r#"#!/usr/bin/env bash
# CodeMetrics pre-commit hook (fast/metrics-only) — installed by `codemetrics install-hooks`
# Remove with: codemetrics uninstall-hooks
set -euo pipefail

{cm_bin}

$CM_BIN check . --format text
"#,
            cm_bin = cm_bin
        )
    } else {
        let test_cmd = profile.test_cmd.join(" ");
        let cov_cmd = profile.coverage_cmd.join(" ");
        let lcov_flag = if !profile.lcov_path.is_empty() {
            format!("--coverage {}", profile.lcov_path)
        } else {
            String::new()
        };
        format!(
            r#"#!/usr/bin/env bash
# CodeMetrics pre-commit hook (full: tests + coverage + metrics) — installed by `codemetrics install-hooks`
# Remove with: codemetrics uninstall-hooks
# To skip: git commit --no-verify
set -euo pipefail

{cm_bin}

echo "[codemetrics] Running tests ({ecosystem})..."
{test_cmd}

echo "[codemetrics] Collecting coverage..."
{cov_cmd}

echo "[codemetrics] Running quality checks..."
$CM_BIN check . {lcov_flag} --format text
"#,
            cm_bin = cm_bin,
            ecosystem = profile.ecosystem,
            test_cmd = test_cmd,
            cov_cmd = cov_cmd,
            lcov_flag = lcov_flag,
        )
    }
}

pub fn uninstall_hooks(repo: &str) -> i32 {
    let hook_path = format!("{}/.git/hooks/pre-commit", repo);

    if !std::path::Path::new(&hook_path).exists() {
        eprintln!("uninstall-hooks: no pre-commit hook found at {}", hook_path);
        return 1;
    }

    let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
    if !content.contains("CodeMetrics pre-commit hook") {
        eprintln!(
            "uninstall-hooks: {} exists but was not installed by codemetrics — refusing to remove",
            hook_path
        );
        return 1;
    }

    match std::fs::remove_file(&hook_path) {
        Ok(_) => {
            println!("Removed pre-commit hook from {}", hook_path);
            0
        }
        Err(e) => {
            eprintln!("uninstall-hooks: remove failed: {}", e);
            1
        }
    }
}
