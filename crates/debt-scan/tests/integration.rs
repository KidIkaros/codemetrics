use assert_cmd::Command;
use predicates::prelude::*;

const TEST_PROJECT: &str = env!("CARGO_MANIFEST_DIR");

fn debt_cmd() -> Command {
    Command::cargo_bin("debt").expect("debt binary not found")
}

#[test]
fn test_basic_scan() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("TECHNICAL DEBT SUMMARY").or(
            predicate::str::contains("No technical debt markers found")
        ));
}

#[test]
fn test_json_output() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn test_marker_filter() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--marker")
        .arg("todo,fixme")
        .assert()
        .success();
}

#[test]
fn test_sort_options() {
    let src = format!("{}/src", TEST_PROJECT);
    for sort in &["age", "file", "type", "author"] {
        debt_cmd()
            .arg(&src)
            .arg("--sort")
            .arg(sort)
            .assert()
            .success();
    }
}

#[test]
fn test_self_no_false_positives() {
    // The debt scanner should not find markers in its own string literals
    let src = format!("{}/src", TEST_PROJECT);
    let output = debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have very few or zero markers (no false positives from string literals)
    let marker_count = stdout.matches("\"marker_type\"").count();
    assert!(marker_count <= 2, "Too many markers found: {} (possible false positives)", marker_count);
}

#[test]
fn test_output_table_format() {
    let src = format!("{}/src", TEST_PROJECT);
    debt_cmd()
        .arg(&src)
        .arg("--recursive")
        .assert()
        .success()
        .stdout(predicate::str::contains("TYPE").or(
            predicate::str::contains("No technical debt")
        ));
}
