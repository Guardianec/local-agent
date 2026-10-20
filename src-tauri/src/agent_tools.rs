use super::*;

pub(super) fn rename_exact(root: &ConfinedRoot, args: &tools::RenameArgs) -> Result<()> {
    security::relative_path(&args.path)?;
    security::relative_path(&args.destination)?;
    if args.path.eq_ignore_ascii_case(&args.destination) {
        return Err(AppError::new(
            "RENAME_PATH",
            "Choose a different filename. Case-only renames are not supported.",
        ));
    }
    let (source, _pins) = root.pin_parent(&args.path, false)?;
    let source = crate::platform::open_file(&source, true, false)?;
    let bytes = root_read_locked(&source)?;
    if hash(&bytes) != args.expected_hash || !security::editable(&bytes) {
        return Err(AppError::new(
            "EDIT_CONFLICT",
            "The source changed after approval; no rename was performed.",
        ));
    }
    root.write_new(&args.destination, &bytes)?;
    if let Err(error) = snapshots::delete_locked(&source) {
        let rollback = (|| {
            let (path, _pins) = root.pin_parent(&args.destination, false)?;
            let file = crate::platform::open_file(&path, true, false)?;
            if hash(&root_read_locked(&file)?) != args.expected_hash {
                return Err(AppError::new(
                    "EDIT_CONFLICT",
                    "The destination changed during rename rollback.",
                ));
            }
            snapshots::delete_locked(&file)
        })();
        if rollback.is_err() {
            return Err(AppError::new("RENAME_INCOMPLETE","The source was preserved, but its new copy could not be removed. Review both paths in the retained proposal."));
        }
        return Err(error);
    }
    Ok(())
}

