use crate::{
    domain::*,
    fs::ConfinedRoot,
    security,
    snapshots::{self, Snapshot},
    store::Store,
};
use serde_json::json;
use std::{io::Write, path::Path};

pub fn preview(store: &Store, task_id: &str) -> Result<HandoffPreview> {
    let task: Task = store.get("tasks", task_id)?;
    let project: Project = store.get("projects", &task.project_id)?;
    let mut omissions=vec!["Credentials, dependency directories, unrelated file bodies and private model reasoning are excluded.".to_string()];
    if !crate::images::ids(&task.inputs).is_empty() {
        omissions.push("Local image pixels are EXCLUDED. Review images for private information and explicitly choose any images to attach in your cloud agent; this export never uploads them.".into());
    }
    let changes = if let Some(sid) = &task.snapshot_id {
        snapshots::deltas(store, &store.get::<Snapshot>("snapshots", sid)?)?
    } else {
        vec![]
    };
    let mut patch = String::new();
    for delta in &changes {
        if !delta.eligible {
            omissions.push(format!(
                "Patch omitted: {} is binary or secret-bearing.",
                delta.path
            ));
            patch.clear();
            break;
        }
        patch.push_str(
            &similar::TextDiff::from_lines(&delta.before, &delta.after)
                .unified_diff()
                .header(&format!("a/{}", delta.path), &format!("b/{}", delta.path))
                .to_string(),
        );
    }
    let patch = if patch.is_empty() {
        None
    } else if patch.len() > 256 * 1024 {
        omissions.push("Complete patch exceeded 256 KiB and was omitted; no truncated patch is presented as applicable.".into());
        None
    } else {
        Some(patch)
    };
    let mut evidence = String::new();
    for check in task.checks.iter().rev().take(3) {
        evidence.push_str(&format!(
            "### {} — {}\nTested manifest: `{}`; exit: {:?}\n```text\n{}\n```\n",
            check.name,
            check.status,
            check.manifest_hash,
            check.exit_code,
            security::bounded(&check.output, 2500)
        ));
    }
    let actions = task
        .events
        .iter()
        .filter(|e| {
            e.kind == "mcp" || matches!(e.title.as_str(), "Browser observation" | "Web observation")
        })
        .rev()
        .take(2)
        .map(|e| security::bounded(&security::redact(&e.detail), 1200))
        .collect::<Vec<_>>()
        .join("\n");
    if !actions.is_empty() {
        evidence.push_str(&format!(
            "### External evidence (untrusted, bounded)\n{actions}\n"
        ));
        omissions.push("Browser evidence is a compact selection; cookies, request headers, full HTML and image pixels are not included. Review page excerpts before sharing.".into());
    }
    let actions = task
        .events
        .iter()
        .filter(|e| {
            matches!(
                e.kind.as_str(),
                "tool" | "changes" | "warning" | "validation"
            )
        })
        .rev()
        .take(16)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|e| format!("- {} — {}", e.title, security::bounded(&e.detail, 180)))
        .collect::<Vec<_>>()
        .join("\n");
    let files = changes
        .iter()
        .map(|d| {
            format!(
                "- `{}`: {} (+{} / -{})",
                d.path, d.kind, d.additions, d.deletions
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut snippets = String::new();
    if let Ok(root) = ConfinedRoot::open(Path::new(&project.path)) {
        let words = task
            .request
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<Vec<_>>();
        let mut relevant = project
            .files
            .iter()
            .filter(|p| {
                words
                    .iter()
                    .any(|w| w.len() > 3 && p.to_lowercase().contains(w))
            })
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        relevant.extend(changes.iter().take(3).map(|d| d.path.clone()));
        relevant.sort();
        relevant.dedup();
        for path in relevant.into_iter().take(6) {
            if let Ok(bytes) = root.read(&path, 512 * 1024) {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    snippets.push_str(&format!(
                        "### {} — current original, hash {}\n```text\n{}\n```\n",
                        path,
                        hash(&bytes),
                        security::bounded(
                            &security::redact(
                                &text.lines().take(35).collect::<Vec<_>>().join("\n")
                            ),
                            900
                        )
                    ));
                }
            }
        }
    }
    let baseline = task
        .snapshot_id
        .as_ref()
        .and_then(|s| store.get::<Snapshot>("snapshots", s).ok())
        .map(|s| s.manifest_hash)
        .unwrap_or_else(|| "No task snapshot (investigation only)".into());
    let raw=format!("# Continue this local coding task\n\n## User request and success criteria\n{}\n\n## Current state — read before changing files\nProject: {} ({})\nBaseline manifest: {}\nGit HEAD: {}\nProposed changes {} applied to the original project.\nExisting user changes: {}\nPatch base is the captured working files, which may include user edits; it is not necessarily HEAD. Verify local hashes before applying.\n\n## Why local execution stopped\n{}\nRoute: level {}; reasons: {}\n\n## Evidence\n{}\n\n## Work already attempted\n{}\n\n## Proposed changes\n{}\n\n## Relevant code\n{}\n\n## Constraints and boundaries\nLocal Windows project, {} source files indexed. Local-only inference. No automatic deployment, installs, account actions or destructive Git operations. Preserve existing user changes.\n\n## Suggested next steps\nVerify the current files and check results above. Investigate the first unresolved failure, make a bounded change, and rerun the relevant tests. Treat suspected causes as unverified until tested.\n\n## Evidence omissions\n{}\n",security::bounded(&task.request,4000),project.name,project.stack,baseline,project.git.as_ref().and_then(|g|g.head.as_deref()).unwrap_or("Unavailable / non-Git"),if task.change_state=="applied"{"ARE"}else{"ARE NOT"},project.git.as_ref().map(|g|if g.status.is_empty(){"Clean at inspection"}else{"Present at inspection; preserve them"}).unwrap_or("Non-Git or Git unavailable"),task.summary,task.route.level,task.route.reasons.join("; "),evidence,actions,if files.is_empty(){"No proposed file changes."}else{&files},snippets,project.files.len(),omissions.iter().map(|s|format!("- {s}")).collect::<Vec<_>>().join("\n"));
    let continuation=task.continuation.as_ref().map(|c|format!("\n## Accumulated local investigation\nStages: {}\nReason: {}\nRelevant files: {}\nRecorded outcomes:\n{}\nCurrent diff is retained against snapshot {}. Remaining questions: {}\n",task.stages.iter().map(|s|format!("{} ({})",s.tier,s.model)).collect::<Vec<_>>().join(" → "),c.escalation_reason,c.relevant_files.join(", "),c.tool_outcomes.iter().rev().take(5).map(|s|security::bounded(s,500)).collect::<Vec<_>>().join("\n"),c.snapshot_id.as_deref().unwrap_or("none"),c.unresolved.join("; "))).unwrap_or_default();
    let raw = raw.replacen(
        "## Evidence\n",
        &format!("{}\n## Evidence\n", continuation),
        1,
    );
    let markdown = privacy(&security::redact(&raw), &project.path);
    let markdown = if markdown.len() > 8000 {
        omissions.push(
            "Prompt bounded to 8,000 bytes; full selected check evidence is available in export."
                .into(),
        );
        security::bounded(&markdown, 7800)
    } else {
        markdown
    };
    let out = HandoffPreview {
        id: id(),
        task_id: task.id,
        bytes: (markdown.len() + patch.as_ref().map_or(0, String::len)) as u32,
        markdown,
        patch,
        omissions,
    };
    store.put("handoffs", &out.id, &out)?;
    Ok(out)
}
fn privacy(text: &str, project_path: &str) -> String {
    let mut out = text.replace(project_path, "<PROJECT>");
    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            out = out.replace(&value, "<USER_HOME>");
            out = out.replace(&value.replace('\\', "/"), "<USER_HOME>");
        }
    }
    out
}
// Redact JSON string values BEFORE serialization so truncation/redaction cannot
// invalidate escaping or leave a partial JSON document in the exported package.
fn evidence_json<T: serde::Serialize>(records: &[T], project_path: &str) -> Result<Vec<u8>> {
    fn clean(value: &mut serde_json::Value, project_path: &str) {
        match value {
            serde_json::Value::String(s) => {
                *s = security::bounded(&privacy(&security::redact(s), project_path), 24000)
            }
            serde_json::Value::Array(values) => {
                values.iter_mut().for_each(|v| clean(v, project_path))
            }
            serde_json::Value::Object(values) => {
                values.values_mut().for_each(|v| clean(v, project_path))
            }
            _ => {}
        }
    }
    let mut selected = Vec::new();
    for record in records.iter().rev().take(100) {
        let mut value = serde_json::to_value(record)?;
        clean(&mut value, project_path);
        selected.push(value);
        if serde_json::to_vec(&selected)?.len() > 96 * 1024 {
            selected.pop();
            break;
        }
    }
    selected.reverse();
    Ok(serde_json::to_vec_pretty(
        &json!({"records":selected,"omittedRecords":records.len()-selected.len(),"stringLimitBytes":24000}),
    )?)
}
pub fn export(store: &Store, preview_id: &str, parent: &Path) -> Result<String> {
    let preview: HandoffPreview = store.get("handoffs", preview_id)?;
    let task: Task = store.get("tasks", &preview.task_id)?;
    let project: Project = store.get("projects", &task.project_id)?;
    let parent = parent.canonicalize()?;
    let project_root = Path::new(&project.path).canonicalize()?;
    if parent.starts_with(&project_root) {
        return Err(AppError::new(
            "EXPORT_LOCATION",
            "Choose a folder outside the project to keep handoff content out of source control.",
        ));
    }
    let folder = parent.join(format!("local-agent-handoff-{}", preview.id));
    std::fs::create_dir(&folder)?;
    std::fs::create_dir(folder.join("evidence"))?;
    let mut manifest = Vec::new();
    let mut write = |name: &str, bytes: &[u8]| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(name))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        manifest.push(json!({"relativePath":name,"sha256":hash(bytes),"bytes":bytes.len()}));
        Ok(())
    };
    write("HANDOFF.md", preview.markdown.as_bytes())?;
    if let Some(patch) = &preview.patch {
        write("changes.patch", patch.as_bytes())?;
    }
    write(
        "evidence/tests.json",
        &evidence_json(&task.checks, &project.path)?,
    )?;
    write(
        "evidence/activity.json",
        &evidence_json(&task.events, &project.path)?,
    )?;
    if let Some(context) = &task.continuation {
        write(
            "evidence/continuation.json",
            &evidence_json(std::slice::from_ref(context), &project.path)?,
        )?;
    }
    write(
        "metadata.json",
        &serde_json::to_vec_pretty(
            &json!({"schemaVersion":1,"taskId":task.id,"createdAt":now(),"changesApplied":task.change_state=="applied","platform":"Windows","stack":project.stack,"model":task.model,"omissions":preview.omissions}),
        )?,
    )?;
    let data = serde_json::to_vec_pretty(
        &json!({"schemaVersion":1,"files":manifest,"requiresManualReview":true}),
    )?;
    std::fs::write(folder.join("MANIFEST.json"), data)?;
    Ok(folder.to_string_lossy().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_evidence_remains_valid_json() {
        let records = vec![json!({"output":"\"C:\\Project\\quoted\"\n".repeat(5000) }); 110];
        let bytes = evidence_json(&records, "C:\\Project").unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value["omittedRecords"].as_u64().unwrap() > 0);
        assert!(!String::from_utf8(bytes.clone())
            .unwrap()
            .contains("Project"));
        assert!(bytes.len() < 128 * 1024);
    }
}
