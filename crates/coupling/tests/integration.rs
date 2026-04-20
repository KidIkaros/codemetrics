use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn coupling_cmd() -> Command {
    Command::cargo_bin("coupling").expect("coupling binary not found")
}

#[test]
fn test_basic_analysis() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("MODULE COUPLING ANALYSIS"));
}

#[test]
fn test_json_output() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"modules\""));
}

#[test]
fn test_dot_output() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .arg("--format")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"));
}

#[test]
fn test_min_coupling_filter() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .arg("--min-coupling")
        .arg("1")
        .assert()
        .success();
}

#[test]
fn test_output_columns() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("FAN-IN"))
        .stdout(predicate::str::contains("FAN-OUT"))
        .stdout(predicate::str::contains("INSTABILITY"));
}

#[test]
fn test_summary_displayed() {
    coupling_cmd()
        .arg(TEST_PROJECT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Total modules"))
        .stdout(predicate::str::contains("Total dependencies"));
}
