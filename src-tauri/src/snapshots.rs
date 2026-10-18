use crate::{domain::*, fs::ConfinedRoot, platform, security, store::Store};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub project_id: String,
    pub identity: RootIdentity,
    pub files: BTreeMap<String, String>,
    pub exclusions: Vec<String>,
    pub git: Option<GitInfo>,
    pub manifest_hash: String,
    #[serde(default)]
    pub ignore_rules: Vec<(String, String)>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub done: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Journal {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub identity: RootIdentity,
    pub state: String,
    pub undo: bool,
    #[serde(default)]
    pub recovery_of: Option<String>,
    pub entries: Vec<JournalEntry>,
}

pub fn working_path(store: &Store, snapshot_id: &str) -> Result<std::path::PathBuf> {
    uuid::Uuid::parse_str(snapshot_id)
        .map_err(|_| AppError::new("SNAPSHOT_ID", "Invalid task copy reference."))?;
    Ok(store.root.join("tasks").join(snapshot_id).join("working"))
}
pub fn capture(
    store: &Store,
    project: &Project,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<Snapshot> {
    let original = ConfinedRoot::open(Path::new(&project.path))?;
    original.verify(&project.identity)?;
    let snapshot_id = id();
    let working = working_path(store, &snapshot_id)?;
    std::fs::create_dir_all(&working)?;
    let copy = ConfinedRoot::open(&working)?;
    let (mut files, exclusions) = original.list()?;
    // Tracked files remain eligible even if newly added ignore patterns match them.
    for path in &project.files {
        if !files.contains(path) && !security::protected(path) && !security::artifact(path) {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();
    let estimated = files
        .iter()
        .map(|p| std::fs::metadata(original.path().join(p)).map(|m| m.len()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum::<u64>();
    if estimated > 512 * 1024 * 1024
        || platform::free_bytes(&store.root)? < estimated.saturating_mul(2) + 256 * 1024 * 1024
    {
        return Err(AppError::new("SNAPSHOT_SPACE","There is not enough free space for an independent task copy and recovery backup. Choose a smaller folder or free disk space."));
    }
    let mut baseline = BTreeMap::new();
    let mut total = 0usize;
    for path in files {
        if cancel.is_cancelled() {
            return Err(AppError::new(
                "CANCELLED",
                "The task was stopped before editing.",
            ));
        }
        let bytes = original.read(&path, 20 * 1024 * 1024)?;
        total += bytes.len();
        if total > 512 * 1024 * 1024 {
            return Err(AppError::new(
                "SNAPSHOT_LIMIT",
                "Choose a smaller project folder (maximum 512 MiB of source).",
            ));
        }
        let digest = store.blob(&bytes)?;
        copy.write_new(&path, &bytes)?;
        baseline.insert(path, digest);
    }
    let mut ignore_rules = Vec::new();
    for (path, digest) in &baseline {
        if path.rsplit('/').next() == Some(".gitignore") {
            let scope = path.rsplit_once('/').map(|v| v.0).unwrap_or("").to_string();
            ignore_rules.push((
                scope,
                String::from_utf8_lossy(&store.bytes(digest)?).into_owned(),
            ));
        }
    }
    ignore_rules.sort_by_key(|(scope, _)| scope.len());
    let snapshot = Snapshot {
        id: snapshot_id,
        project_id: project.id.clone(),
        identity: project.identity.clone(),
        manifest_hash: manifest_hash(&baseline),
        files: baseline,
        exclusions,
        git: project.git.clone(),
        ignore_rules,
    };
    let check = manifest_for(&original, &snapshot)?;
    if check != snapshot.files {
        return Err(AppError::new(
            "PROJECT_BUSY",
            "Project files changed while creating the task copy. Finish saving, then try again.",
        ));
    }
    store.put("snapshots", &snapshot.id, &snapshot)?;
    Ok(snapshot)
}
pub fn manifest(root: &ConfinedRoot) -> Result<BTreeMap<String, String>> {
    let (files, _) = root.list()?;
    let mut result = BTreeMap::new();
    let mut total = 0usize;
    for path in files {
        let bytes = root.read(&path, 20 * 1024 * 1024)?;
        total += bytes.len();
        if total > 512 * 1024 * 1024 {
            return Err(AppError::new(
                "SNAPSHOT_LIMIT",
                "Task source exceeds its storage limit.",
            ));
        }
        result.insert(path, hash(&bytes));
    }
    Ok(result)
}
pub fn manifest_hash(files: &BTreeMap<String, String>) -> String {
    hash(&serde_json::to_vec(files).expect("manifest serializable"))
}
pub fn manifest_for(root: &ConfinedRoot, snapshot: &Snapshot) -> Result<BTreeMap<String, String>> {
    let (files, _) = root.list_policy(Some(&snapshot.ignore_rules))?;
    let mut result = BTreeMap::new();
    let mut total = 0usize;
    for path in files {
        let bytes = root.read(&path, 20 * 1024 * 1024)?;
        total += bytes.len();
        if total > 512 * 1024 * 1024 {
            return Err(AppError::new(
                "SNAPSHOT_LIMIT",
                "Task source exceeded its 512 MiB limit.",
            ));
        }
        result.insert(path, hash(&bytes));
    }
    for path in snapshot.files.keys() {
        if !result.contains_key(path) {
            let target = root.path().join(path);
            match std::fs::symlink_metadata(&target) {
                Ok(_) => {
                    result.insert(path.clone(), hash(&root.read(path, 20 * 1024 * 1024)?));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(result)
}

pub fn deltas(store: &Store, snapshot: &Snapshot) -> Result<Vec<FileDelta>> {
    let root = ConfinedRoot::open(&working_path(store, &snapshot.id)?)?;
    let current = manifest_for(&root, snapshot)?;
    let paths = snapshot
        .files
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    for path in paths {
        let before_hash = snapshot.files.get(&path).cloned();
        let after_hash = current.get(&path).cloned();
        if before_hash == after_hash {
            continue;
        }
        let before = before_hash
            .as_ref()
            .map(|h| store.bytes(h))
            .transpose()?
            .unwrap_or_default();
        let after = if after_hash.is_some() {
            root.read(&path, 20 * 1024 * 1024)?
        } else {
            vec![]
        };
        let eligible = security::editable(&before)
            && security::editable(&after)
            && !security::protected(&path);
        if !eligible {
            let kind = if before_hash.is_none() {
                "added"
            } else if after_hash.is_none() {
                "deleted"
            } else {
                "modified"
            }
            .into();
            result.push(FileDelta {
                path,
                before_hash,
                after_hash,
                before: "[Binary, large or protected content omitted]".into(),
                after: "[Binary, large or protected content omitted]".into(),
                additions: 0,
                deletions: 0,
                kind,
                eligible: false,
            });
            continue;
        }
        let before = String::from_utf8_lossy(&before).into_owned();
        let after = String::from_utf8_lossy(&after).into_owned();
        let diff = similar::TextDiff::from_lines(&before, &after);
        let additions = diff
            .iter_all_changes()
            .filter(|c| c.tag() == similar::ChangeTag::Insert)
            .count() as u32;
        let deletions = diff
            .iter_all_changes()
            .filter(|c| c.tag() == similar::ChangeTag::Delete)
            .count() as u32;
        let kind = if before_hash.is_none() {
            "added"
        } else if after_hash.is_none() {
            "deleted"
        } else {
            "modified"
        }
        .into();
        result.push(FileDelta {
            path,
            before_hash,
            after_hash,
            before: security::redact(&before),
            after: security::redact(&after),
            additions,
            deletions,
            kind,
            eligible,
        });
    }
    Ok(result)
}

fn read_locked(file: &mut File) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(20 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(AppError::new(
            "FILE_LIMIT",
            "The original file is too large to restore automatically.",
        ));
    }
    Ok(bytes)
}
pub(crate) fn delete_locked(file: &File) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::{BOOLEAN, HANDLE},
        Storage::FileSystem::{
            FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
        },
    };
    let info = FILE_DISPOSITION_INFO {
        DeleteFile: BOOLEAN(1),
    };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            &info as *const _ as *const _,
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(|e| {
        AppError::new(
            "DELETE_FAILED",
            "The file could not be removed; recovery data is retained.",
        )
        .detail(e)
    })?;
    Ok(())
}

pub fn transact(
    store: &Store,
    project: &Project,
    task: &Task,
    undo: bool,
    current_git: Option<GitInfo>,
) -> Result<Journal> {
    let snapshot: Snapshot = store.get(
        "snapshots",
        task.snapshot_id
            .as_deref()
            .ok_or_else(|| AppError::new("NO_CHANGES", "This task has no proposed files."))?,
    )?;
    let pending: Vec<Journal> = store.list("journals")?;
    if pending
        .iter()
        .any(|j| j.project_id == project.id && j.state == "recovery_required")
    {
        return Err(AppError::new(
            "RECOVERY_REQUIRED",
            "Restore the interrupted file operation before changing this project.",
        ));
    }
    if let (Some(base), Some(current)) = (&snapshot.git, &current_git) {
        if base.head != current.head || base.index_hash != current.index_hash {
            return Err(AppError::new("GIT_CHANGED", "The project branch or staged changes changed. Review them before applying this task."));
        }
    }
    let root = ConfinedRoot::open(Path::new(&project.path))?;
    root.verify(&snapshot.identity)?;
    let entries = if undo {
        let applied = pending
            .iter()
            .find(|j| j.task_id == task.id && !j.undo && j.state == "complete")
            .ok_or_else(|| {
                AppError::new(
                    "UNDO_UNAVAILABLE",
                    "No applied changes are available to undo.",
                )
            })?;
        applied
            .entries
            .iter()
            .map(|e| JournalEntry {
                path: e.path.clone(),
                before: e.after.clone(),
                after: e.before.clone(),
                done: false,
            })
            .collect::<Vec<_>>()
    } else {
        let changes = deltas(store, &snapshot)?;
        if changes.is_empty() {
            return Err(AppError::new(
                "NO_CHANGES",
                "There are no proposed changes to apply.",
            ));
        }
        if changes.len() > if task.route.level == 3 { 6 } else { 12 }
            || changes
                .iter()
                .map(|d| d.before.len() + d.after.len())
                .sum::<usize>()
                > 1024 * 1024
        {
            return Err(AppError::new(
                "SCOPE_LIMIT",
                "This change set exceeds the task's safety limits. Export it for review.",
            ));
        }
        if changes.iter().any(|d| !d.eligible) {
            return Err(AppError::new(
                "UNSAFE_CHANGE",
                "Binary or secret-bearing changes need manual review and cannot be applied here.",
            ));
        }
        let working = ConfinedRoot::open(&working_path(store, &snapshot.id)?)?;
        let mut entries = Vec::new();
        for delta in changes {
            let after = if delta.after_hash.is_some() {
                Some(store.blob(&working.read(&delta.path, 512 * 1024)?)?)
            } else {
                None
            };
            entries.push(JournalEntry {
                path: delta.path,
                before: delta.before_hash,
                after,
                done: false,
            });
        }
        entries
    };
    apply_entries(store, &root, task, entries, undo, None)
}

fn apply_entries(
    store: &Store,
    root: &ConfinedRoot,
    task: &Task,
    entries: Vec<JournalEntry>,
    undo: bool,
    recovery_of: Option<String>,
) -> Result<Journal> {
    let mut locked = Vec::new();
    let mut parents = Vec::new();
    // Lock and validate every existing path before original bytes can change.
    for entry in &entries {
        let (path, pins) = root.pin_parent(&entry.path, false).or_else(|e| {
            if entry.before.is_none() && e.code == "FILE_ACCESS" {
                Ok((root.path().join(&entry.path), vec![]))
            } else {
                Err(e)
            }
        })?;
        parents.extend(pins);
        if let Some(expected) = &entry.before {
            let mut file = platform::open_file(&path, true, false)?;
            if hash(&read_locked(&mut file)?) != *expected {
                return Err(AppError::new(
                    "APPLY_CONFLICT",
                    format!(
                        "{} has newer changes. They have been preserved.",
                        entry.path
                    ),
                ));
            }
            // DELETE access is needed only when disposing a file, obtained through the same file opener.
            locked.push(Some(file));
        } else {
            if std::fs::symlink_metadata(&path).is_ok() {
                return Err(AppError::new(
                    "APPLY_CONFLICT",
                    format!("{} already exists. It has been preserved.", entry.path),
                ));
            }
            locked.push(None);
        }
        if let Some(after) = &entry.after {
            store.bytes(after)?;
        }
    }
    let mut journal = Journal {
        id: id(),
        task_id: task.id.clone(),
        project_id: task.project_id.clone(),
        identity: root.identity.clone(),
        state: "recovery_required".into(),
        undo,
        recovery_of,
        entries,
    };
    store.put("journals", &journal.id, &journal)?;
    // Any error beyond this point keeps a recovery-required durable journal.
    for (i, locked_file) in locked.iter_mut().enumerate() {
        let entry = &journal.entries[i];
        if locked_file.is_none() && entry.after.is_some() {
            let (path, pins) = root.pin_parent(&entry.path, true)?;
            parents.extend(pins);
            *locked_file = Some(platform::open_file(&path, true, true)?);
        }
        if let Some(after) = &entry.after {
            let bytes = store.bytes(after)?;
            let file = locked_file.as_mut().expect("reserved file");
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&bytes)?;
            file.set_len(bytes.len() as u64)?;
            file.sync_all()?;
            if hash(&read_locked(file)?) != *after {
                return Err(AppError::new(
                    "WRITE_VERIFY",
                    "The file did not match the proposed content. Recovery is required.",
                ));
            }
        } else if let Some(file) = locked_file.as_ref() {
            delete_locked(file)?;
        }
        #[cfg(test)]
        FAIL_AFTER.with(|n| {
            if n.get() == i as isize {
                Err(AppError::new(
                    "INJECTED_CRASH",
                    "Injected interruption after a file write.",
                ))
            } else {
                Ok(())
            }
        })?;
        journal.entries[i].done = true;
        store.put("journals", &journal.id, &journal)?;
    }
    drop(locked);
    drop(parents);
    journal.state = "complete".into();
    store.put("journals", &journal.id, &journal)?;
    Ok(journal)
}

#[cfg(test)]
thread_local! { static FAIL_AFTER:std::cell::Cell<isize>=const {std::cell::Cell::new(-1)}; }

pub fn recover(store: &Store, project: &Project, task: &Task) -> Result<&'static str> {
    let journals: Vec<Journal> = store.list("journals")?;
    let mut journal = journals
        .iter()
        .rev()
        .find(|j| j.task_id == task.id && j.recovery_of.is_none() && j.state == "recovery_required")
        .cloned()
        .ok_or_else(|| {
            AppError::new(
                "NO_RECOVERY",
                "No interrupted file operation needs recovery.",
            )
        })?;
    let root = ConfinedRoot::open(Path::new(&project.path))?;
    root.verify(&journal.identity)?;
    let mut entries = Vec::new();
    for e in &journal.entries {
        let actual = match root.read(&e.path, 20 * 1024 * 1024) {
            Ok(b) => Some(store.blob(&b)?),
            Err(_) if !root.path().join(&e.path).exists() => None,
            Err(e) => return Err(e),
        };
        if actual != e.before && actual != e.after {
            return Err(AppError::new("RECOVERY_CONFLICT", format!("{} contains unrecognized or partial content. It is preserved. Export recovery files to review it manually.",e.path)));
        }
        if actual != e.before {
            entries.push(JournalEntry {
                path: e.path.clone(),
                before: actual,
                after: e.before.clone(),
                done: false,
            });
        }
    }
    if !entries.is_empty() {
        apply_entries(store, &root, task, entries, true, Some(journal.id.clone()))?;
    }
    // Restore the root operation's BEFORE state, not an interrupted recovery's
    // intermediate state. Keep the root pending until every auxiliary is closed.
    for mut auxiliary in journals.into_iter().filter(|j| {
        j.recovery_of.as_deref() == Some(journal.id.as_str()) && j.state == "recovery_required"
    }) {
        auxiliary.state = "recovered".into();
        store.put("journals", &auxiliary.id, &auxiliary)?;
    }
    journal.state = "recovered".into();
    store.put("journals", &journal.id, &journal)?;
    Ok(if journal.undo { "applied" } else { "proposed" })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Store, Project, Task) {
        let original = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let store = Store::open(data.path()).unwrap();
        std::fs::write(original.path().join("a.txt"), b"user dirty\r\n").unwrap();
        let root = ConfinedRoot::open(original.path()).unwrap();
        let p = Project {
            id: id(),
            name: "test".into(),
            path: original.path().to_string_lossy().into(),
            identity: root.identity.clone(),
            stack: "other".into(),
            mode: Mode::Project,
            revision: 1,
            opened_at: now(),
            files: vec![],
            exclusions: vec![],
            git: None,
            recipes: vec![],
        };
        let snapshot = capture(&store, &p, &tokio_util::sync::CancellationToken::new()).unwrap();
        let t = Task {
            id: id(),
            project_id: p.id.clone(),
            request: "edit".into(),
            preset: "task".into(),
            state: "review_ready".into(),
            change_state: "proposed".into(),
            revision: 1,
            mode: Mode::Project,
            created_at: now(),
            finished_at: None,
            model: None,
            route: Route {
                level: 1,
                score: 0,
                strategy: "fast".into(),
                reasons: vec![],
                estimated_files: Some(1),
            },
            summary: String::new(),
            warnings: vec![],
            events: vec![],
            checks: vec![],
            approval: None,
            turns: 0,
            tool_calls: 0,
            commands: 0,
            generated_tokens: 0,
            model_eval_ms: 0.0,
            inputs: vec![],
            automatic_escalation: true,
            stages: vec![],
            continuation: None,
            metrics: TaskMetrics::default(),
            parent_task_id: None,
            snapshot_id: Some(snapshot.id),
        };
        (original, data, store, p, t)
    }
    #[test]
    fn apply_undo_preserves_dirty_baseline_and_bytes() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent\r\n")
            .unwrap();
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"user dirty\r\n"
        );
        transact(&store, &p, &t, false, None).unwrap();
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"agent\r\n"
        );
        transact(&store, &p, &t, true, None).unwrap();
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"user dirty\r\n"
        );
    }
    #[test]
    fn apply_conflict_and_undo_conflict_preserve_newer_work() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        std::fs::write(original.path().join("a.txt"), b"newer").unwrap();
        assert!(transact(&store, &p, &t, false, None).is_err());
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"newer"
        );
    }
    #[test]
    fn undo_does_not_replace_post_apply_edits() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        transact(&store, &p, &t, false, None).unwrap();
        std::fs::write(original.path().join("a.txt"), b"new user edit").unwrap();
        assert!(transact(&store, &p, &t, true, None).is_err());
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"new user edit"
        );
    }
    #[test]
    fn new_and_deleted_files_round_trip() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.write_new("new/sub.txt", b"new").unwrap();
        let (path, _pins) = copy.pin_parent("a.txt", false).unwrap();
        let file = platform::open_file(&path, true, false).unwrap();
        delete_locked(&file).unwrap();
        drop(file);
        transact(&store, &p, &t, false, None).unwrap();
        assert!(!original.path().join("a.txt").exists());
        assert_eq!(
            std::fs::read(original.path().join("new/sub.txt")).unwrap(),
            b"new"
        );
        transact(&store, &p, &t, true, None).unwrap();
        assert!(!original.path().join("new/sub.txt").exists());
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"user dirty\r\n"
        );
    }
    #[test]
    fn interrupted_apply_recovers_known_bytes() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        FAIL_AFTER.with(|n| n.set(0));
        assert!(transact(&store, &p, &t, false, None).is_err());
        FAIL_AFTER.with(|n| n.set(-1));
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"agent"
        );
        recover(&store, &p, &t).unwrap();
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"user dirty\r\n"
        );
    }
    #[test]
    fn recovery_preserves_unknown_partial_content() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        FAIL_AFTER.with(|n| n.set(0));
        assert!(transact(&store, &p, &t, false, None).is_err());
        FAIL_AFTER.with(|n| n.set(-1));
        std::fs::write(
            original.path().join("a.txt"),
            b"unknown partial or newer edit",
        )
        .unwrap();
        assert_eq!(
            recover(&store, &p, &t).unwrap_err().code,
            "RECOVERY_CONFLICT"
        );
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"unknown partial or newer edit"
        );
    }
    #[test]
    fn interrupted_undo_recovery_restores_applied_state() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        transact(&store, &p, &t, false, None).unwrap();
        FAIL_AFTER.with(|n| n.set(0));
        assert!(transact(&store, &p, &t, true, None).is_err());
        FAIL_AFTER.with(|n| n.set(-1));
        assert_eq!(recover(&store, &p, &t).unwrap(), "applied");
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"agent"
        );
        assert!(!store
            .list::<Journal>("journals")
            .unwrap()
            .iter()
            .any(|j| j.state == "recovery_required"));
    }
    #[test]
    fn interrupted_recovery_keeps_original_operation_as_anchor() {
        let (original, _data, store, p, t) = fixture();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        FAIL_AFTER.with(|n| n.set(0));
        assert!(transact(&store, &p, &t, false, None).is_err());
        assert!(recover(&store, &p, &t).is_err());
        FAIL_AFTER.with(|n| n.set(-1));
        assert_eq!(recover(&store, &p, &t).unwrap(), "proposed");
        assert_eq!(
            std::fs::read(original.path().join("a.txt")).unwrap(),
            b"user dirty\r\n"
        );
        assert!(!store
            .list::<Journal>("journals")
            .unwrap()
            .iter()
            .any(|j| j.state == "recovery_required"));
    }
    #[test]
    fn restart_reconciles_committed_files_with_unfinished_ui_state() {
        let (_original, data, store, p, mut t) = fixture();
        store.put("projects", &p.id, &p).unwrap();
        let copy =
            ConfinedRoot::open(&working_path(&store, t.snapshot_id.as_ref().unwrap()).unwrap())
                .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"agent")
            .unwrap();
        t.change_state = "applying".into();
        store.put("tasks", &t.id, &t).unwrap();
        let core = crate::agent::AppCore::open(data.path()).unwrap();
        assert_eq!(
            core.store.get::<Task>("tasks", &t.id).unwrap().change_state,
            "proposed"
        );
        transact(&store, &p, &t, false, None).unwrap();
        store.put("tasks", &t.id, &t).unwrap();
        let core = crate::agent::AppCore::open(data.path()).unwrap();
        assert_eq!(
            core.store.get::<Task>("tasks", &t.id).unwrap().change_state,
            "applied"
        );
        transact(&store, &p, &t, true, None).unwrap();
        t.change_state = "undoing".into();
        store.put("tasks", &t.id, &t).unwrap();
        let core = crate::agent::AppCore::open(data.path()).unwrap();
        assert_eq!(
            core.store.get::<Task>("tasks", &t.id).unwrap().change_state,
            "undone"
        );
    }
    #[test]
    fn changing_gitignore_cannot_hide_a_change() {
        let (original, _data, store, p, _t) = fixture();
        std::fs::write(original.path().join(".gitignore"), "hidden.txt\n").unwrap();
        let snap = capture(&store, &p, &tokio_util::sync::CancellationToken::new()).unwrap();
        let copy = ConfinedRoot::open(&working_path(&store, &snap.id).unwrap()).unwrap();
        copy.replace(".gitignore", &hash(b"hidden.txt\n"), b"a.txt\nhidden.txt\n")
            .unwrap();
        copy.replace("a.txt", &hash(b"user dirty\r\n"), b"changed")
            .unwrap();
        let changes = deltas(&store, &snap).unwrap();
        assert!(changes.iter().any(|d| d.path == "a.txt"));
        assert!(changes.iter().any(|d| d.path == ".gitignore"));
    }
}
