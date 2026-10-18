use crate::{domain::*, platform, security};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub struct ConfinedRoot {
    path: PathBuf,
    pub identity: RootIdentity,
    _pins: Vec<File>,
}
impl ConfinedRoot {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(AppError::new(
                "PROJECT_PATH",
                "Choose a project with the folder picker.",
            ));
        }
        let mut pins = Vec::new();
        for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
            let dir = platform::open_dir(ancestor)?;
            platform::ensure_ordinary(&dir)?;
            pins.push(dir);
        }
        let canonical = path.canonicalize()?;
        platform::ensure_volume(&canonical)?;
        let root = pins
            .last()
            .ok_or_else(|| AppError::new("PROJECT_PATH", "The project folder is unavailable."))?;
        Ok(Self {
            identity: platform::identity(root)?,
            path: canonical,
            _pins: pins,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn verify(&self, expected: &RootIdentity) -> Result<()> {
        if &self.identity != expected {
            return Err(AppError::new(
                "ROOT_CHANGED",
                "The project folder was replaced or moved. Open it again before continuing.",
            ));
        }
        Ok(())
    }
    pub fn pin_parent(&self, relative: &str, create_dirs: bool) -> Result<(PathBuf, Vec<File>)> {
        let relative = security::relative_path(relative)?;
        if security::protected(&relative) {
            return Err(AppError::new(
                "PROTECTED_FILE",
                "Credentials and repository metadata are protected.",
            ));
        }
        let mut at = self.path.clone();
        let mut pins = Vec::new();
        let parts = relative.split('/').collect::<Vec<_>>();
        for part in &parts[..parts.len() - 1] {
            at.push(part);
            if create_dirs && !at.exists() {
                std::fs::create_dir(&at)?;
            }
            let dir = platform::open_dir(&at)?;
            platform::ensure_ordinary(&dir)?;
            if at.join(".git").exists() {
                return Err(AppError::new(
                    "NESTED_REPOSITORY",
                    "Open this nested repository as a separate project.",
                ));
            }
            pins.push(dir);
        }
        Ok((at.join(parts.last().unwrap()), pins))
    }
    pub fn read(&self, relative: &str, max: u64) -> Result<Vec<u8>> {
        let (path, _pins) = self.pin_parent(relative, false)?;
        let mut file = platform::open_file(&path, false, false)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > max {
            return Err(AppError::new(
                "FILE_LIMIT",
                "This file is too large or is not an ordinary file.",
            ));
        }
        let mut bytes = Vec::new();
        (&mut file).take(max + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max {
            return Err(AppError::new(
                "FILE_LIMIT",
                "This file exceeds the read limit.",
            ));
        }
        Ok(bytes)
    }
    pub fn write_new(&self, relative: &str, bytes: &[u8]) -> Result<()> {
        let (path, _pins) = self.pin_parent(relative, true)?;
        let mut file = platform::open_file(&path, true, true)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn replace(&self, relative: &str, expected: &str, bytes: &[u8]) -> Result<()> {
        let (path, _pins) = self.pin_parent(relative, false)?;
        let mut file = platform::open_file(&path, true, false)?;
        let mut before = Vec::new();
        file.read_to_end(&mut before)?;
        if hash(&before) != expected {
            return Err(AppError::new(
                "EDIT_CONFLICT",
                "The file changed. Read it again before editing.",
            ));
        }
        file.seek(SeekFrom::Start(0))?;
        file.write_all(bytes)?;
        file.set_len(bytes.len() as u64)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn list(&self) -> Result<(Vec<String>, Vec<String>)> {
        self.list_policy(None)
    }
    pub fn list_policy(
        &self,
        rules: Option<&[(String, String)]>,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut files = Vec::new();
        let mut excluded = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let matchers = rules
            .map(|rules| {
                rules
                    .iter()
                    .map(|(scope, text)| {
                        let scoped = self.path.join(scope);
                        let mut builder = ignore::gitignore::GitignoreBuilder::new(&scoped);
                        for line in text.lines() {
                            builder.add_line(None, line).map_err(|e| {
                                AppError::new("IGNORE_RULE", "A saved ignore rule is invalid.")
                                    .detail(e)
                            })?;
                        }
                        Ok((
                            scoped,
                            builder.build().map_err(|e| {
                                AppError::new(
                                    "IGNORE_RULE",
                                    "Saved ignore rules could not be loaded.",
                                )
                                .detail(e)
                            })?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let walk = ignore::WalkBuilder::new(&self.path)
            .hidden(false)
            .require_git(false)
            .git_ignore(rules.is_none())
            .git_exclude(false)
            .git_global(false)
            .follow_links(false)
            .filter_entry(|e| {
                let path = e.path().to_string_lossy();
                let name = e.file_name().to_string_lossy();
                use std::os::windows::fs::MetadataExt;
                if e.depth() == 0 {
                    return true;
                }
                !(security::protected(&name)
                    || security::artifact(&path.replace('\\', "/"))
                    || std::fs::symlink_metadata(e.path())
                        .map_or(true, |m| m.file_attributes() & 0x400 != 0)
                    || (e.path().is_dir() && e.path().join(".git").exists()))
            })
            .build();
        for entry in walk {
            let entry = entry.map_err(|e| {
                AppError::new("SCAN_FAILED", "Part of the project could not be read.").detail(e)
            })?;
            if entry.depth() == 0 {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&self.path)
                .map_err(|e| {
                    AppError::new("SCAN_PATH", "A project path escaped its root.").detail(e)
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = std::fs::symlink_metadata(entry.path())?;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                excluded.push(format!("{relative} — link or placeholder"));
                continue;
            }
            if metadata.is_dir() {
                if entry.path().join(".git").exists() {
                    excluded.push(format!("{relative} — nested repository"));
                }
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if let Some(matchers) = &matchers {
                let mut ignored = false;
                for (scoped, matcher) in matchers {
                    if !entry.path().starts_with(scoped) {
                        continue;
                    }
                    let matched = matcher.matched_path_or_any_parents(entry.path(), false);
                    if matched.is_ignore() {
                        ignored = true;
                    } else if matched.is_whitelist() {
                        ignored = false;
                    }
                }
                if ignored {
                    continue;
                }
            }
            if security::relative_path(&relative).is_err() || security::protected(&relative) {
                excluded.push(format!("{relative} — protected"));
                continue;
            }
            if !seen.insert(relative.to_lowercase()) {
                return Err(AppError::new(
                    "NAME_COLLISION",
                    "Two project paths differ only by letter case.",
                ));
            }
            // Pin and validate ancestors before any content read. Nested repositories are rejected here.
            if self.pin_parent(&relative, false).is_err() {
                excluded.push(format!("{relative} — unsupported boundary"));
                continue;
            }
            files.push(relative);
            if files.len() > 20_000 {
                return Err(AppError::new(
                    "PROJECT_LIMIT",
                    "Choose a smaller project folder (maximum 20,000 source files).",
                ));
            }
        }
        files.sort();
        excluded.sort();
        Ok((files, excluded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_files_and_hash_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = ConfinedRoot::open(tmp.path()).unwrap();
        root.write_new("src/a.txt", b"first\r\n").unwrap();
        assert!(root.replace("src/a.txt", "stale", b"second").is_err());
        root.replace("src/a.txt", &hash(b"first\r\n"), b"second")
            .unwrap();
        assert_eq!(root.read("src/a.txt", 100).unwrap(), b"second");
        assert!(root.read("../outside", 100).is_err());
    }
    #[test]
    fn hardlinks_cannot_be_mutated() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a"), "a").unwrap();
        std::fs::hard_link(tmp.path().join("a"), tmp.path().join("b")).unwrap();
        let root = ConfinedRoot::open(tmp.path()).unwrap();
        assert!(root.replace("b", &hash(b"a"), b"x").is_err());
    }
}
