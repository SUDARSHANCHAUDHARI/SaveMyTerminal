use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_phase_one_commands() {
    Command::cargo_bin("smt")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("service"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn run_requires_a_command_after_separator() {
    Command::cargo_bin("smt")
        .unwrap()
        .arg("run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("command"));
}
