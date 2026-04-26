use assert_cmd::Command;
use predicates::prelude::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

fn coupling_cmd() -> Command {
    Command::cargo_bin("coupling").expect("coupling binary not found")
}

#[test]
fn test_multi_language_scan() {
    coupling_cmd()
        .arg(FIXTURES)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}
