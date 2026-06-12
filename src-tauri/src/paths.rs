//! Canonical weft home + derived paths. Everything persistent lives under
//! ~/.weft so worktree cwds stay stable across restarts (resume depends on it).

use std::path::{Component, Path, PathBuf};

/// weft home. Honors the WEFT_HOME env override (used for test isolation and to
/// let users relocate weft's data); otherwise ~/.weft. Created if missing.
pub fn weft_home() -> std::io::Result<PathBuf> {
    let dir = match std::env::var("WEFT_HOME") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => {
            let home = dirs::home_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))?;
            home.join(".weft")
        }
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// ~/.weft/weft.db
pub fn db_path() -> std::io::Result<PathBuf> {
    Ok(weft_home()?.join("weft.db"))
}

/// ~/.weft/worktrees
pub fn worktree_home() -> std::io::Result<PathBuf> {
    let dir = weft_home()?.join("worktrees");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn checked_segment(segment: &str, label: &str) -> std::io::Result<String> {
    let trimmed = segment.trim();
    let mut components = Path::new(trimmed).components();
    match (
        components.next(),
        components.next(),
        trimmed.contains('/') || trimmed.contains('\\'),
    ) {
        (Some(Component::Normal(_)), None, false) => Ok(trimmed.to_string()),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid {label} segment"),
        )),
    }
}

/// ~/.weft/workspaces/<workspace>/tasks/<task>/runs/<run>
pub fn run_home(workspace_slug: &str, task_slug: &str, run_slug: &str) -> std::io::Result<PathBuf> {
    let ws = checked_segment(workspace_slug, "workspace")?;
    let task = checked_segment(task_slug, "task")?;
    let run = checked_segment(run_slug, "run")?;
    let dir = weft_home()?
        .join("workspaces")
        .join(ws)
        .join("tasks")
        .join(task)
        .join("runs")
        .join(run);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// ~/.weft/skills/sources — git-cloned skill source caches, one dir per source.
pub fn skills_home() -> std::io::Result<PathBuf> {
    let dir = weft_home()?.join("skills").join("sources");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Process-global lock guarding the shared `WEFT_HOME` env var across lib
/// tests. The lib-test binary runs tests on parallel threads sharing one
/// process env, so a test that *sets* WEFT_HOME (e.g. materialize tests) and a
/// test that *reads* the default (`paths_are_under_weft_home`) must not overlap.
/// Every test that touches WEFT_HOME acquires this for the duration it relies on
/// a particular env state. Panic-tolerant: a poisoned guard is recovered so one
/// failing test doesn't cascade into the rest.
#[cfg(test)]
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::{db_path, run_home, skills_home, weft_home, worktree_home, ENV_LOCK};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    struct WeftHomeGuard {
        old: Option<OsString>,
        tmp: Option<PathBuf>,
    }

    impl WeftHomeGuard {
        fn unset() -> Self {
            let old = std::env::var_os("WEFT_HOME");
            std::env::remove_var("WEFT_HOME");
            Self { old, tmp: None }
        }

        fn new(name: &str) -> Self {
            let old = std::env::var_os("WEFT_HOME");
            let tmp = std::env::temp_dir().join(format!(
                "weft-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::env::set_var("WEFT_HOME", &tmp);
            Self {
                old,
                tmp: Some(tmp),
            }
        }

        fn path(&self) -> &Path {
            self.tmp
                .as_deref()
                .expect("WEFT_HOME guard should have a temp path")
        }
    }

    impl Drop for WeftHomeGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(old) => std::env::set_var("WEFT_HOME", old),
                None => std::env::remove_var("WEFT_HOME"),
            }
            if let Some(tmp) = &self.tmp {
                let _ = std::fs::remove_dir_all(tmp);
            }
        }
    }

    #[test]
    fn paths_are_under_weft_home() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Assert against the default home, so a WEFT_HOME another test set (and
        // may not have cleared yet on its own thread) can't leak in here.
        let _home = WeftHomeGuard::unset();
        let home = weft_home().unwrap();
        assert!(home.ends_with(".weft"));
        assert!(db_path().unwrap().ends_with("weft.db"));
        assert!(worktree_home().unwrap().ends_with("worktrees"));
        assert!(skills_home().unwrap().ends_with("skills/sources"));
    }

    #[test]
    fn run_home_is_namespaced_under_weft_home() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = WeftHomeGuard::new("paths");

        let p = run_home("people-ops", "draft-offer", "main").unwrap();
        assert!(p.ends_with("workspaces/people-ops/tasks/draft-offer/runs/main"));
        assert!(p.is_dir(), "run_home should create the directory");
        assert!(p.starts_with(home.path()));
    }

    #[test]
    fn run_home_rejects_empty_segments() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let err = run_home("workspace", "", "run").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn run_home_rejects_dot_segments() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for segment in [".", ".."] {
            let err = run_home("workspace", "task", segment).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        }
    }
}
