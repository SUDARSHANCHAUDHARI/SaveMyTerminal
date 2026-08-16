use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn invalid_hook_input_is_neutral_success_and_does_not_echo_content() {
    let secret = "secret-hook-body";
    Command::cargo_bin("savemyterminal")
        .unwrap()
        .args(["hook", "codex"])
        .write_stdin(secret)
        .assert()
        .success()
        .stdout("{}\n")
        .stderr(predicate::str::contains(secret).not());
}

#[test]
fn unsupported_hook_event_is_neutral_success_without_starting_the_service() {
    let input = serde_json::json!({
        "session_id": "one",
        "hook_event_name": "UnknownFutureEvent",
        "prompt": "private prompt"
    });
    Command::cargo_bin("savemyterminal")
        .unwrap()
        .args(["hook", "gemini"])
        .write_stdin(input.to_string())
        .assert()
        .success()
        .stdout("{}\n")
        .stderr("");
}
