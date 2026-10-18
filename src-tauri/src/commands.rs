use crate::{domain::*, security};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

pub struct CommandResult {
    pub code: Option<i32>,
    pub output: String,
    pub status: String,
    pub duration_ms: f64,
}
#[derive(Clone)]
pub struct CommandPlan {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub directory: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub timeout: Duration,
}

pub fn find_program(name: &str) -> Option<String> {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(name))
                .find(|p| p.is_file())
        })
        .map(|p| p.to_string_lossy().into_owned())
}
pub fn minimal_env(executable: &Path, home: &Path) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for key in ["SystemRoot", "WINDIR", "SystemDrive", "COMSPEC", "PATHEXT"] {
        if let Ok(v) = std::env::var(key) {
            env.insert(key.into(), v);
        }
    }
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    env.insert(
        "PATH".into(),
        format!(
            "{};{}\\System32;{}",
            executable.parent().unwrap_or(Path::new("")).display(),
            system,
            system
        ),
    );
    for key in [
        "HOME",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        env.insert(key.into(), home.to_string_lossy().into_owned());
    }
    env.insert("CI".into(), "1".into());
    env.insert("NO_COLOR".into(), "1".into());
    env.insert("PIP_NO_INPUT".into(), "1".into());
    env.insert(
        "PYTHONPYCACHEPREFIX".into(),
        home.join("pycache").to_string_lossy().into(),
    );
    env
}
pub fn plan(recipe: &Recipe, copy: &Path, settings: &Settings, home: &Path) -> Result<CommandPlan> {
    std::fs::create_dir_all(home)?;
    let (executable, args) = match recipe.adapter.as_str() {
        "node" => {
            let node = settings
                .node_path
                .clone()
                .or_else(|| find_program("node.exe"))
                .ok_or_else(|| {
                    AppError::new(
                        "NODE_MISSING",
                        "Choose Node.js in Settings before running project checks.",
                    )
                })?;
            let node = process_path(Path::new(&node))?;
            let npm = node
                .parent()
                .unwrap_or(Path::new(""))
                .join("node_modules/npm/bin/npm-cli.js");
            if !npm.is_file() {
                return Err(AppError::new("NPM_MISSING", "npm could not be found beside Node.js. Choose a complete Node.js installation."));
            }
            if !recipe
                .script
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_:".contains(c))
                || recipe.script.is_empty()
            {
                return Err(AppError::new(
                    "RECIPE_INVALID",
                    "This project script name is unsupported.",
                ));
            }
            (
                node,
                vec![
                    npm.to_string_lossy().into(),
                    "--userconfig".into(),
                    home.join("empty.npmrc").to_string_lossy().into(),
                    "--globalconfig".into(),
                    home.join("empty-global.npmrc").to_string_lossy().into(),
                    "run".into(),
                    recipe.script.clone(),
                ],
            )
        }
        "python" => {
            let python = settings
                .python_path
                .clone()
                .or_else(|| find_program("python.exe"))
                .ok_or_else(|| {
                    AppError::new(
                        "PYTHON_MISSING",
                        "Choose a Python interpreter in Settings before running project checks.",
                    )
                })?;
            let args = match recipe.script.as_str() {
                "pytest" => vec!["-m", "pytest", "-q"],
                "ruff" => vec!["-m", "ruff", "check", "."],
                "ruff_fix" => vec!["-m", "ruff", "check", "--fix", "."],
                _ => {
                    return Err(AppError::new(
                        "RECIPE_INVALID",
                        "This check is not registered.",
                    ))
                }
            }
            .into_iter()
            .map(String::from)
            .collect();
            (PathBuf::from(python), args)
        }
        _ => {
            return Err(AppError::new(
                "RECIPE_INVALID",
                "This project type does not have a supported command adapter.",
            ))
        }
    };
    if !executable.is_absolute()
        || !executable.is_file()
        || executable.extension().is_none_or(|e| e != "exe")
    {
        return Err(AppError::new(
            "EXECUTABLE_INVALID",
            "Choose an installed .exe tool in Settings.",
        ));
    }
    let environment = minimal_env(&executable, home);
    Ok(CommandPlan {
        executable,
        args,
        directory: copy.into(),
        environment,
        timeout: Duration::from_secs(if recipe.kind == "build" { 300 } else { 120 }),
    })
}

