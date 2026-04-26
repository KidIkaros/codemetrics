use assert_cmd::Command;
use predicates::prelude::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

fn taint_cmd() -> Command {
    Command::cargo_bin("taint").expect("taint binary not found")
}

#[test]
fn test_multi_language_scan() {
    taint_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .assert()
        .success();
}

#[test]
fn test_json_multi_language() {
    taint_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"violations\""));
}
