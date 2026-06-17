//! Narrow git helper used by local agent-directory injection.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_GRAFT_FILE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_WORK_TREE",
];

pub fn command() -> Command {
    let mut cmd = Command::new("git");
    clear_local_env(&mut cmd);
    cmd
}

pub fn clear_local_env(cmd: &mut Command) {
    for key in LOCAL_ENV_VARS {
        cmd.env_remove(key);
    }
}

fn git_exclude_path(cwd: &Path) -> Option<PathBuf> {
    let out = command()
        .args([
            "-C",
            &cwd.to_string_lossy(),
            "rev-parse",
            "--git-path",
            "info/exclude",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if rel.is_empty() {
        return None;
    }
    let p = Path::new(&rel);
    Some(if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    })
}

/// Best-effort: add `name` to the current directory's git exclude file.
pub fn git_exclude(cwd: &Path, name: &str) {
    let Some(exclude_path) = git_exclude_path(cwd) else {
        return;
    };
    if let Some(parent) = exclude_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == name) {
        return;
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(name);
    content.push('\n');
    let _ = std::fs::write(&exclude_path, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_env_vars_match_git_reported_vars() {
        let out = Command::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let local_env = String::from_utf8_lossy(&out.stdout);
        let mut expected = local_env
            .lines()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        expected.sort_unstable();

        let mut actual = LOCAL_ENV_VARS.to_vec();
        actual.sort_unstable();

        assert_eq!(actual, expected);
    }
}
