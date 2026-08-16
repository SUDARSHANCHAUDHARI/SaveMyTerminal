use crate::{adapter::NativeAgent, integration::json::JsonDescriptor};
use serde_json::{Map, Value, json};
use std::{path::Path, sync::Arc};

pub fn descriptors(home: &Path) -> Vec<JsonDescriptor> {
    descriptors_with_codex_home(home, None)
}

pub fn descriptors_with_codex_home(home: &Path, codex_home: Option<&Path>) -> Vec<JsonDescriptor> {
    [NativeAgent::Codex, NativeAgent::Claude, NativeAgent::Gemini]
        .into_iter()
        .map(|agent| descriptor(home, codex_home, agent))
        .collect()
}

fn descriptor(home: &Path, codex_home: Option<&Path>, agent: NativeAgent) -> JsonDescriptor {
    let (target, events) = match agent {
        NativeAgent::Codex => (
            codex_home
                .map_or_else(|| home.join(".codex"), Path::to_path_buf)
                .join("hooks.json"),
            vec![
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "Stop",
            ],
        ),
        NativeAgent::Claude => (
            home.join(".claude/settings.json"),
            vec![
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "Stop",
                "SessionEnd",
            ],
        ),
        NativeAgent::Gemini => (
            home.join(".gemini/settings.json"),
            vec![
                "SessionStart",
                "BeforeAgent",
                "BeforeTool",
                "AfterTool",
                "AfterAgent",
                "SessionEnd",
            ],
        ),
    };
    let command = format!("savemyterminal hook {}", agent.id());
    let managed_name = matches!(agent, NativeAgent::Gemini).then_some("savemyterminal");
    let install_events = events.clone();
    let install_command = command.clone();
    let uninstall_events = events;
    let uninstall_command = command;
    JsonDescriptor::new(
        agent.id(),
        1,
        target,
        Arc::new(move |root: &mut Value| {
            install_hooks(root, agent, &install_events, &install_command, managed_name)
        }),
        Arc::new(move |root: &mut Value| {
            uninstall_hooks(root, &uninstall_events, &uninstall_command, managed_name)
        }),
        None,
    )
    .expect("built-in agent descriptor is valid")
}

fn install_hooks(
    root: &mut Value,
    agent: NativeAgent,
    events: &[&str],
    command: &str,
    managed_name: Option<&str>,
) -> Result<(), String> {
    let root = root
        .as_object_mut()
        .ok_or_else(|| "settings root must be a JSON object".to_owned())?;
    let hooks = object_entry(root, "hooks")?;
    for event in events {
        let groups = array_entry(hooks, event)?;
        if matches!(agent, NativeAgent::Codex) {
            remove_matching_handlers(groups, command, managed_name);
        }
        if groups.iter().any(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|handlers| {
                    handlers
                        .iter()
                        .any(|handler| handler_matches(handler, command, managed_name))
                })
        }) {
            continue;
        }
        let handler = match agent {
            NativeAgent::Codex => json!({
                "type": "command",
                "command": command,
                "timeout": 5
            }),
            NativeAgent::Claude => json!({
                "type": "command",
                "command": command,
                "timeout": 5
            }),
            NativeAgent::Gemini => json!({
                "name": "savemyterminal",
                "type": "command",
                "command": command,
                "timeout": 5000,
                "description": "Report privacy-safe lifecycle metadata locally"
            }),
        };
        let group = match agent {
            // Omitting the matcher means "all events" and works across Codex
            // versions that do not special-case "*" as a wildcard regex.
            NativeAgent::Codex => json!({"hooks": [handler]}),
            NativeAgent::Claude | NativeAgent::Gemini => {
                json!({"matcher": "*", "hooks": [handler]})
            }
        };
        groups.push(group);
    }
    Ok(())
}

fn remove_matching_handlers(groups: &mut Vec<Value>, command: &str, managed_name: Option<&str>) {
    for group in groups.iter_mut() {
        if let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) {
            handlers.retain(|handler| !handler_matches(handler, command, managed_name));
        }
    }
    groups.retain(|group| {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|handlers| !handlers.is_empty())
    });
}

fn uninstall_hooks(
    root: &mut Value,
    events: &[&str],
    command: &str,
    managed_name: Option<&str>,
) -> Result<(), String> {
    let Some(root) = root.as_object_mut() else {
        return Err("settings root must be a JSON object".to_owned());
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    for event in events {
        let Some(groups) = hooks.get_mut(*event).and_then(Value::as_array_mut) else {
            continue;
        };
        remove_matching_handlers(groups, command, managed_name);
    }
    hooks.retain(|_, value| value.as_array().is_none_or(|items| !items.is_empty()));
    if hooks.is_empty() {
        root.remove("hooks");
    }
    Ok(())
}

fn handler_matches(handler: &Value, command: &str, managed_name: Option<&str>) -> bool {
    handler.get("command").and_then(Value::as_str) == Some(command)
        && managed_name.is_none_or(|name| handler.get("name").and_then(Value::as_str) == Some(name))
}

fn object_entry<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    root.entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| format!("{key} must be a JSON object"))
}

fn array_entry<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Vec<Value>, String> {
    root.entry(key.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{key} must be a JSON array"))
}