pub fn quote(arg: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        if c == '"' {
            out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
        } else {
            out.extend(std::iter::repeat_n('\\', slashes));
        }
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}
pub fn process_path(path: &Path) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    if text.len() > 240 || text.starts_with("UNC\\") {
        return Err(AppError::new("COMMAND_PATH","This tool needs a shorter local project path. File inspection and review remain available."));
    }
    Ok(PathBuf::from(text))
}

pub fn execute(plan: CommandPlan, cancel: CancellationToken) -> Result<CommandResult> {
    use crate::platform::wide;
    use std::{
        io::Read,
        os::windows::io::{AsRawHandle, FromRawHandle},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{
            Foundation::*,
            Security::SECURITY_ATTRIBUTES,
            Storage::FileSystem::*,
            System::{JobObjects::*, Pipes::CreatePipe, Threading::*},
        },
    };
    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let start = Instant::now();
    unsafe {
        let job = Owned(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| {
            AppError::new("PROCESS_JOB", "Windows could not supervise this command.").detail(e)
        })?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 64;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|e| {
            AppError::new("PROCESS_JOB", "Command safety limits could not be set.").detail(e)
        })?;
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: BOOL(1),
        };
        let mut read_out = HANDLE::default();
        let mut write_out = HANDLE::default();
        let mut read_err = HANDLE::default();
        let mut write_err = HANDLE::default();
        CreatePipe(&mut read_out, &mut write_out, Some(&security), 0).map_err(|e| {
            AppError::new("PROCESS_PIPE", "Command output could not be connected.").detail(e)
        })?;
        let out_writer = Owned(write_out);
        let out_file = std::fs::File::from_raw_handle(read_out.0);
        CreatePipe(&mut read_err, &mut write_err, Some(&security), 0).map_err(|e| {
            AppError::new("PROCESS_PIPE", "Command output could not be connected.").detail(e)
        })?;
        let err_writer = Owned(write_err);
        let err_file = std::fs::File::from_raw_handle(read_err.0);
        SetHandleInformation(
            HANDLE(out_file.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(0),
        )
        .map_err(|e| {
            AppError::new("PROCESS_PIPE", "Output handles could not be protected.").detail(e)
        })?;
        SetHandleInformation(
            HANDLE(err_file.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(0),
        )
        .map_err(|e| {
            AppError::new("PROCESS_PIPE", "Output handles could not be protected.").detail(e)
        })?;
        let nul = Owned(
            CreateFileW(
                PCWSTR(wide(std::ffi::OsStr::new("NUL")).as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(&security),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
            .map_err(|e| {
                AppError::new("PROCESS_INPUT", "Command input could not be closed.").detail(e)
            })?,
        );
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            hStdOutput: out_writer.0,
            hStdError: err_writer.0,
            hStdInput: nul.0,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        let command = std::iter::once(plan.executable.to_string_lossy().into_owned())
            .chain(plan.args.clone())
            .map(|s| quote(&s))
            .collect::<Vec<_>>()
            .join(" ");
        let mut command = wide(std::ffi::OsStr::new(&command));
        let executable_path = process_path(&plan.executable)?;
        let directory_path = process_path(&plan.directory)?;
        let executable = wide(executable_path.as_os_str());
        let directory = wide(directory_path.as_os_str());
        let mut environment = Vec::new();
        for (k, v) in &plan.environment {
            environment.extend(wide(std::ffi::OsStr::new(&format!("{k}={v}"))));
        }
        if environment.is_empty() {
            environment.push(0u16);
        }
        environment.push(0u16);
        CreateProcessW(
            PCWSTR(executable.as_ptr()),
            PWSTR(command.as_mut_ptr()),
            None,
            None,
            true,
            CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            Some(environment.as_ptr() as *const _),
            PCWSTR(directory.as_ptr()),
            &startup,
            &mut info,
        )
        .map_err(|e| {
            AppError::new(
                "COMMAND_START",
                "The project check could not start. Check the selected tool installation.",
            )
            .detail(e)
        })?;
        let process = Owned(info.hProcess);
        let thread = Owned(info.hThread);
        if let Err(e) = AssignProcessToJobObject(job.0, process.0) {
            let _ = TerminateProcess(process.0, 1);
            return Err(AppError::new(
                "PROCESS_JOB",
                "The command was stopped because Windows could not supervise it.",
            )
            .detail(e));
        }
        drop(out_writer);
        drop(err_writer);
        drop(nul);
        let count = Arc::new(AtomicUsize::new(0));
        let reader = |mut file: std::fs::File, count: Arc<AtomicUsize>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    match file.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let old = count.fetch_add(n, Ordering::Relaxed);
                            if old < 10 * 1024 * 1024 {
                                bytes.extend_from_slice(&buf[..n.min(10 * 1024 * 1024 - old)]);
                            }
                        }
                    }
                }
                bytes
            })
        };
        let out = reader(out_file, count.clone());
        let err = reader(err_file, count.clone());
        if ResumeThread(thread.0) == u32::MAX {
            let _ = TerminateJobObject(job.0, 1);
            return Err(AppError::new(
                "COMMAND_START",
                "The supervised command could not resume.",
            ));
        }
        let mut status = "completed";
        loop {
            if WaitForSingleObject(process.0, 25) == WAIT_OBJECT_0 {
                break;
            }
            if cancel.is_cancelled() {
                status = "cancelled";
                let _ = TerminateJobObject(job.0, 1);
                break;
            }
            if start.elapsed() > plan.timeout {
                status = "timed_out";
                let _ = TerminateJobObject(job.0, 1);
                break;
            }
            if count.load(Ordering::Relaxed) > 10 * 1024 * 1024 {
                status = "output_limit";
                let _ = TerminateJobObject(job.0, 1);
                break;
            }
        }
        let _ = TerminateJobObject(job.0, 1);
        let _ = WaitForSingleObject(process.0, 3000);
        let mut exit = 0;
        let _ = GetExitCodeProcess(process.0, &mut exit);
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&out.join().unwrap_or_default()),
            String::from_utf8_lossy(&err.join().unwrap_or_default())
        );
        Ok(CommandResult {
            code: if status == "completed" {
                Some(exit as i32)
            } else {
                None
            },
            output: security::bounded(&security::redact(&output), 48 * 1024),
            status: status.into(),
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_windows_arguments_without_shell() {
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("x\\"), "\"x\\\\\"");
        assert_eq!(quote("& % ! $()"), "\"& % ! $()\"");
    }
    #[test]
    fn controlled_environment_omits_credentials_and_loader_options() {
        let env = minimal_env(Path::new("C:/tools/node.exe"), Path::new("C:/task/temp"));
        assert!(!env.contains_key("NODE_OPTIONS"));
        assert!(!env.contains_key("AWS_SECRET_ACCESS_KEY"));
        assert_eq!(env.get("CI").unwrap(), "1");
    }
    #[test]
    fn actual_node_process_arguments_and_exit() {
        let Some(node) = find_program("node.exe") else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("echo.cjs"),
            "console.log(JSON.stringify(process.argv.slice(2))); process.exitCode=7",
        )
        .unwrap();
        let args = vec![
            "echo.cjs".into(),
            "a b".into(),
            "a\"b".into(),
            "trailing\\".into(),
            "& % ! $(noop)".into(),
            "київ".into(),
        ];
        let environment = minimal_env(Path::new(&node), tmp.path());
        let plan = CommandPlan {
            executable: node.into(),
            args,
            directory: tmp.path().into(),
            environment,
            timeout: Duration::from_secs(5),
        };
        let result = execute(plan, CancellationToken::new()).unwrap();
        assert_eq!(result.code, Some(7));
        let values: Vec<String> = serde_json::from_str(result.output.trim()).unwrap();
        assert_eq!(
            values,
            vec!["a b", "a\"b", "trailing\\", "& % ! $(noop)", "київ"]
        );
    }
    #[test]
    fn timeout_kills_descendants() {
        let Some(node) = find_program("node.exe") else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("parent.cjs"),"require('node:child_process').spawn(process.execPath,['child.cjs'],{stdio:'inherit'}); setInterval(()=>{},1000)").unwrap();
        std::fs::write(
            tmp.path().join("child.cjs"),
            "require('node:fs').writeFileSync('child-started.txt','ready'); setTimeout(()=>require('node:fs').writeFileSync('escaped.txt','bad'),2000)",
        )
        .unwrap();
        let plan = CommandPlan {
            executable: PathBuf::from(&node),
            args: vec!["parent.cjs".into()],
            directory: tmp.path().into(),
            environment: minimal_env(Path::new(&node), tmp.path()),
            timeout: Duration::from_millis(800),
        };
        let result = execute(plan, CancellationToken::new()).unwrap();
        assert_eq!(
            result.status, "timed_out",
            "exit={:?}, output={}",
            result.code, result.output
        );
        assert!(
            tmp.path().join("child-started.txt").exists(),
            "The descendant must start for this test to prove termination"
        );
        std::thread::sleep(Duration::from_millis(2200));
        assert!(!tmp.path().join("escaped.txt").exists());
    }
}
