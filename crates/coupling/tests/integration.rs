use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");
const FIXTURE_PROJECT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn coupling_cmd() -> Command {
    Command::cargo_bin("coupling").expect("coupling binary not found")
}

#[test]
fn test_basic_analysis() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("MODULE COUPLING ANALYSIS"));
}

#[test]
fn test_json_output() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"modules\""));
}

#[test]
fn test_dot_output() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .arg("--format")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"));
}

#[test]
fn test_min_coupling_filter() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .arg("--min-coupling")
        .arg("1")
        .assert()
        .success();
}

#[test]
fn test_output_columns() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("FAN-IN"))
        .stdout(predicate::str::contains("FAN-OUT"))
        .stdout(predicate::str::contains("INSTABILITY"));
}

#[test]
fn test_summary_displayed() {
    coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total modules"))
        .stdout(predicate::str::contains("Total dependencies"));
}

#[test]
fn test_min_coupling_5_filter() {
    // Run with --min-coupling 5 on the workspace root to find tightly coupled modules
    let workspace = format!("{}/../..", TEST_PROJECT);
    let output = coupling_cmd()
        .arg(&workspace)
        .arg("--min-coupling")
        .arg("5")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.contains("No modules found") {
        // No modules meet the threshold - this is valid
        return;
    }

    assert!(stdout.contains("MODULE COUPLING ANALYSIS"), "Missing header");
    assert!(stdout.contains("MODULE"), "Missing MODULE column header");
    assert!(stdout.contains("FAN-IN"), "Missing FAN-IN column header");
    assert!(stdout.contains("FAN-OUT"), "Missing FAN-OUT column header");
    assert!(stdout.contains("INSTABILITY"), "Missing INSTABILITY column header");
    assert!(stdout.contains("STATUS"), "Missing STATUS column header");
    assert!(stdout.contains("Total modules"), "Missing 'Total modules' in summary");
    assert!(stdout.contains("Total dependencies"), "Missing 'Total dependencies' in summary");
}

#[test]
fn test_module_listing_format() {
    // Verify that module rows contain expected data fields
    let output = coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify separator lines (uses Unicode ─ characters)
    assert!(stdout.contains("\u{2500}"), "Missing table separator line");
    // Verify summary fields
    assert!(stdout.contains("Total modules"), "Missing 'Total modules'");
    assert!(stdout.contains("Total dependencies"), "Missing 'Total dependencies'");
    assert!(stdout.contains("Avg fan-in"), "Missing 'Avg fan-in'");
    assert!(stdout.contains("Avg fan-out"), "Missing 'Avg fan-out'");
}

#[test]
fn test_most_coupled_section() {
    // Verify the MOST COUPLED section appears when there are tightly coupled modules
    let output = coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If any modules have coupling > 5, MOST COUPLED section should appear
    if stdout.contains("MOST COUPLED") {
        // Should show fan-in and fan-out for each listed module
        assert!(stdout.contains("fan-in:"), "Missing 'fan-in:' in MOST COUPLED section");
        assert!(stdout.contains("fan-out:"), "Missing 'fan-out:' in MOST COUPLED section");
    }
}

#[test]
fn test_status_indicators() {
    // Verify status indicators (low/moderate/high) appear based on coupling levels
    let output = coupling_cmd()
        .arg(FIXTURE_PROJECT)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // At least one status indicator should be present
    let has_status = stdout.contains("low") || stdout.contains("moderate") || stdout.contains("high");
    assert!(has_status, "No status indicators found in output");
}
