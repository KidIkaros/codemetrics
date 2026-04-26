use assert_cmd::Command;
use predicates::prelude::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

fn dupfind_cmd() -> Command {
    Command::cargo_bin("dupfind").expect("dupfind binary not found")
}

#[test]
fn test_multi_language_scan() {
    dupfind_cmd()
        .arg(FIXTURES)
        .arg("--recursive")
        .assert()
        .success();
}

#[test]
fn test_json_multi_language() {
    dupfind_cmd()
        .arg(FIXTURES)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}
