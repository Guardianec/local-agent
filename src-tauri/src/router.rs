use crate::domain::*;
pub fn route(request: &str, preset: &str, project: &Project) -> Route {
    if matches!(
        preset,
        "search" | "tests" | "lint" | "fix_lint" | "review" | "browser"
    ) {
        return Route {
            level: 0,
            score: 0,
            strategy: "recipe".into(),
            reasons: vec!["Use deterministic project tools first".into()],
            estimated_files: None,
        };
    }
    let text = request.to_lowercase();
    let read_only = preset == "explain"
        || ["explain ", "where ", "investigate ", "diagnose "]
            .iter()
            .any(|p| text.starts_with(p));
    let mut reasons = Vec::new();
    if text.contains("codex") || text.contains("cloud agent") {
        reasons.push("Cloud assistance was requested".into());
    }
    if !read_only {
        for (terms, reason) in [
            (
                vec![
                    "authentication",
                    "authorization",
                    "oauth",
                    "jwt",
                    "cryptograph",
                    "payment",
                    "password hashing",
                    "auth.ts",
                    "auth.js",
                    "login.ts",
                    "security policy",
                ],
                "Security-sensitive behavior needs broader review",
            ),
            (
                vec![
                    "database migration",
                    "schema migration",
                    "drop table",
                    "production deploy",
                    "terraform",
                    "kubernetes",
                ],
                "Infrastructure or data changes need broader review",
            ),
            (
                vec![
                    "rewrite architecture",
                    "rewrite the entire",
                    "redesign architecture",
                    "entire codebase",
                ],
                "This change exceeds a bounded local task",
            ),
        ] {
            if terms.iter().any(|t| text.contains(t)) {
                reasons.push(reason.into());
            }
        }
    }
    if !reasons.is_empty() {
        return Route {
            level: 4,
            score: 8,
            strategy: "handoff".into(),
            reasons,
            estimated_files: None,
        };
    }
    let tokens = text
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '/')
        .map(|term| term.trim_end_matches('.'))
        .filter(|s| s.len() >= 3)
        .collect::<Vec<_>>();
    let likely = project
        .files
        .iter()
        .filter(|f| tokens.iter().any(|t| f.to_lowercase().contains(t)))
        .count();
    let mut score = match likely {
        0 => 3,
        1..=2 => 0,
        3..=5 => 2,
        6..=12 => 4,
        _ => 8,
    };
    if likely > 12 {
        reasons.push("More than 12 files may be affected".into());
    }
    if project.stack == "other" || project.recipes.is_empty() {
        score += 2;
        reasons.push("No supported validation recipe was detected".into());
    }
    if text.contains("clean up everything") || text.contains("fix everything") {
        score += 3;
        reasons.push("The requested scope needs narrowing".into());
    }
    if text.contains("install ") || text.contains("upgrade dependenc") {
        score += 3;
        reasons.push("Dependency changes require manual setup".into());
    }
    if reasons.is_empty() {
        reasons.push(
            if likely == 0 {
                "Scope will be checked through targeted searches"
            } else {
                "Task appears bounded to relevant files"
            }
            .into(),
        );
    }
    let mut level = match score {
        0..=1 => 1,
        2..=4 => 2,
        5..=7 => 3,
        _ => 4,
    };
    let simple = read_only && !text.starts_with("investigate ") && !text.starts_with("diagnose ")
        || [
            "summarize ",
            "summarise ",
            "rename ",
            "translate ",
            "describe ",
            "поясни ",
            "опиши ",
            "переклади ",
        ]
        .iter()
        .any(|p| text.starts_with(p));
    if simple && likely <= 5 && score < 8 {
        level = 1;
    } else if level < 3 {
        level = 2;
    }
    Route {
        level,
        score,
        strategy: if level == 4 {
            "handoff"
        } else if level == 1 {
            "fast"
        } else {
            "balanced"
        }
        .into(),
        reasons,
        estimated_files: if likely == 0 {
            None
        } else {
            Some(likely as u32)
        },
    }
}
