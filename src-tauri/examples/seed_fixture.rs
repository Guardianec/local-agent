use local_agent_core::{agent::AppCore, domain::*, fs::ConfinedRoot, snapshots, tools};
fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo = if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap().to_path_buf()
    } else {
        cwd
    };
    let data = std::env::var_os("LOCAL_AGENT_TEST_DATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo.join(".local-data/ui"));
    let project_path = repo.join(".local-data/ui-project");
    std::fs::create_dir_all(project_path.join("test"))?;
    for file in ["package.json", "math.cjs", "test/math.test.cjs"] {
        std::fs::copy(
            repo.join("fixtures/node-small").join(file),
            project_path.join(file),
        )?;
    }
    let core = AppCore::open(&data)?;
    let project = core.add_project(&project_path)?;
    // Seed an actual proposal via the same snapshot/patch operations used by the agent.
    let snapshot = snapshots::capture(
        &core.store,
        &project,
        &tokio_util::sync::CancellationToken::new(),
    )?;
    let copy = ConfinedRoot::open(&snapshots::working_path(&core.store, &snapshot.id)?)?;
    let before = copy.read("math.cjs", 10000)?;
    let patch = tools::PatchArgs {
        path: "math.cjs".into(),
        expected_hash: hash(&before),
        hunks: vec![tools::Hunk {
            old_text: "return a - b;".into(),
            new_text: "return a + b;".into(),
        }],
    };
    let after = tools::patch(&before, &patch)?;
    copy.replace("math.cjs", &hash(&before), &after)?;
    let mut task=Task{intent:Intent::Execute,intent_reason:String::new(),conversation_context:String::new(),result:None,id:id(),project_id:project.id,request:"Fix the addition function in math.cjs".into(),preset:"task".into(),state:"review_ready".into(),change_state:"proposed".into(),revision:1,mode:Mode::Assisted,created_at:now(),finished_at:Some(now()),model:None,route:Route{level:1,score:0,strategy:"fast".into(),reasons:vec!["A single-file fixture edit".into()],estimated_files:Some(1)},summary:"Replaced subtraction with addition in the fixture function. This fixture proposal was prepared by the native smoke-test utility; no model result is claimed.".into(),warnings:vec!["Tests have not been run for this seeded proposal.".into()],events:vec![],checks:vec![],approval:None,turns:0,tool_calls:0,commands:0,generated_tokens:0,model_eval_ms:0.0,inputs:vec![],automatic_escalation:true,stages:vec![],continuation:None,metrics:TaskMetrics::default(),parent_task_id:None,snapshot_id:Some(snapshot.id)};
    task.event(
        "activity",
        "Creating recoverable task copy",
        "Captured the fixture's actual current files.",
    );
    task.event(
        "changes",
        "Edited math.cjs",
        "Changed one exact expression in the private copy.",
    );
    core.save(&task)?;
    println!("Test data: {}", data.display());
    Ok(())
}
