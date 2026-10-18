use crate::domain::{AppError, Result};
use regex::Regex;
use std::sync::OnceLock;

pub fn relative_path(input: &str) -> Result<String> {
    if input.is_empty()
        || input.len() > 1024
        || input.starts_with(['/', '\\'])
        || input
            .chars()
            .any(|c| c.is_control() || ":*?<>|\"".contains(c))
    {
        return Err(AppError::new(
            "UNSAFE_PATH",
            "Choose an ordinary file inside the project.",
        ));
    }
    let normalized = input.replace('\\', "/");
    for part in normalized.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.chars().count() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.chars().last(), Some('1'..='9' | '¹' | '²' | '³')));
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) || reserved
        {
            return Err(AppError::new(
                "UNSAFE_PATH",
                "This file name is not supported safely on Windows.",
            ));
        }
    }
    Ok(normalized)
}

pub fn protected(path: &str) -> bool {
    path.replace('\\', "/").split('/').any(|p| {
        let p = p.to_lowercase();
        p.starts_with(".env")
            || matches!(
                p.as_str(),
                ".git"
                    | ".hg"
                    | ".svn"
                    | ".local-agent"
                    | ".aws"
                    | ".ssh"
                    | ".azure"
                    | ".npmrc"
                    | ".pypirc"
                    | "credentials"
                    | "credentials.json"
                    | "secrets.json"
                    | "storage-state.json"
                    | "auth.json"
                    | "id_rsa"
                    | "id_ed25519"
            )
            || p.ends_with(".pem")
            || p.ends_with(".pfx")
            || p.ends_with(".key")
            || p.ends_with(".p12")
            || p.ends_with(".kdbx")
    })
}
pub fn artifact(path: &str) -> bool {
    path.split('/').any(|p| {
        matches!(
            p.to_lowercase().as_str(),
            "node_modules"
                | ".venv"
                | "venv"
                | "__pycache__"
                | ".pytest_cache"
                | ".ruff_cache"
                | "dist"
                | "build"
                | "target"
                | "coverage"
                | ".next"
                | ".cache"
        )
    })
}
fn patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| [
        r"(?i)-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)",
        r"\b(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[A-Z0-9]{16})\b",
        r"\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\b",
        r#"(?im)(?:api[_-]?key|access[_-]?token|secret[_-]?key|password|authorization)\s*[:=]\s*[\"']?[^\s\"',;]{8,}"#,
        r"(?i)https?://[^\s/@]+:[^\s/@]+@[^\s]+",
    ].iter().map(|s| Regex::new(s).expect("static secret pattern")).collect())
}
pub fn has_secret(text: &str) -> bool {
    patterns().iter().any(|r| r.is_match(text)) || !entropy_candidates(text).is_empty()
}
fn entropy_candidates(text: &str) -> Vec<(usize, usize)> {
    static CANDIDATES: OnceLock<Regex> = OnceLock::new();
    CANDIDATES
        .get_or_init(|| Regex::new(r#"["']([A-Za-z0-9+/=_-]{32,256})["']"#).unwrap())
        .captures_iter(text)
        .filter_map(|capture| {
            let token = capture.get(1)?;
            let bytes = token.as_str().as_bytes();
            if bytes.iter().all(u8::is_ascii_hexdigit)
                || !bytes.iter().any(u8::is_ascii_uppercase)
                || !bytes.iter().any(u8::is_ascii_lowercase)
                || !bytes.iter().any(u8::is_ascii_digit)
            {
                return None;
            }
            let mut counts = [0usize; 128];
            for b in bytes {
                counts[*b as usize] += 1;
            }
            let entropy = counts
                .iter()
                .filter(|n| **n > 0)
                .map(|n| {
                    let p = *n as f64 / bytes.len() as f64;
                    -p * p.log2()
                })
                .sum::<f64>();
            (entropy >= 4.2).then_some((token.start(), token.end()))
        })
        .collect()
}
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for r in patterns() {
        out = r.replace_all(&out, "[REDACTED]").into_owned();
    }
    for (start, end) in entropy_candidates(&out).into_iter().rev() {
        out.replace_range(start..end, "[REDACTED]");
    }
    // Strip all terminal escape sequences before text reaches UI or model.
    static ANSI: OnceLock<Regex> = OnceLock::new();
    out = ANSI
        .get_or_init(|| {
            Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))").unwrap()
        })
        .replace_all(&out, "")
        .into_owned();
    out.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect()
}
pub fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.into();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[omitted {} bytes]", &text[..end], text.len() - end)
}
pub fn editable(bytes: &[u8]) -> bool {
    bytes.len() <= 512 * 1024
        && !bytes.contains(&0)
        && std::str::from_utf8(bytes).is_ok_and(|s| !has_secret(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_windows_escape_forms() {
        for p in [
            "../x",
            "a/../x",
            "C:foo",
            "C:/foo",
            "\\\\host\\share",
            "a//b",
            "a/./b",
            "a:stream",
            "con.txt",
            "COM1.js",
            "lpt9",
            "x. ",
            "x.",
            "x ",
            "a\0b",
            "\\??\\C:\\x",
        ] {
            assert!(relative_path(p).is_err(), "{p}");
        }
        assert_eq!(relative_path("src\\a.ts").unwrap(), "src/a.ts");
    }
    #[test]
    fn protects_secrets_and_metadata() {
        for p in [
            ".env",
            ".env.example",
            "a/.git/config",
            "keys/a.pem",
            ".ssh/id_rsa",
        ] {
            assert!(protected(p));
        }
        assert!(!protected("src/key.ts"));
    }
    #[test]
    fn redacts_values_and_terminal_codes() {
        let v = "api_key=abcdefghijklm\n\u{1b}[31merror\u{1b}[0m";
        assert!(has_secret(v));
        let v = redact(v);
        assert!(!v.contains("abcdefghijklm"));
        assert!(!v.contains('\u{1b}'));
        assert!(v.contains("error"));
    }
}
