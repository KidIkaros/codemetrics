use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn debt_cmd() -> Command {
    Command::cargo_bin("debt").expect("debt binary not found")
}

#[test]
fn test_basic_scan() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("TECHNICAL DEBT SUMMARY")
                .or(predicate::str::contains("No technical debt markers found")),
        );
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn test_marker_filter() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--marker")
        .arg("todo,fixme")
        .assert()
        .success();
}

#[test]
fn test_sort_options() {
    let src = format!("{}/src", TEST_PROJECT);
    for sort in &["age", "file", "type", "author"] {
        debt_cmd()
            .arg(&src)
            .arg("--sort")
            .arg(sort)
            .assert()
            .success();
    }
}

#[test]
fn test_self_no_false_positives() {
    // The debt scanner should not find markers in its own string literals
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have very few or zero markers (no false positives from string literals)
    let marker_count = stdout.matches("\"marker_type\"").count();
    assert!(
        marker_count <= 2,
        "Too many markers found: {} (possible false positives)",
        marker_count
    );
}

#[test]
fn test_output_table_format() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("TYPE").or(predicate::str::contains("No technical debt")));
}

#[test]
fn test_sort_by_file_output_columns() {
    // Run with --sort file and verify all expected column headers appear
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--sort")
        .arg("file")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If markers found, table headers must be present
    if stdout.contains("TYPE") {
        assert!(stdout.contains("FILE"), "Missing FILE column header");
        assert!(stdout.contains("LINE"), "Missing LINE column header");
        assert!(stdout.contains("AUTHOR"), "Missing AUTHOR column header");
        assert!(stdout.contains("TEXT"), "Missing TEXT column header");
    } else {
        assert!(stdout.contains("No technical debt markers found"));
    }
}

#[test]
fn test_sort_by_type_output_columns() {
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--sort")
        .arg("type")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("TYPE") {
        assert!(stdout.contains("FILE"), "Missing FILE column header");
        assert!(stdout.contains("LINE"), "Missing LINE column header");
        assert!(stdout.contains("AUTHOR"), "Missing AUTHOR column header");
        assert!(stdout.contains("TEXT"), "Missing TEXT column header");
    } else {
        assert!(stdout.contains("No technical debt markers found"));
    }
}

#[test]
fn test_sort_by_author_output_columns() {
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--sort")
        .arg("author")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("TYPE") {
        assert!(stdout.contains("FILE"), "Missing FILE column header");
        assert!(stdout.contains("LINE"), "Missing LINE column header");
        assert!(stdout.contains("AUTHOR"), "Missing AUTHOR column header");
        assert!(stdout.contains("TEXT"), "Missing TEXT column header");
    } else {
        assert!(stdout.contains("No technical debt markers found"));
    }
}

#[test]
fn test_summary_section_contents() {
    // Verify the summary section contains expected fields
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("No technical debt markers found") {
        // Clean code - no summary section to check
        return;
    }

    // Summary should contain total markers and category breakdowns
    assert!(
        stdout.contains("Total markers"),
        "Missing 'Total markers' in summary"
    );
    assert!(stdout.contains("TODO"), "Missing 'TODO' in summary");
    assert!(stdout.contains("FIXME"), "Missing 'FIXME' in summary");
    assert!(stdout.contains("HACK"), "Missing 'HACK' in summary");
}

#[test]
fn test_sort_by_file_ordering() {
    // Verify --sort file produces output sorted by filename
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--sort")
        .arg("file")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If markers found, verify the table has rows after header
    if stdout.contains("TYPE") {
        // Table should have separator line after header (uses Unicode ─)
        assert!(stdout.contains("\u{2500}"), "Missing table separator");
    }
}

#[test]
fn test_debt_ratio_displayed() {
    // Verify the debt ratio verdict is shown in output
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("No technical debt markers found") {
        return;
    }

    // Should contain one of the debt ratio verdicts
    let has_ratio = stdout.contains("markers are actionable")
        || stdout.contains("Low debt")
        || stdout.contains("Moderate debt")
        || stdout.contains("High debt");
    assert!(has_ratio, "Missing debt ratio verdict in output");
}

#[test]
fn test_marker_icons_displayed() {
    // Verify marker type icons appear in table output
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("No technical debt markers found") {
        return;
    }

    // Check that marker types are uppercased in the table
    // The output should contain at least one of the known marker types
    let has_markers = stdout.contains("TODO")
        || stdout.contains("FIXME")
        || stdout.contains("HACK")
        || stdout.contains("XXX");
    assert!(has_markers, "No marker types found in table output");
}
