use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn crap_cmd() -> Command {
    Command::cargo_bin("crap").expect("crap binary not found")
}

#[test]
fn test_basic_analysis() {
    let src = format!("{}/src", TEST_PROJECT);
    crap_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("FUNCTION"))
        .stdout(predicate::str::contains("SUMMARY"));
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    crap_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"functions\""))
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn test_with_coverage() {
    // Create a minimal valid lcov file
    let lcov_path = "/tmp/test-integration-crap.info";
    std::fs::write(lcov_path, "TN:\nSF:/fake.rs\nLF:10\nLH:5\nend_of_record\n").unwrap();

    let src = format!("{}/src", TEST_PROJECT);
    crap_cmd()
        .arg(&src)
        .arg("--coverage")
        .arg(lcov_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("SUMMARY"));

    std::fs::remove_file(lcov_path).ok();
}

#[test]
fn test_min_score_filter() {
    let src = format!("{}/src", TEST_PROJECT);
    crap_cmd()
        .arg(&src)
        .arg("--min-score")
        .arg("100")
        .assert()
        .success();
}

#[test]
fn test_single_file() {
    let lib = format!("{}/src/main.rs", TEST_PROJECT);
    crap_cmd()
        .arg(&lib)
        .assert()
        .success()
        .stdout(predicate::str::contains("main"));
}

#[test]
fn test_nonexistent_path() {
    crap_cmd()
        .arg("/tmp/nonexistent-path-12345")
        .assert()
        .failure() // exits 1 when no files found
        .stderr(predicate::str::contains("No .rs files found"));
}

#[test]
fn test_categories_displayed() {
    let src = format!("{}/src", TEST_PROJECT);
    crap_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("excellent"))
        .stdout(predicate::str::contains("good"));
}
