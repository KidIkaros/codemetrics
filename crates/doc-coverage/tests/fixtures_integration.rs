use assert_cmd::Command;
use predicates::prelude::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

fn doccov_cmd() -> Command {
    Command::cargo_bin("doccov").expect("doccov binary not found")
}

#[test]
fn test_multi_language_scan() {
    doccov_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("DOCUMENTATION COVERAGE"));
}

#[test]
fn test_json_multi_language() {
    doccov_cmd()
        .arg(FIXTURES)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}
