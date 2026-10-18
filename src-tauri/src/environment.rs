use crate::{domain::*, fs::ConfinedRoot, security};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

pub struct DependencyCopy {
    pub source: PathBuf,
    pub files: Vec<(String, u64)>,
    pub bytes: u64,
}
pub fn inspect(project: &Path) -> Result<Option<DependencyCopy>> {
    let source = project.join("node_modules");
    if !source.exists() {
        return Ok(None);
    }
    let root = ConfinedRoot::open(&source)?;
    let mut files = Vec::new();
    let mut bytes = 0u64;
    let mut directories = vec![String::new()];
    let started = Instant::now();
    while let Some(relative) = directories.pop() {
        for item in std::fs::read_dir(root.path().join(&relative))? {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            let logical = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };
            security::relative_path(&logical)?;
            if security::protected(&logical) {
                continue;
            }
            let metadata = std::fs::symlink_metadata(item.path())?;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(AppError::new("DEPENDENCY_LINK","This dependency tree contains links. Validation needs an ordinary npm environment; no shared links or in-place command fallback are used."));
            }
            if metadata.is_dir() {
                directories.push(logical);
            } else if metadata.is_file() {
                bytes += metadata.len();
                files.push((logical, metadata.len()));
            }
            if files.len() > 250_000
                || bytes > 4 * 1024 * 1024 * 1024
                || started.elapsed().as_secs() > 30
            {
                return Err(AppError::new("DEPENDENCY_LIMIT","The dependency copy is too large. Prepare a smaller test environment externally."));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Some(DependencyCopy {
        source,
        files,
        bytes,
    }))
}
pub fn prepare(plan: DependencyCopy, working: &Path, cancel: &CancellationToken) -> Result<()> {
    let source = ConfinedRoot::open(&plan.source)?;
    let target = working.join("node_modules");
    std::fs::create_dir(&target)?;
    let dest = ConfinedRoot::open(&target)?;
    let mut total = 0u64;
    for (path, expected_size) in plan.files {
        if cancel.is_cancelled() {
            return Err(AppError::new(
                "CANCELLED",
                "Dependency preparation was stopped.",
            ));
        }
        let bytes = source.read(&path, 64 * 1024 * 1024)?;
        if bytes.len() as u64 != expected_size {
            return Err(AppError::new(
                "DEPENDENCY_CHANGED",
                "Dependencies changed during preparation. Retry with a stable environment.",
            ));
        }
        total += bytes.len() as u64;
        if total > 4 * 1024 * 1024 * 1024 {
            return Err(AppError::new(
                "DEPENDENCY_LIMIT",
                "The dependency copy exceeded 4 GiB.",
            ));
        }
        dest.write_new(&path, &bytes)?;
    }
    Ok(())
}
