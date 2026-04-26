use assert_cmd::Command;
use predicates::prelude::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

fn propcov_cmd() -> Command {
    Command::cargo_bin("propcov").expect("propcov binary not found")
}

#[test]
fn test_multi_language_scan() {
    propcov_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .assert()
        .success();
}

#[test]
fn test_json_multi_language() {
    propcov_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}
