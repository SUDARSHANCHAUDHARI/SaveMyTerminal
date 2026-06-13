use assert_cmd::Command;
use clap::Parser;
use predicates::prelude::*;
use savemyterminal::cli::{Cli, Command as CliCommand};

#[test]
fn help_lists_available_commands() {
    Command::cargo_bin("smt")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("service"))
        .stdout(predicate::str::contains("dashboard"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("uninstall"));
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

#[test]
fn run_preserves_hyphenated_child_arguments_after_separator() {
    let cli = Cli::try_parse_from(["smt", "run", "--", "tool", "--flag", "value"]).unwrap();

    match cli.command {
        CliCommand::Run(args) => {
            assert_eq!(args.command, ["tool", "--flag", "value"]);
            assert!(!args.no_status);
        }
        command => panic!("expected run command, got {command:?}"),
    }
}
