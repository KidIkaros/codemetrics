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
    doccov_cmd()
        .arg(&lib)
        .assert()
        .success();
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
