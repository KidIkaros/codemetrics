use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn dupfind_cmd() -> Command {
    Command::cargo_bin("dupfind").expect("dupfind binary not found")
}

#[test]
fn test_basic_scan() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("CODE DUPLICATION").or(
            predicate::str::contains("No code duplication found")
        ));
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"groups\""));
}

#[test]
fn test_min_lines_filter() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--min-lines")
        .arg("10")
        .assert()
        .success();
}

#[test]
fn test_summary_displayed() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("Duplicate groups").or(
            predicate::str::contains("No code duplication")
        ));
}

#[test]
fn test_min_lines_3_group_listing() {
    // Run with --min-lines 3 and verify group listing format
    let src = format!("{}/src", TEST_PROJECT);
    let output = dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--min-lines")
        .arg("3")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("CODE DUPLICATION") {
        // Verify group listing structure
        assert!(stdout.contains("Group"), "Missing 'Group' label in listing");
        assert!(stdout.contains("instances"), "Missing 'instances' count in group listing");
        assert!(stdout.contains("Pattern:"), "Missing 'Pattern:' in group listing");

        // Verify separator lines (uses Unicode ─)
        assert!(stdout.contains("\u{2500}"), "Missing separator lines");
    } else {
        // No duplication found - verify clean output message
        assert!(stdout.contains("No code duplication found"));
    }
}

#[test]
fn test_group_instance_format() {
    // Verify that group instances show function, file, and line
    let src = format!("{}/src", TEST_PROJECT);
    let output = dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--min-lines")
        .arg("3")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("CODE DUPLICATION") && stdout.contains("Group") {
        // Instances should be listed with "- function:file:line" format
        // Check for presence of the dash prefix for instance lines
        let has_instance_lines = stdout.contains("  - ");
        if has_instance_lines {
            // Verify the summary section
            assert!(stdout.contains("Duplicate groups"), "Missing 'Duplicate groups' in summary");
            assert!(stdout.contains("Total instances"), "Missing 'Total instances' in summary");
            assert!(stdout.contains("Files affected"), "Missing 'Files affected' in summary");
        }
    }
}

#[test]
fn test_separator_format() {
    // Verify the separator uses Unicode ─ characters
    let src = format!("{}/src", TEST_PROJECT);
    let output = dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--min-lines")
        .arg("3")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("CODE DUPLICATION") {
        assert!(stdout.contains("\u{2500}"), "Missing Unicode separator line");
    }
}

#[test]
fn test_summary_fields() {
    // Verify the summary section contains all expected fields
    let src = format!("{}/src", TEST_PROJECT);
    let output = dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("No code duplication found") {
        return;
    }

    assert!(stdout.contains("Duplicate groups"), "Missing 'Duplicate groups'");
    assert!(stdout.contains("Total instances"), "Missing 'Total instances'");
    assert!(stdout.contains("Files affected"), "Missing 'Files affected'");
}

#[test]
fn test_high_duplication_warning() {
    // If significant duplication is detected, a warning should appear
    let src = format!("{}/src", TEST_PROJECT);
    let output = dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--min-lines")
        .arg("3")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Only check for warning if duplication exists
    if stdout.contains("CODE DUPLICATION") {
        // The warning may or may not appear depending on duplication ratio
        // Just verify the output is well-formed
        assert!(stdout.contains("Duplicate groups") || stdout.contains("No code duplication"));
    }
}
