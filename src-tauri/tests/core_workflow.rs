use local_agent_core::{agent::AppCore, domain::*, fs::ConfinedRoot, handoff, projects, snapshots};
use std::{
    path::Path,
    time::{Duration, Instant},
};

async fn wait_for(core: &AppCore, id: &str, predicate: impl Fn(&Task) -> bool) -> Task {
    let start = Instant::now();
    loop {
        let task: Task = core.store.get("tasks", id).unwrap();
        if predicate(&task) {
            return task;
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "task did not reach expected state: {} {}",
            task.state,
            task.summary
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
fn node_project(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
        path.join("package.json"),
        r#"{"name":"fixture","version":"1.0.0","scripts":{"test":"node --test test.cjs"}}"#,
    )
    .unwrap();
    std::fs::write(path.join("test.cjs"),"const {test}=require('node:test');const assert=require('node:assert/strict');test('proof',()=>assert.equal(2+2,4));").unwrap();
}

#[tokio::test]
async fn deterministic_tests_permission_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let project_path = temp.path().join("project");
    node_project(&project_path);
    let core = AppCore::open(&temp.path().join("data")).unwrap();
    let project = core.add_project(&project_path).unwrap();
    let task = core
        .start(
            &project.id,
            "Run project tests",
            "tests",
            None,
            Mode::Assisted,
            None,
        )
        .unwrap();
    let waiting = wait_for(&core, &task.id, |t| t.approval.is_some()).await;
    assert!(waiting
        .approval
        .as_ref()
        .unwrap()
        .detail
        .contains("Windows account"));
    core.answer(&task.id, &waiting.approval.unwrap().id, "once")
        .unwrap();
    let done = wait_for(&core, &task.id, |t| !t.active()).await;
    assert_eq!(done.state, "review_ready", "{}", done.summary);
    assert_eq!(done.checks.len(), 1);
    assert_eq!(done.checks[0].status, "passed", "{}", done.checks[0].output);
    assert_eq!(done.checks[0].exit_code, Some(0));
    assert_eq!(done.commands, 1);
    assert!(done.tool_calls > 0);
    let prompt = handoff::preview(&core.store, &task.id).unwrap();
    assert!(prompt.markdown.contains("ARE NOT"));
    assert!(prompt.markdown.contains("passed"));
    assert!(!prompt
        .markdown
        .contains(&project_path.to_string_lossy().to_string()));
    let exported = handoff::export(&core.store, &prompt.id, &temp.path().join("data")).unwrap();
    assert!(Path::new(&exported).join("MANIFEST.json").is_file());
    drop(core);
    let core = AppCore::open(&temp.path().join("data")).unwrap();
    let persisted: Task = core.store.get("tasks", &task.id).unwrap();
    assert_eq!(persisted.state, "review_ready");
    assert_eq!(persisted.checks[0].status, "passed");
}
#[tokio::test]
async fn denial_cancellation_and_single_task_lease() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("project");
    node_project(&path);
    let core = AppCore::open(&temp.path().join("data")).unwrap();
    let p = core.add_project(&path).unwrap();
    let task = core
        .start(&p.id, "Run tests", "tests", None, Mode::Project, None)
        .unwrap();
    let waiting = wait_for(&core, &task.id, |t| t.approval.is_some()).await;
    assert!(core
        .start(&p.id, "another", "task", None, Mode::Project, None)
        .is_err());
    let aid = waiting.approval.unwrap().id;
    core.answer(&task.id, &aid, "deny").unwrap();
    let done = wait_for(&core, &task.id, |t| !t.active()).await;
    assert_eq!(done.commands, 0);
    assert_eq!(done.state, "failed");
    assert!(core.answer(&task.id, &aid, "once").is_err());
    let task = core
        .start(&p.id, "Run tests", "tests", None, Mode::Project, None)
        .unwrap();
    wait_for(&core, &task.id, |t| t.approval.is_some()).await;
    core.cancel(&task.id).unwrap();
    let done = wait_for(&core, &task.id, |t| !t.active()).await;
    assert_eq!(done.state, "cancelled");
    assert_eq!(done.commands, 0);
}
#[tokio::test]
async fn deterministic_search_and_sensitive_routing_need_no_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("project");
    node_project(&path);
    std::fs::write(path.join(".env"), "API_KEY=do-not-read").unwrap();
    let core = AppCore::open(&temp.path().join("data")).unwrap();
    let p = core.add_project(&path).unwrap();
    assert!(!p.files.iter().any(|p| p == ".env"));
    let task = core
        .start(&p.id, "proof", "search", None, Mode::Observe, None)
        .unwrap();
    let done = wait_for(&core, &task.id, |t| !t.active()).await;
    assert_eq!(done.state, "review_ready");
    assert_eq!(done.turns, 0);
    assert!(done
        .events
        .iter()
        .any(|e| e.title.contains("search matches")));
    let task = core
        .start(
            &p.id,
            "Change authentication and oauth policy",
            "task",
            None,
            Mode::Project,
            None,
        )
        .unwrap();
    let done = wait_for(&core, &task.id, |t| !t.active()).await;
    assert_eq!(done.state, "escalation_ready");
    assert_eq!(done.turns, 0);
    assert!(done.snapshot_id.is_none());
}

#[test]
fn dirty_git_index_and_ignored_tracked_file_are_preserved() {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let Some(git) = local_agent_core::commands::find_program("git.exe") else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("project");
    std::fs::create_dir(&path).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(&git)
            .current_dir(&path)
            .creation_flags(0x08000000)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&["init"]);
    std::fs::write(path.join("a.txt"), "staged").unwrap();
    run(&["add", "a.txt"]);
    std::fs::write(path.join("a.txt"), "user dirty").unwrap();
    std::fs::write(path.join(".gitignore"), "a.txt\n").unwrap();
    let core = AppCore::open(&temp.path().join("data")).unwrap();
    let p = core.add_project(&path).unwrap();
    assert!(p.files.iter().any(|f| f == "a.txt"));
    let index = std::fs::read(path.join(".git/index")).unwrap();
    let snapshot =
        snapshots::capture(&core.store, &p, &tokio_util::sync::CancellationToken::new()).unwrap();
    let copy =
        ConfinedRoot::open(&snapshots::working_path(&core.store, &snapshot.id).unwrap()).unwrap();
    copy.replace("a.txt", &hash(b"user dirty"), b"agent")
        .unwrap();
    let task = Task {
        id: id(),
        project_id: p.id.clone(),
        request: "edit".into(),
        preset: "task".into(),
        state: "review_ready".into(),
        change_state: "proposed".into(),
        revision: 1,
        mode: Mode::Project,
        created_at: now(),
        finished_at: Some(now()),
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
    snapshots::transact(
        &core.store,
        &p,
        &task,
        false,
        projects::git(&path, &core.store.settings()),
    )
    .unwrap();
    assert_eq!(std::fs::read(path.join("a.txt")).unwrap(), b"agent");
    assert_eq!(std::fs::read(path.join(".git/index")).unwrap(), index);
    snapshots::transact(
        &core.store,
        &p,
        &task,
        true,
        projects::git(&path, &core.store.settings()),
    )
    .unwrap();
    assert_eq!(std::fs::read(path.join("a.txt")).unwrap(), b"user dirty");
    assert_eq!(std::fs::read(path.join(".git/index")).unwrap(), index);
}
