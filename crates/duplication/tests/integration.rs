use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn dupfind_cmd() -> Command {
    Command::cargo_bin("dupfind").expect("dupfind binary not found")
}

#[test]
fn test_basic_scan() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("CODE DUPLICATION").or(
            predicate::str::contains("No code duplication found")
        ));
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"groups\""));
}

#[test]
fn test_min_lines_filter() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--min-lines")
        .arg("10")
        .assert()
        .success();
}

#[test]
fn test_summary_displayed() {
    let src = format!("{}/src", TEST_PROJECT);
    dupfind_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("Duplicate groups").or(
            predicate::str::contains("No code duplication")
        ));
}