impl AppCore {
    pub(super) async fn execute_tool(
        &self,
        task: &mut Task,
        project: &Project,
        root: &ConfinedRoot,
        name: &str,
        args: Value,
        session: &mut Session,
    ) -> Result<Value> {
        session.check(task)?;
        if task.tool_calls >= 40 {
            return Err(AppError::new(
                "LOOP_LIMIT",
                "This task reached its tool-call limit.",
            ));
        }
        task.tool_calls += 1;
        if project.browser_only
            && !matches!(
                name,
                "browser_open"
                    | "browser_action"
                    | "finish_task"
                    | "request_escalation"
                    | "browser_check_page"
                    | "web_fetch"
                    | "browser_submit"
                    | "request_browser"
            )
        {
            return Err(AppError::new(
                "BROWSER_WORKSPACE",
                "Browser-only tasks cannot access project files or run host commands.",
            ));
        }
        if task.intent == Intent::Converse
            || (task.intent == Intent::Inspect
                && !matches!(
                    name,
                    "inspect_project"
                        | "read_file"
                        | "search_text"
                        | "search_files"
                        | "list_directory"
                        | "git_status"
                        | "git_diff"
                        | "run_project_command"
                        | "run_tests"
                        | "finish_task"
                        | "request_escalation"
                        | "web_fetch"
                        | "browser_open"
                        | "browser_action"
                        | "request_browser"
                ))
        {
            return Err(AppError::new(
                "READ_ONLY",
                "This interaction does not authorize that capability.",
            ));
        }
        let call = tools::parse(name, args)?;
        task.event(
            "tool",
            match name {
                "read_file" => "Reading project file",
                "search_text" => "Searching project",
                "apply_patch" => "Preparing file edit",
                "create_file" => "Preparing new file",
                "run_project_command" | "run_tests" => "Preparing project check",
                "browser_open" | "request_browser" => "Opening browser",
                "browser_action" => "Using browser",
                "browser_submit" => "Preparing form submission",
                "web_fetch" => "Reading webpage",
                "start_dev_server" => "Starting development server",
                "stop_dev_server" => "Stopping development server",
                _ => name,
            },
            "",
        );
        self.save(task)?;
        let value = match call {
            Call::Inspect => {
                json!({"stack":project.stack,"files":root.list()?.0.into_iter().take(150).collect::<Vec<_>>(),"recipes":project.recipes,"exclusions":project.exclusions})
            }
            Call::List(a) => {
                let prefix = if a.path.is_empty() {
                    String::new()
                } else {
                    format!("{}/", security::relative_path(&a.path)?)
                };
                let list = root
                    .list()?
                    .0
                    .into_iter()
                    .filter(|p| p.starts_with(&prefix))
                    .take(200)
                    .collect::<Vec<_>>();
                json!({"entries":list})
            }
            Call::Files(a) => {
                if a.pattern.len() > 512 {
                    return Err(AppError::new(
                        "SEARCH_LIMIT",
                        "Use a shorter filename query.",
                    ));
                }
                let needle = a.pattern.to_lowercase();
                let hits = root
                    .list()?
                    .0
                    .into_iter()
                    .filter(|p| p.to_lowercase().contains(&needle))
                    .take(100)
                    .collect::<Vec<_>>();
                json!({"files":hits})
            }
            Call::Search(a) => {
                if a.query.is_empty() || a.query.len() > 512 {
                    return Err(AppError::new(
                        "SEARCH_LIMIT",
                        "Search text must be 1–512 characters.",
                    ));
                }
                let expression = if a.regex.unwrap_or(false) {
                    a.query.clone()
                } else {
                    regex::escape(&a.query)
                };
                let re = regex::RegexBuilder::new(&expression)
                    .size_limit(1024 * 1024)
                    .build()
                    .map_err(|e| {
                        AppError::new("SEARCH_PATTERN", "This search pattern is invalid.").detail(e)
                    })?;
                let start = Instant::now();
                let mut hits = Vec::new();
                let mut truncated = false;
                'files: for path in root.list()?.0 {
                    if start.elapsed() > Duration::from_secs(5) {
                        truncated = true;
                        break;
                    }
                    let bytes = match root.read(&path, 512 * 1024) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let text = match std::str::from_utf8(&bytes) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    for (n, line) in text.lines().enumerate() {
                        if re.is_match(line) {
                            hits.push(json!({"path":path,"line":n+1,"text":security::bounded(&security::redact(line),500),"hash":hash(&bytes)}));
                            if hits.len() >= 100 {
                                truncated = true;
                                break 'files;
                            }
                        }
                    }
                }
                task.event(
                    "result",
                    format!("{} search matches", hits.len()),
                    security::bounded(&serde_json::to_string_pretty(&hits)?, 24000),
                );
                json!({"summary":format!("{} matches{}.",hits.len(),if truncated{" (results limited)"}else{""}),"matches":hits,"truncated":truncated})
            }
            Call::Read(a) => {
                let bytes = root.read(&a.path, 512 * 1024)?;
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| AppError::new("BINARY_FILE", "This file is not UTF-8 text."))?;
                let line = a.start_line.unwrap_or(1).max(1);
                let count = a.line_count.unwrap_or(120).clamp(1, 200);
                let excerpt = text
                    .lines()
                    .skip(line - 1)
                    .take(count)
                    .enumerate()
                    .map(|(i, l)| format!("{}: {l}", line + i))
                    .collect::<Vec<_>>()
                    .join("\n");
                task.event(
                    "result",
                    format!("Read {}", a.path),
                    format!("Lines {line}–{}", line + count - 1),
                );
                json!({"path":a.path,"expectedHash":hash(&bytes),"startLine":line,"totalLines":text.lines().count(),"content":security::bounded(&security::redact(&excerpt),24000),"protected":security::has_secret(text)})
            }
            Call::Patch(a) => {
                self.edit_allowed(task, &a.path)?;
                let before = root.read(&a.path, 512 * 1024)?;
                let after = tools::patch(&before, &a)?;
                self.preflight_budget(task, &a.path, before.len() + after.len())?;
                let preview = similar::TextDiff::from_lines(
                    std::str::from_utf8(&before).unwrap(),
                    std::str::from_utf8(&after).unwrap(),
                )
                .unified_diff()
                .header(&a.path, &a.path)
                .to_string();
                if task.mode == Mode::Assisted {
                    self.approve(
                        task,
                        session,
                        "edit",
                        "Change this file in the task copy?",
                        "Your original project is unchanged until you apply the reviewed proposal.",
                        preview,
                        false,
                    )
                    .await?;
                }
                session.check(task)?;
                root.replace(&a.path, &a.expected_hash, &after)?;
                task.change_state = "proposed".into();
                self.check_scope(task, root)?;
                task.event(
                    "changes",
                    format!("Edited {}", a.path),
                    "Updated the private task copy.",
                );
                json!({"path":a.path,"hash":hash(&after),"status":"ok"})
            }
            Call::Create(a) => {
                self.edit_allowed(task, &a.path)?;
                if a.content.len() > 128 * 1024 || !security::editable(a.content.as_bytes()) {
                    return Err(AppError::new(
                        "FILE_LIMIT",
                        "New file content is too large or contains likely secrets.",
                    ));
                }
                self.preflight_budget(task, &a.path, a.content.len())?;
                if task.mode == Mode::Assisted {
                    self.approve(
                        task,
                        session,
                        "create",
                        "Create a file in the task copy?",
                        &a.path,
                        a.content.clone(),
                        false,
                    )
                    .await?;
                }
                session.check(task)?;
                root.write_new(&a.path, a.content.as_bytes())?;
                task.change_state = "proposed".into();
                self.check_scope(task, root)?;
                task.event(
                    "changes",
                    format!("Created {}", a.path),
                    "Created in the private task copy.",
                );
                json!({"path":a.path,"hash":hash(a.content.as_bytes())})
            }
            Call::Rename(a) => {
                self.edit_allowed(task, &a.path)?;
                self.edit_allowed(task, &a.destination)?;
                let before = root.read(&a.path, 512 * 1024)?;
                if hash(&before) != a.expected_hash || !security::editable(&before) {
                    return Err(AppError::new(
                        "EDIT_CONFLICT",
                        "The source is protected or changed. Read it again before renaming.",
                    ));
                }
                self.preflight_budget(task, &a.destination, before.len() * 2)?;
                let snapshot: Snapshot = self
                    .store
                    .get("snapshots", task.snapshot_id.as_deref().unwrap_or(""))?;
                let mut paths = snapshots::deltas(&self.store, &snapshot)?
                    .into_iter()
                    .map(|d| d.path)
                    .collect::<HashSet<_>>();
                paths.insert(a.path.clone());
                paths.insert(a.destination.clone());
                if paths.len() > if task.route.level == 3 { 6 } else { 12 } {
                    return Err(AppError::new(
                        "SCOPE_LIMIT",
                        "This rename would exceed the task's changed-file limit.",
                    ));
                }
                self.approve(task,session,"rename","Rename this file in the task copy?",&format!("{} → {}",a.path,a.destination),format!("{} → {}\nExact source hash: {}\nContents are copied byte-for-byte; your original project stays unchanged until Apply.",a.path,a.destination,a.expected_hash),false).await?;
                session.check(task)?;
                rename_exact(root, &a)?;
                task.change_state = "proposed".into();
                self.check_scope(task, root)?;
                task.event(
                    "changes",
                    format!("Renamed {} → {}", a.path, a.destination),
                    "Exact source bytes preserved in the private task copy.",
                );
                json!({"status":"renamed","path":a.path,"destination":a.destination,"hash":a.expected_hash})
            }
            Call::Delete(a) => {
                self.edit_allowed(task, &a.path)?;
                let before = root.read(&a.path, 512 * 1024)?;
                if hash(&before) != a.expected_hash || !security::editable(&before) {
                    return Err(AppError::new(
                        "EDIT_CONFLICT",
                        "The file is protected or has changed.",
                    ));
                }
                self.approve(
                    task,
                    session,
                    "delete",
                    "Delete this file from the task copy?",
                    &a.path,
                    String::from_utf8_lossy(&before).into(),
                    false,
                )
                .await?;
                let (path, _pins) = root.pin_parent(&a.path, false)?;
                let file = crate::platform::open_file(&path, true, false)?;
                if hash(&root_read_locked(&file)?) != a.expected_hash {
                    return Err(AppError::new(
                        "EDIT_CONFLICT",
                        "The file changed after approval.",
                    ));
                }
                snapshots::delete_locked(&file)?;
                drop(file);
                task.change_state = "proposed".into();
                json!({"deleted":a.path})
            }
            Call::Recipe(a) => {
                if task.intent == Intent::Converse {
                    return Err(AppError::new(
                        "READ_ONLY",
                        "Conversation cannot execute commands.",
                    ));
                }

                if task.commands >= 6 {
                    return Err(AppError::new(
                        "LOOP_LIMIT",
                        "This task reached its six-command limit.",
                    ));
                }
                let recipe = project
                    .recipes
                    .iter()
                    .find(|r| r.id == a.recipe_id)
                    .ok_or_else(|| {
                        AppError::new("RECIPE_INVALID", "Choose a registered project check.")
                    })?;
                if recipe.kind == "dev_server" {
                    return Err(AppError::new(
                        "RECIPE_INVALID",
                        "Use start_dev_server for a task-owned development server.",
                    ));
                }
                if task.intent == Intent::Inspect
                    && (recipe.script.contains("fix")
                        || !matches!(recipe.kind.as_str(), "test" | "lint"))
                {
                    return Err(AppError::new(
                        "READ_ONLY",
                        "Inspection permits approved tests or non-fixing lint checks only.",
                    ));
                }
                let digest = projects::config_digest(root)?;
                let mut recipe = recipe.clone();
                recipe.config_digest = digest.clone();
                let home = self.store.root.join("tasks").join(&task.id).join("runtime");
                let plan = commands::plan(&recipe, root.path(), &self.store.settings(), &home)?;
                let executable_hash = hash(&std::fs::read(&plan.executable)?);
                let manifest = snapshots::manifest(root)?;
                let mh = snapshots::manifest_hash(&manifest);
                let dependencies =
                    if recipe.adapter == "node" && !root.path().join("node_modules").exists() {
                        let source = std::path::PathBuf::from(&project.path);
                        tokio::task::spawn_blocking(move || crate::environment::inspect(&source))
                            .await
                            .map_err(|e| {
                                AppError::new(
                                    "ENVIRONMENT_FAILED",
                                    "The dependency environment could not be inspected.",
                                )
                                .detail(e)
                            })??
                    } else {
                        None
                    };
                let dependency_note=dependencies.as_ref().map(|d|format!("Prepare test environment: physically copy {} existing dependency files ({:.1} MiB). No links or package installation.",d.files.len(),d.bytes as f64/1048576.0)).unwrap_or_else(||"No dependency preparation required or available; missing modules will be reported.".into());
                let preview=format!("{}\n{}\nExecutable: {}\nExecutable hash: {executable_hash}\nConfiguration hash: {digest}\nWorking folder: private task copy\nNetwork: host access (not isolated)\n{dependency_note}",recipe.name,recipe.description,plan.executable.display());
                self.approve(task,session,"command","Run this project's code on your PC?","Tests and build tools can access files and network services as your Windows account. Only run projects you trust.",preview,true).await?;
                if projects::config_digest(root)? != digest
                    || hash(&std::fs::read(&plan.executable)?) != executable_hash
                {
                    return Err(AppError::new(
                        "RECIPE_CHANGED",
                        "The check changed after preview. Review a fresh permission request.",
                    ));
                }
                if let Some(dependencies) = dependencies {
                    task.event(
                        "activity",
                        "Preparing independent test dependencies",
                        dependency_note,
                    );
                    self.save(task)?;
                    let destination = root.path().to_path_buf();
                    let token = session.cancel.clone();
                    tokio::task::spawn_blocking(move || {
                        crate::environment::prepare(dependencies, &destination, &token)
                    })
                    .await
                    .map_err(|e| {
                        AppError::new(
                            "ENVIRONMENT_FAILED",
                            "Dependency preparation stopped unexpectedly.",
                        )
                        .detail(e)
                    })??;
                }
                session.check(task)?;
                task.commands += 1;
                task.event(
                    "activity",
                    format!("Running {}", recipe.name),
                    recipe.description.clone(),
                );
                self.save(task)?;
                let token = session.cancel.clone();
                let result = tokio::task::spawn_blocking(move || commands::execute(plan, token))
                    .await
                    .map_err(|e| {
                        AppError::new("COMMAND_FAILED", "The command worker stopped unexpectedly.")
                            .detail(e)
                    })??;
                let status = if result.status != "completed" {
                    result.status.clone()
                } else if result.code == Some(0) {
                    "passed".into()
                } else {
                    "failed".into()
                };
                let (passed, failed) = parse_test_counts(&result.output);
                let check = Check {
                    id: id(),
                    recipe_id: recipe.id.clone(),
                    name: recipe.name.clone(),
                    status: status.clone(),
                    exit_code: result.code,
                    output: result.output.clone(),
                    manifest_hash: mh.clone(),
                    duration_ms: result.duration_ms,
                    passed,
                    failed,
                    outcome_status: Some(status.clone()),
                };
                if status == "failed"
                    && task
                        .checks
                        .iter()
                        .any(|c| c.recipe_id == recipe.id && c.outcome() == "failed")
                {
                    task.metrics.retries += 1;
                }
                task.checks.push(check);
                task.event(
                    "validation",
                    format!("{}: {}", recipe.name, status),
                    security::bounded(&result.output, 24000),
                );
                self.check_scope(task, root)?;
                self.save(task)?;
                if status == "cancelled" {
                    return Err(AppError::new("CANCELLED", "The task was stopped."));
                }
                if status != "passed" {
                    let f = hash(format!("{mh}:{}", result.output).as_bytes());
                    let n = session.failures.entry(f).or_default();
                    *n += 1;
                    if *n >= 2 {
                        return Err(AppError::new("NO_PROGRESS","The same check failed twice without a meaningful change. Prepare a handoff."));
                    }
                }
                json!({"status":status,"exitCode":result.code,"output":security::bounded(&result.output,10000),"manifestHash":mh})
            }
            Call::Git => {
                task.event(
                    "result",
                    "Existing project changes",
                    project
                        .git
                        .as_ref()
                        .map(|g| g.status.clone())
                        .unwrap_or_else(|| {
                            "This folder is not a Git repository, or Git is unavailable.".into()
                        }),
                );
                json!({"git":project.git})
            }
            Call::Diff => {
                let snapshot: Snapshot = self
                    .store
                    .get("snapshots", task.snapshot_id.as_deref().unwrap_or(""))?;
                json!({"changes":snapshots::deltas(&self.store,&snapshot)?.into_iter().map(|d|json!({"path":d.path,"additions":d.additions,"deletions":d.deletions})).collect::<Vec<_>>()})
            }
            Call::Finish(a) => {
                if a.summary.trim().is_empty() {
                    return Err(AppError::new(
                        "INVALID_DATA",
                        "Finish with a nonempty answer.",
                    ));
                }
                task.summary = security::bounded(&security::redact(&a.summary), 12000);
                task.event("result", "Agent notes (unverified)", task.summary.clone());
                task.warnings.extend(
                    a.unresolved
                        .into_iter()
                        .take(20)
                        .map(|s| security::bounded(&security::redact(&s), 1000)),
                );
                task.transition("validating");
                json!({"status":"completion_requested"})
            }
            Call::Escalate(a) => {
                task.summary = security::bounded(&security::redact(&a.reason), 4000);
                task.transition("routing");
                json!({"status":"escalation_requested"})
            }
            Call::StopDevServer => {
                let server = session.dev_server.take().ok_or_else(|| {
                    AppError::new(
                        "DEV_SERVER_MISSING",
                        "No app-owned development server is running.",
                    )
                })?;
                server.close().await?;
                task.event("activity","Development server stopped","The app-owned process tree was stopped. External servers are never terminated.");
                json!({"status":"stopped"})
            }
            Call::StartDevServer(args) => {
                if task.commands >= 6 || session.dev_server.is_some() {
                    return Err(AppError::new(
                        "DEV_SERVER_LIMIT",
                        "Use at most one development server within the task's command budget.",
                    ));
                }
                let recipe = project
                    .recipes
                    .iter()
                    .find(|r| r.id == args.recipe_id && r.kind == "dev_server")
                    .ok_or_else(|| {
                        AppError::new(
                            "RECIPE_INVALID",
                            "Select a registered npm dev/start recipe.",
                        )
                    })?;
                let digest = projects::config_digest(root)?;
                let source_hash = snapshots::manifest_hash(&snapshots::manifest(root)?);
                let home = self.store.root.join("tasks").join(&task.id).join("runtime");
                let plan = commands::plan(recipe, root.path(), &self.store.settings(), &home)?;
                let executable_hash = hash(&std::fs::read(&plan.executable)?);
                let dependencies = if !root.path().join("node_modules").exists() {
                    let source = std::path::PathBuf::from(&project.path);
                    tokio::task::spawn_blocking(move || crate::environment::inspect(&source))
                        .await
                        .map_err(|e| {
                            AppError::new("ENVIRONMENT_FAILED", "Dependency inspection stopped.")
                                .detail(e)
                        })??
                } else {
                    None
                };
                self.approve(task,session,"command","Start this project's development server?","This runs approved project code as your Windows account in the private task copy. It has host file/network access. The app assigns a loopback port, supervises the process tree, and stops it with this task.",format!("{}\n{}\nConfiguration hash: {digest}\nSource manifest: {source_hash}\nExecutable hash: {executable_hash}\nExisting dependencies may be copied without links. No packages are installed.",recipe.name,recipe.description),false).await?;
                if projects::config_digest(root)? != digest
                    || snapshots::manifest_hash(&snapshots::manifest(root)?) != source_hash
                    || hash(&std::fs::read(&plan.executable)?) != executable_hash
                {
                    return Err(AppError::new(
                        "RECIPE_CHANGED",
                        "Development recipe changed after review.",
                    ));
                }
                if let Some(dependencies) = dependencies {
                    let destination = root.path().to_path_buf();
                    let token = session.cancel.clone();
                    tokio::task::spawn_blocking(move || {
                        crate::environment::prepare(dependencies, &destination, &token)
                    })
                    .await
                    .map_err(|e| {
                        AppError::new("ENVIRONMENT_FAILED", "Dependency preparation stopped.")
                            .detail(e)
                    })??;
                }
                task.commands += 1;
                let server =
                    crate::dev_server::DevServer::start(plan, &recipe.description, &session.cancel)
                        .await?;
                let url = server.url.clone();
                session.dev_server = Some(server);
                task.event("activity", "Development server ready", &url);
                json!({"status":"ready","url":url,"message":"App-owned development server is running in the private task copy. Open with browser_open using local_test."})
            }
            Call::BrowserSubmit(args) => {
                if task.mode == Mode::Observe {
                    return Err(AppError::new(
                        "BROWSER_EFFECT",
                        "Observe mode cannot submit external forms.",
                    ));
                }
                if crate::browser_session::forbids_submission(&task.request) {
                    return Err(AppError::new(
                        "BROWSER_EFFECT",
                        "The user prohibited submission in this task.",
                    ));
                }
                let browser = session
                    .browser
                    .as_mut()
                    .ok_or_else(|| AppError::new("BROWSER_SESSION", "Open a browser first."))?;
                let prepared = browser
                    .execute(crate::browser_session::BrowserAction::PrepareSubmit {
                        element: args.element,
                    })
                    .await?;
                let form:crate::browser_session::PreparedForm=serde_json::from_value(prepared["observation"]["submission"].clone()).map_err(|_|AppError::new("BROWSER_FORM","A supported form could not be prepared. Refresh the snapshot and select its submit button."))?;
                let request = form.request(&browser.spec.profile, browser.allows(&form.url))?;
                let origin = reqwest::Url::parse(&form.url)
                    .unwrap()
                    .origin()
                    .ascii_serialization();
                let spec = browser.spec.clone();
                self.approve(task,session,"browser_submit","Submit this form to the website?","This sends the exact reviewed fields once and may change remote data. Project Undo cannot reverse it. Cookies and credentials are not sent.",serde_json::to_string_pretty(&form)?,false).await?;
                task.event("activity", "Submitting reviewed form", &origin);
                self.save(task)?;
                let response =
                    crate::browser::fetch_approved(&request, &origin, &spec, &session.cancel, true)
                        .await?;
                let receipt = json!({"status":"sent","origin":origin,"httpStatus":response["status"],"redirect":response["redirect"],"message":"The reviewed HTTP request was sent once. The HTTP response alone does not prove the site's business operation succeeded. The browser page is unchanged; do not retry automatically."});
                task.event(
                    "result",
                    "Reviewed form request sent",
                    serde_json::to_string(&receipt)?,
                );
                receipt
            }
            Call::WebFetch(args) => {
                if !crate::browser_session::requested_urls(&task.request).contains(&args.url)
                    && !session.browser.as_ref().is_some_and(|b| {
                        matches!(b.spec.profile.as_str(), "public_read" | "public_web")
                            && b.allows(&args.url)
                    })
                {
                    return Err(AppError::new("WEB_SCOPE","Read a user-supplied URL or an origin approved in the public browser session."));
                }
                task.event("activity", "Reading public page", &args.url);
                self.save(task)?;
                let result = crate::web::fetch(&args.url, &session.cancel).await?;
                task.event(
                    "result",
                    "Web observation",
                    security::bounded(&serde_json::to_string(&result)?, 12000),
                );
                result
            }
            Call::BrowserOpen(spec) => {
                crate::browser::validate(&spec)?;
                if session.browser.is_some() {
                    let browser = session.browser.as_ref().unwrap();
                    if !spec.steps.is_empty() || spec.profile != browser.spec.profile {
                        return Err(AppError::new(
                            "BROWSER_SESSION",
                            "The existing session retains its profile. Use empty steps.",
                        ));
                    }
                    if !browser.allows(&spec.url) {
                        self.approve(task,session,"browser_origin","Allow this additional browser origin?","This grants navigation and resources on the additional origin for the same isolated session. Existing network and action limits still apply.",spec.url.clone(),false).await?;
                        session
                            .browser
                            .as_mut()
                            .unwrap()
                            .approve_origin(&spec.url)?;
                    }
                    let mut result = session
                        .browser
                        .as_mut()
                        .unwrap()
                        .execute(crate::browser_session::BrowserAction::Navigate { url: spec.url })
                        .await?;
                    self.capture_requested_browser_image(task, session, &mut result)
                        .await?;
                    self.record_browser_result(task, session, &mut result)?;
                    self.save(task)?;
                    return Ok(result);
                }
                if !spec.steps.is_empty()
                    || (name != "request_browser"
                        && !crate::browser_session::requested_urls(&task.request)
                            .contains(&spec.url)
                        && !session
                            .dev_server
                            .as_ref()
                            .is_some_and(|s| s.url == spec.url))
                {
                    return Err(AppError::new("BROWSER_SCOPE", "Open one task session using an exact user-supplied URL and no scenario steps."));
                }
                if spec.profile == "local_test"
                    && ((task.intent != Intent::Execute
                        && !crate::browser_session::local_interaction_requested(&task.request))
                        || task.mode == Mode::Observe)
                {
                    return Err(AppError::new("BROWSER_EFFECT", "Interactive localhost sessions require an execution task and explicit approval."));
                }
                if spec.profile == "local_test"
                    && crate::browser_session::forbids_submission(&task.request)
                {
                    return Err(AppError::new("BROWSER_EFFECT","Use local_read when the user forbids sending form data; interactive disposable-server grants permit server writes."));
                }
                let detail=match spec.profile.as_str() {
                    "local_test"=>"Use only a disposable loopback test app. JavaScript and interactions may change server state; project Undo does not restore those effects. The grant covers this exact origin for this task.",
                    "local_read"=>"Read a disposable loopback application with JavaScript enabled. Only GET/HEAD network requests are allowed; model interactions and submissions are blocked.",
                    "public_web"=>"Read an anonymous public JavaScript application. Only GET/HEAD requests on approved origins are allowed. Model interactions, submissions, accounts and files are blocked.",
                    _=>"Read anonymous static public pages with JavaScript disabled. Harmless form fields may be prepared in an execution task. Submission requires a separate review of the exact fields and destination. Accounts and files are blocked."
                };
                self.approve(
                    task,
                    session,
                    "browser",
                    "Open this isolated browser session?",
                    detail,
                    serde_json::to_string_pretty(&spec)?,
                    false,
                )
                .await?;
                session.browser = Some(crate::browser_session::BrowserSession::start(
                    &self.store,
                    &task.id,
                    spec,
                    &session.cancel,
                )?);
                session.browser_enabled = true;
                task.event(
                    "activity",
                    "Opening browser session",
                    "Starting an isolated browser with the approved origin policy.",
                );
                self.save(task)?;
                let mut result = session
                    .browser
                    .as_mut()
                    .unwrap()
                    .execute(crate::browser_session::BrowserAction::Snapshot)
                    .await?;
                self.capture_requested_browser_image(task, session, &mut result)
                    .await?;
                self.record_browser_result(task, session, &mut result)?;
                result
            }
            Call::BrowserAction(action) => {
                if let crate::browser_session::BrowserAction::AllowResources { urls } = &action {
                    if urls.is_empty() || urls.len() > 8 {
                        return Err(AppError::new(
                            "BROWSER_SCOPE",
                            "Approve one to eight resource origins.",
                        ));
                    }
                    if session.browser.is_none() {
                        return Err(AppError::new("BROWSER_SESSION", "Open a browser first."));
                    }
                    for url in urls {
                        crate::browser::validate(&crate::browser::BrowserSpec {
                            url: url.clone(),
                            profile: "public_read".into(),
                            steps: vec![],
                        })?;
                    }
                    self.approve(task,session,"browser_resources","Allow these public resource origins?","This permits bounded anonymous GET/HEAD subresources. It does not grant page navigation, private-network access, uploads or submissions.",serde_json::to_string_pretty(urls)?,false).await?;
                    session.browser.as_mut().unwrap().approve_resources(urls)?;
                    return Ok(
                        json!({"status":"resources_approved","message":"Reload the page to retry blocked resources. Navigation authority is unchanged."}),
                    );
                }
                if matches!(
                    action,
                    crate::browser_session::BrowserAction::PrepareSubmit { .. }
                ) {
                    return Err(AppError::new(
                        "BROWSER_FORM",
                        "Use browser_submit for native review of a static form.",
                    ));
                }
                if let crate::browser_session::BrowserAction::Navigate { url }
                | crate::browser_session::BrowserAction::OpenTab { url } = &action
                {
                    let browser = session.browser.as_ref().ok_or_else(|| {
                        AppError::new("BROWSER_SESSION", "Open a browser session first.")
                    })?;
                    if !browser.allows(url) {
                        crate::browser::validate(&crate::browser::BrowserSpec {
                            url: url.clone(),
                            profile: browser.spec.profile.clone(),
                            steps: vec![],
                        })?;
                        self.approve(task,session,"browser_origin","Allow this additional browser origin?","This extends page and resource access for the current isolated session. Public sessions still block private addresses and submissions. Local sessions may change the approved disposable server.",url.clone(),false).await?;
                        session.browser.as_mut().unwrap().approve_origin(url)?;
                    }
                }
                if matches!(action, crate::browser_session::BrowserAction::Screenshot)
                    && !self
                        .provider
                        .health(&self.store)
                        .await
                        .models
                        .iter()
                        .any(|m| Some(&m.name) == task.model.as_ref() && m.vision)
                {
                    return Err(AppError::new("MODEL_VISION", "This model does not support browser images. Continue with semantic evidence."));
                }
                if matches!(action, crate::browser_session::BrowserAction::Screenshot)
                    && crate::images::ids(&task.inputs).len() >= crate::images::MAX_IMAGES
                {
                    return Err(AppError::new(
                        "IMAGE_LIMIT",
                        "This task has reached its four-image budget.",
                    ));
                }
                let browser = session.browser.as_mut().ok_or_else(|| {
                    AppError::new("BROWSER_SESSION", "Open a browser session first.")
                })?;
                if ((task.intent != Intent::Execute
                    && !(browser.spec.profile == "local_test"
                        && crate::browser_session::local_interaction_requested(&task.request)))
                    || task.mode == Mode::Observe)
                    && matches!(
                        action,
                        crate::browser_session::BrowserAction::Click { .. }
                            | crate::browser_session::BrowserAction::Fill { .. }
                            | crate::browser_session::BrowserAction::Select { .. }
                            | crate::browser_session::BrowserAction::Check { .. }
                            | crate::browser_session::BrowserAction::Press { .. }
                    )
                {
                    return Err(AppError::new(
                        "BROWSER_EFFECT",
                        "This inspection does not authorize browser interaction.",
                    ));
                }
                task.event(
                    "activity",
                    action.activity_label(),
                    security::bounded(&serde_json::to_string(&action)?, 500),
                );
                self.save(task)?;
                let mut result = browser.execute(action).await?;
                self.record_browser_result(task, session, &mut result)?;
                result
            }
            Call::Browser(spec) => {
                if task.preset != "browser" || session.browser.is_some() {
                    return Err(AppError::new("BROWSER_SESSION","Use browser_open and browser_action in agent tasks. Deterministic scenarios use the Browser checks preset."));
                }
                crate::browser::validate(&spec)?;
                if !task.request.contains(&spec.url) {
                    return Err(AppError::new(
                        "BROWSER_SCOPE",
                        "Only a page address explicitly supplied in the user task can be opened.",
                    ));
                }
                session.browser_actions += spec.steps.len() as u32 + 1;
                if session.browser_actions > 30 {
                    return Err(AppError::new(
                        "LOOP_LIMIT",
                        "This task reached its browser-action limit.",
                    ));
                }
                crate::browser::component(&self.store)?;
                self.approve(task,session,"browser",if spec.profile=="local_test"{"Check this disposable local web app?"}else{"Read this public page?"},if spec.profile=="local_test"{"Use only a disposable test server. The page may write to its server. Project Undo does not restore browser or server actions. Other origins, accounts and credentials are blocked."}else{"Read an unauthenticated page on this exact origin. JavaScript and submissions are disabled; no project contents are sent."},serde_json::to_string_pretty(&spec)?,false).await?;
                task.event("activity", "Running approved browser check", &spec.url);
                self.save(task)?;
                let started = Instant::now();
                let result =
                    crate::browser::run(&self.store, &task.id, &spec, session.cancel.clone())
                        .await?;
                let output = security::bounded(
                    &security::redact(&serde_json::to_string_pretty(&result)?),
                    24000,
                );
                let passed = result["status"] == "passed";
                task.checks.push(Check {
                    id: id(),
                    recipe_id: "browser".into(),
                    name: "Browser scenario".into(),
                    status: if passed { "passed" } else { "failed" }.into(),
                    exit_code: None,
                    output: output.clone(),
                    manifest_hash: String::new(),
                    outcome_status: Some(if passed { "passed" } else { "failed" }.into()),
                    duration_ms: started.elapsed().as_secs_f64() * 1000.0,
                    passed: None,
                    failed: None,
                });
                task.event("result", "Browser observation", output);
                result
            }
        };
        self.save(task)?;
        Ok(value)
    }
    async fn capture_requested_browser_image(
        &self,
        task: &mut Task,
        session: &mut Session,
        result: &mut Value,
    ) -> Result<()> {
        if session.browser_visual_captured
            || !crate::browser_session::wants_browser_image(&task.request)
        {
            return Ok(());
        }
        if crate::images::ids(&task.inputs).len() >= crate::images::MAX_IMAGES {
            return Err(AppError::new(
                "IMAGE_LIMIT",
                "The requested browser image exceeds this task's four-image budget.",
            ));
        }
        if !self
            .provider
            .health(&self.store)
            .await
            .models
            .iter()
            .any(|m| Some(&m.name) == task.model.as_ref() && m.vision)
        {
            return Err(AppError::new(
                "MODEL_VISION",
                "The visual browser task requires a vision-capable local model.",
            ));
        }
        task.event(
            "activity",
            "Capturing requested browser viewport",
            "One image is captured for the explicitly visual task and remains local.",
        );
        self.save(task)?;
        let capture = session
            .browser
            .as_mut()
            .unwrap()
            .execute(crate::browser_session::BrowserAction::Screenshot)
            .await?;
        result["observation"]["screenshot"] = capture["observation"]["screenshot"].clone();
        Ok(())
    }
    fn record_browser_result(
        &self,
        task: &mut Task,
        session: &mut Session,
        result: &mut Value,
    ) -> Result<()> {
        if let Some(encoded) = result["observation"]
            .as_object_mut()
            .and_then(|o| o.remove("screenshot"))
        {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded.as_str().unwrap_or(""))
                .map_err(|_| AppError::new("IMAGE_FORMAT", "Invalid browser image."))?;
            let image = crate::images::ingest(&self.store, "Browser viewport", &bytes, "browser")?;
            task.inputs.push(ContentBlock::Image {
                attachment_id: image.id.clone(),
            });
            result["image_refs"] = json!([image.id]);
            session.browser_visual_captured = true;
        }
        task.event(
            "result",
            "Browser observation",
            security::bounded(&serde_json::to_string(result)?, 26000),
        );
        Ok(())
    }
    fn edit_allowed(&self, task: &Task, path: &str) -> Result<()> {
        let path = security::relative_path(path)?;
        if task.intent != Intent::Execute || task.mode == Mode::Observe || task.preset == "explain"
        {
            return Err(AppError::new(
                "READ_ONLY",
                "This task can inspect files but cannot propose edits.",
            ));
        }
        if security::protected(&path) || security::artifact(&path) {
            return Err(AppError::new(
                "PROTECTED_FILE",
                "This file is outside the editable project scope.",
            ));
        }
        let lower = path.to_lowercase();
        let base = lower.rsplit('/').next().unwrap_or("");
        if [
            "auth/",
            "authentication",
            "authorization",
            "migrations/",
            "terraform/",
            "payments/",
            "crypto/",
        ]
        .iter()
        .any(|s| lower.contains(s))
            || [
                "auth.",
                "login.",
                "signin.",
                "password.",
                "security.",
                "crypto.",
            ]
            .iter()
            .any(|s| base.starts_with(s))
        {
            return Err(AppError::new("SENSITIVE_SCOPE","The agent encountered a security- or data-sensitive file. Prepare a broader review handoff."));
        }
        Ok(())
    }
    fn preflight_budget(&self, task: &Task, path: &str, bytes: usize) -> Result<()> {
        let snapshot: Snapshot = self
            .store
            .get("snapshots", task.snapshot_id.as_deref().unwrap_or(""))?;
        let changes = snapshots::deltas(&self.store, &snapshot)?;
        let count = changes.len() + usize::from(!changes.iter().any(|d| d.path == path));
        let total = changes
            .iter()
            .filter(|d| d.path != path)
            .map(|d| d.before.len() + d.after.len())
            .sum::<usize>()
            + bytes;
        if count > if task.route.level == 3 { 6 } else { 12 } || total > 1024 * 1024 {
            return Err(AppError::new(
                "SCOPE_LIMIT",
                "This edit would exceed the task's bounded source scope.",
            ));
        }
        Ok(())
    }
}
