use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn mutate_cmd() -> Command {
    Command::cargo_bin("mutate").expect("mutate binary not found")
}

#[test]
fn test_verify_original_tests() {
    // Mutation tester should verify tests pass on original code first
    mutate_cmd()
        .arg(TEST_PROJECT)
        .arg("--max-mutants")
        .arg("0")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Original tests pass")
                .or(predicate::str::contains("No mutants to test")),
        );
}

#[test]
fn test_json_output_with_zero_mutants() {
    mutate_cmd()
        .arg(TEST_PROJECT)
        .arg("--max-mutants")
        .arg("0")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
}

#[test]
fn test_nonexistent_path() {
    mutate_cmd()
        .arg("/tmp/nonexistent-crate-12345")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Cargo.toml found"));
}
