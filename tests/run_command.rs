use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn build_helper() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp
        .path()
        .join(format!("exit-with{}", std::env::consts::EXE_SUFFIX));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/helpers/exit_with.rs");
    let status = std::process::Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());
    (temp, binary)
}

#[test]
fn preserves_arguments_and_success_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "hello world", "--flag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=hello world"))
        .stdout(predicate::str::contains("arg=--flag"));
}

#[test]
fn preserves_nonzero_exit_code() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .arg("23")
        .assert()
        .code(23);
}

#[test]
fn launches_child_when_service_is_unavailable() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--no-status", "--"])
        .arg(helper)
        .args(["0", "still-ran"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arg=still-ran"));
}

#[test]
fn unknown_executable_name_is_not_rendered() {
    let (_temp, helper) = build_helper();
    Command::cargo_bin("smt")
        .unwrap()
        .env("SMT_TEST_FORCE_SERVICE_FAILURE", "1")
        .args(["run", "--"])
        .arg(helper)
        .arg("0")
        .assert()
        .success()
        .stderr(predicate::str::contains("smt [unknown] starting"))
        .stderr(predicate::str::contains("exit-with").not());
}
