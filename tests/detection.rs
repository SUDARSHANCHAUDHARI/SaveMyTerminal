use savemyterminal::detection::{
    AgentId, EnvironmentReport, EnvironmentSource, OsId, ShellId, TerminalId, detect_with,
};
use std::{collections::HashMap, ffi::OsString};

#[derive(Default)]
struct FakeEnvironment {
    variables: HashMap<String, OsString>,
    executables: Vec<String>,
}

impl EnvironmentSource for FakeEnvironment {
    fn variable(&self, name: &str) -> Option<OsString> {
        self.variables.get(name).cloned()
    }

    fn executable_exists(&self, name: &str) -> bool {
        self.executables.iter().any(|candidate| candidate == name)
    }
}

#[test]
fn detection_reports_only_known_capability_identifiers() {
    let source = FakeEnvironment {
        variables: HashMap::from([
            ("SHELL".to_owned(), OsString::from("/bin/zsh")),
            ("TERM_PROGRAM".to_owned(), OsString::from("ghostty")),
        ]),
        executables: vec!["codex".to_owned(), "claude".to_owned()],
    };

    let report = detect_with(&source, OsId::Macos);

    assert_eq!(report.os, OsId::Macos);
    assert_eq!(report.shell, Some(ShellId::Zsh));
    assert_eq!(report.agents, [AgentId::Codex, AgentId::Claude]);
    assert_eq!(report.terminals, [TerminalId::Ghostty]);
}

#[test]
fn detection_deduplicates_terminal_signals_and_ignores_unknown_values() {
    let source = FakeEnvironment {
        variables: HashMap::from([
            (
                "SHELL".to_owned(),
                OsString::from("/private/bin/custom-shell"),
            ),
            ("TERM".to_owned(), OsString::from("xterm-kitty")),
            ("TERM_PROGRAM".to_owned(), OsString::from("kitty")),
            ("LC_SECRET".to_owned(), OsString::from("do-not-report")),
        ]),
        executables: vec!["gemini".to_owned()],
    };

    let report = detect_with(&source, OsId::Linux);

    assert_eq!(report.shell, None);
    assert_eq!(report.agents, [AgentId::Gemini]);
    assert_eq!(report.terminals, [TerminalId::Kitty]);
}

#[test]
fn serialized_report_contains_no_paths_or_raw_environment_values() {
    let report = EnvironmentReport {
        os: OsId::Windows,
        shell: Some(ShellId::Pwsh),
        agents: vec![AgentId::Codex],
        terminals: vec![TerminalId::Wezterm],
    };

    let encoded = serde_json::to_string(&report).unwrap();

    assert_eq!(
        encoded,
        r#"{"os":"windows","shell":"pwsh","agents":["codex"],"terminals":["wezterm"]}"#
    );
    for prohibited in ["/Users/", "PATH", "username", "hostname", "do-not-report"] {
        assert!(!encoded.contains(prohibited));
    }
}
