use crate::{commands, domain::*, fs::ConfinedRoot, security};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn inspect(path: &Path, settings: &Settings) -> Result<Project> {
    let root = ConfinedRoot::open(path)?;
    let (mut files, exclusions) = root.list()?;
    if let Some(tracked) = git_read(path, settings, &["ls-files", "-z"]) {
        for file in tracked.split('\0').filter(|s| !s.is_empty()) {
            if security::relative_path(file).is_ok()
                && !security::protected(file)
                && !security::artifact(file)
                && root.read(file, 20 * 1024 * 1024).is_ok()
            {
                files.push(file.into());
            }
        }
        files.sort();
        files.dedup();
    }
    let stack = if files.iter().any(|p| p == "package.json") {
        "node"
    } else if files.iter().any(|p| {
        matches!(
            p.as_str(),
            "pyproject.toml" | "requirements.txt" | "setup.py"
        )
    }) {
        "python"
    } else {
        "other"
    }
    .to_string();
    let recipes = recipes(&root, &stack)?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Project {
        browser_only: false,
        id: id(),
        name,
        path: path.to_string_lossy().into(),
        identity: root.identity.clone(),
        stack,
        mode: Mode::Assisted,
        revision: 1,
        opened_at: now(),
        files,
        exclusions,
        git: git(path, settings),
        recipes,
    })
}
pub fn config_digest(root: &ConfinedRoot) -> Result<String> {
    let (files, _) = root.list()?;
    let mut digests = Vec::new();
    for p in files.into_iter().filter(|p| {
        let n = p.rsplit('/').next().unwrap_or("");
        matches!(
            n,
            "package.json"
                | "package-lock.json"
                | "pyproject.toml"
                | "requirements.txt"
                | "pytest.ini"
                | "setup.cfg"
                | "tox.ini"
                | ".eslintrc"
                | ".eslintrc.json"
        ) || n.starts_with("eslint.config.")
            || n.starts_with("vitest.config.")
            || n.starts_with("vite.config.")
            || n.starts_with("jest.config.")
    }) {
        digests.push((p.clone(), hash(&root.read(&p, 1024 * 1024)?)));
    }
    Ok(hash(&serde_json::to_vec(&digests)?))
}
pub fn recipes(root: &ConfinedRoot, stack: &str) -> Result<Vec<Recipe>> {
    let mut recipes = Vec::new();
    let digest = config_digest(root)?;
    if stack == "node" {
        let package: serde_json::Value =
            serde_json::from_slice(&root.read("package.json", 512 * 1024)?)?;
        if let Some(scripts) = package.get("scripts").and_then(|v| v.as_object()) {
            for (name, script) in scripts {
                if !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_:".contains(c))
                {
                    continue;
                }
                let kind = match name.as_str() {
                    "test" | "test:unit" => "test",
                    "lint" | "lint:fix" => "lint",
                    "format" | "format:check" => "format",
                    "build" | "typecheck" | "check" => "build",
                    "dev" | "start" => "dev_server",
                    _ => continue,
                };
                recipes.push(Recipe {
                    id: format!("npm:{name}"),
                    name: format!("npm · {name}"),
                    kind: kind.into(),
                    adapter: "node".into(),
                    script: name.clone(),
                    description: security::bounded(
                        &security::redact(script.as_str().unwrap_or("")),
                        1000,
                    ),
                    config_digest: digest.clone(),
                });
            }
        }
    } else if stack == "python" {
        for (script, name, kind) in [
            ("pytest", "Python · tests", "test"),
            ("ruff", "Python · lint", "lint"),
            ("ruff_fix", "Python · fix lint", "lint"),
        ] {
            recipes.push(Recipe {
                id: format!("python:{script}"),
                name: name.into(),
                kind: kind.into(),
                adapter: "python".into(),
                script: script.into(),
                description: format!(
                    "python -m {}",
                    if script == "pytest" {
                        "pytest -q"
                    } else if script == "ruff_fix" {
                        "ruff check --fix ."
                    } else {
                        "ruff check ."
                    }
                ),
                config_digest: digest.clone(),
            });
        }
    }
    Ok(recipes)
}
pub fn git(path: &Path, settings: &Settings) -> Option<GitInfo> {
    let run = |args: &[&str]| git_read(path, settings, args).map(|s| s.trim().to_string());
    let git_root = run(&["rev-parse", "--show-toplevel"])?;
    let head = run(&["rev-parse", "--verify", "HEAD"]);
    let branch =
        run(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "New repository".into());
    let index = run(&["rev-parse", "--git-path", "index"]).map(PathBuf::from)?;
    let index = if index.is_absolute() {
        index
    } else {
        path.join(index)
    };
    let index_hash = std::fs::read(index).ok().map(|b| hash(&b));
    let status = run(&["status", "--porcelain=v2", "-z", "--untracked-files=normal"])?;
    Some(GitInfo {
        head,
        branch,
        index_hash,
        status: security::bounded(&security::redact(&status.replace('\0', "\n")), 8192),
        root: git_root,
    })
}
fn git_read(path: &Path, settings: &Settings, args: &[&str]) -> Option<String> {
    use std::{
        io::Read,
        os::windows::process::CommandExt,
        process::Stdio,
        time::{Duration, Instant},
    };
    let executable = settings
        .git_path
        .clone()
        .or_else(|| commands::find_program("git.exe"))?;
    let canonical = Path::new(&executable).canonicalize().ok()?;
    let project = path.canonicalize().ok()?;
    if canonical.starts_with(&project) {
        return None;
    }
    let mut cmd = Command::new(&executable);
    cmd.current_dir(path)
        .creation_flags(0x08000000)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Ok(v) = std::env::var("SystemRoot") {
        cmd.env("SystemRoot", v);
    }
    cmd.env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL");
    let mut child = cmd
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.pager=cat",
            "-c",
            "credential.helper=",
            "-c",
            "core.hooksPath=NUL",
        ])
        .args(args)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Err(_) => break false,
            _ => {
                if started.elapsed() > Duration::from_secs(5) {
                    let _ = child.kill();
                    let _ = child.wait();
                    break false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let bytes = reader.join().ok()?;
    if !success || bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}
