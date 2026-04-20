use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn riskmap_cmd() -> Command {
    Command::cargo_bin("riskmap").expect("riskmap binary not found")
}

#[test]
fn test_basic_analysis() {
    riskmap_cmd()
        .arg(TEST_PROJECT)
        .arg("--since")
        .arg("1 year ago")
        .assert()
        .success()
        .stdout(predicate::str::contains("RISK MAP").or(
            predicate::str::contains("No risk data found")
        ));
}

#[test]
fn test_json_output() {
    riskmap_cmd()
        .arg(TEST_PROJECT)
        .arg("--format")
        .arg("json")
        .arg("--since")
        .arg("1 year ago")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"files\"").or(
            predicate::str::contains("No risk data")
        ));
}

#[test]
fn test_min_risk_filter() {
    riskmap_cmd()
        .arg(TEST_PROJECT)
        .arg("--since")
        .arg("1 year ago")
        .arg("--min-risk")
        .arg("10")
        .assert()
        .success();
}

#[test]
fn test_output_columns() {
    riskmap_cmd()
        .arg(TEST_PROJECT)
        .arg("--since")
        .arg("1 year ago")
        .assert()
        .success()
        .stdout(predicate::str::contains("CHURN").or(
            predicate::str::contains("No risk data")
        ));
}

#[test]
fn test_summary_displayed() {
    riskmap_cmd()
        .arg(TEST_PROJECT)
        .arg("--since")
        .arg("1 year ago")
        .assert()
        .success()
        .stdout(predicate::str::contains("Files analyzed").or(
            predicate::str::contains("No risk data")
        ));
}
