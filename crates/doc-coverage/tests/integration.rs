use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn doccov_cmd() -> Command {
    Command::cargo_bin("doccov").expect("doccov binary not found")
}

#[test]
fn test_basic_scan() {
    let src = format!("{}/src", TEST_PROJECT);
    doccov_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("DOCUMENTATION COVERAGE"));
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    doccov_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn test_min_threshold_pass() {
    let src = format!("{}/src", TEST_PROJECT);
    doccov_cmd()
        .arg(&src)
        .arg("--min")
        .arg("0")
        .assert()
        .success();
}

#[test]
fn test_single_file() {
    let lib = format!("{}/src/main.rs", TEST_PROJECT);
    doccov_cmd().arg(&lib).assert().success();
}

#[test]
fn test_output_sections() {
    let src = format!("{}/src", TEST_PROJECT);
    doccov_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("Public items"))
        .stdout(predicate::str::contains("Documented"))
        .stdout(predicate::str::contains("Coverage"));
}

#[test]
fn test_undocumented_items_listing_columns() {
    // Verify the undocumented items table has KIND and NAME columns
    let src = format!("{}/src", TEST_PROJECT);
    let output = doccov_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If there are undocumented items, verify the table structure
    if stdout.contains("UNDOCUMENTED PUBLIC ITEMS") {
        assert!(stdout.contains("KIND"), "Missing KIND column header");
        assert!(stdout.contains("NAME"), "Missing NAME column header");
        assert!(
            stdout.contains("FILE"),
            "Missing FILE column header in undocumented listing"
        );
        assert!(
            stdout.contains("LINE"),
            "Missing LINE column header in undocumented listing"
        );
    }
}

#[test]
fn test_undocumented_items_table_rows() {
    // Verify table separator appears after headers in undocumented items
    let src = format!("{}/src", TEST_PROJECT);
    let output = doccov_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("UNDOCUMENTED PUBLIC ITEMS") {
        // Should have separator line after column headers (uses Unicode ─)
        assert!(stdout.contains("\u{2500}"), "Missing table separator line");
    }
}

#[test]
fn test_summary_fields_present() {
    // Verify the summary section has all expected breakdown fields
    let src = format!("{}/src", TEST_PROJECT);
    let output = doccov_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("DOCUMENTATION COVERAGE"), "Missing header");
    assert!(
        stdout.contains("Public items"),
        "Missing 'Public items' in summary"
    );
    assert!(
        stdout.contains("Documented"),
        "Missing 'Documented' in summary"
    );
    assert!(
        stdout.contains("Undocumented"),
        "Missing 'Undocumented' in summary"
    );
    assert!(stdout.contains("By kind"), "Missing 'By kind' breakdown");
    assert!(
        stdout.contains("Functions"),
        "Missing 'Functions' breakdown"
    );
    assert!(stdout.contains("Structs"), "Missing 'Structs' breakdown");
    assert!(stdout.contains("Coverage"), "Missing 'Coverage' verdict");
}

#[test]
fn test_by_kind_breakdown() {
    // Verify the per-kind breakdown shows percentage values
    let src = format!("{}/src", TEST_PROJECT);
    let output = doccov_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Functions:"), "Missing Functions breakdown");
    assert!(stdout.contains("Structs:"), "Missing Structs breakdown");
    assert!(stdout.contains("Enums:"), "Missing Enums breakdown");
    assert!(stdout.contains("Traits:"), "Missing Traits breakdown");
}

#[test]
fn test_coverage_verdict() {
    // Verify the final coverage verdict line
    let src = format!("{}/src", TEST_PROJECT);
    let output = doccov_cmd().arg(&src).arg("--recursive").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let has_verdict = stdout.contains("Excellent")
        || stdout.contains("Good")
        || stdout.contains("Needs work")
        || stdout.contains("Poor");
    assert!(
        has_verdict,
        "Missing coverage verdict (Excellent/Good/Needs work/Poor)"
    );
}
