//! Selection is task state, never an action grant. Dispatch independently checks policy.
use crate::domain::{Intent, Mode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Read,
    ProjectWrite,
    HostExecution,
    ExternalUnknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub effect: Effect,
    pub data_access: String,
    pub enabled: bool,
    pub approved: bool,
    pub connected: bool,
    pub tools: Vec<String>,
    pub origin: String,
    pub trust: String,
}

pub fn catalog(
    configs: &[crate::mcp::Integration],
    connected: &BTreeSet<String>,
    statuses: &std::collections::BTreeMap<String, crate::mcp::IntegrationStatus>,
) -> Vec<Capability> {
    let mut native = std::collections::BTreeMap::<String, Capability>::new();
    let mut definitions = crate::tools::definitions(true, true);
    definitions.extend(crate::tools::browser_session_definitions());
    for definition in definitions {
        let Some(name) = definition["function"]["name"].as_str() else {
            continue;
        };
        let Some(group) = native_group(name) else {
            continue;
        };
        let capability = native.entry(group.into()).or_insert_with(|| Capability {
            id: group.into(),
            name: group.replace('_', " "),
            provider: "native".into(),
            effect: match group {
                "project_write" => Effect::ProjectWrite,
                "test_build" => Effect::HostExecution,
                _ => Effect::Read,
            },
            data_access: if matches!(group, "browser" | "web_read") {
                "reviewed network origins"
            } else {
                "confined task project"
            }
            .into(),
            enabled: true,
            approved: false,
            connected: true,
            tools: vec![],
            origin: "app_known".into(),
            trust: "native_policy".into(),
        });
        if !capability.tools.iter().any(|tool| tool == name) {
            capability.tools.push(name.into());
        }
    }
    let mut catalog: Vec<_> = native.into_values().collect();
    catalog.extend(configs.iter().map(|config| {
        Capability {
            id: config.group(),
            name: config.name.clone(),
            provider: config.id.clone(),
            effect: if config.preset == "custom" {
                Effect::ExternalUnknown
            } else {
                Effect::Read
            },
            data_access: if config.preset == "context7" {
                "public documentation queries"
            } else {
                "reviewed outgoing arguments and configured account"
            }
            .into(),
            enabled: config.enabled,
            approved: config.approved,
            connected: connected.contains(&config.id),
            tools: statuses
                .get(&config.id)
                .map(|status| status.tools.iter().map(|(name, _)| name.clone()).collect())
                .unwrap_or_default(),
            origin: if config.preset != "custom" {
                "official_preset"
            } else if matches!(config.transport, crate::mcp::Transport::Stdio { .. }) {
                "user_local"
            } else {
                "user_remote"
            }
            .into(),
            trust: "untrusted_content_and_annotations".into(),
        }
    }));
    catalog
}

#[derive(Default, Debug)]
pub struct Selection {
    pub groups: BTreeSet<String>,
    pub reasons: Vec<String>,
}

pub fn route(intent: Intent, request: &str, browser_only: bool, checks: bool) -> Selection {
    let mut selection = Selection::default();
    if intent == Intent::Converse {
        selection.reasons.push("Conversation: no tools".into());
        return selection;
    }
    selection.groups.insert("control".into());
    if !browser_only {
        selection
            .groups
            .extend(["project_read".into(), "git_read".into()]);
        if intent == Intent::Execute {
            selection.groups.insert("project_write".into());
        }
        if checks {
            selection.groups.insert("test_build".into());
        }
    }
    if browser_only || crate::browser_session::web_requested(request) {
        selection
            .groups
            .extend(["browser".into(), "web_read".into()]);
    }
    let text = request.to_lowercase();
    if text.contains("github") || text.contains("github.com/") {
        selection.groups.insert("github_read".into());
        selection.reasons.push("Explicit GitHub reference".into());
    }
    if text.contains("context7")
        || [
            "current",
            "latest",
            "deprecated",
            "version-specific",
            "актуальн",
        ]
        .iter()
        .any(|word| text.contains(word))
            && [
                "api",
                "documentation",
                "docs",
                "library",
                "framework",
                "документац",
            ]
            .iter()
            .any(|word| text.contains(word))
    {
        selection.groups.insert("external_docs".into());
        selection
            .reasons
            .push("Current library documentation requested".into());
    }
    selection
}

pub fn native_group(name: &str) -> Option<&'static str> {
    Some(match name {
        "inspect_project" | "read_file" | "search_text" | "search_files" | "list_directory" => {
            "project_read"
        }
        "apply_patch" | "create_file" | "delete_file" | "rename_file" => "project_write",
        "git_status" | "git_diff" => "git_read",
        "run_project_command" | "run_tests" | "start_dev_server" | "stop_dev_server" => {
            "test_build"
        }
        "web_fetch" => "web_read",
        "browser_open" | "browser_action" | "browser_submit" | "browser_check_page" => "browser",
        "finish_task" | "request_escalation" | "request_browser" | "request_capability" => {
            "control"
        }
        _ => return None,
    })
}

pub fn eligible(effect: &Effect, intent: Intent, mode: Mode) -> bool {
    intent != Intent::Converse
        && match effect {
            Effect::Read | Effect::HostExecution => true,
            Effect::ProjectWrite | Effect::ExternalUnknown => {
                intent == Intent::Execute && mode != Mode::Observe
            }
        }
}

pub fn filter_native(definitions: &mut Vec<Value>, selected: &Selection) {
    definitions.retain(|definition| {
        definition["function"]["name"]
            .as_str()
            .and_then(native_group)
            .is_some_and(|group| selected.groups.contains(group))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routing_matrix_keeps_integrations_selective() {
        for (intent, request, browser, expected, absent) in [
            (
                Intent::Converse,
                "What is a segment tree?",
                false,
                "",
                "project_read",
            ),
            (
                Intent::Execute,
                "Fix this failing unit test.",
                false,
                "test_build",
                "github_read",
            ),
            (
                Intent::Inspect,
                "Use current Tauri documentation to check this API.",
                false,
                "external_docs",
                "github_read",
            ),
            (
                Intent::Inspect,
                "Why did my latest GitHub Actions run fail?",
                false,
                "github_read",
                "external_docs",
            ),
            (
                Intent::Execute,
                "Test this localhost page.",
                true,
                "browser",
                "github_read",
            ),
            (
                Intent::Execute,
                "Upgrade this code to the current library API.",
                false,
                "external_docs",
                "github_read",
            ),
        ] {
            let selected = route(intent, request, browser, true);
            assert!(
                expected.is_empty() || selected.groups.contains(expected),
                "{request}"
            );
            assert!(!selected.groups.contains(absent), "{request}");
        }
        assert!(
            !route(Intent::Execute, "Fix this React component", false, true)
                .groups
                .contains("external_docs")
        );
        assert!(!eligible(
            &Effect::ExternalUnknown,
            Intent::Inspect,
            Mode::Project
        ));
        assert!(!eligible(
            &Effect::ExternalUnknown,
            Intent::Execute,
            Mode::Observe
        ));
    }
}
