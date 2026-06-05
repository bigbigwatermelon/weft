//! Claude session-storage helpers (encoded-cwd + native session-id capture).
//!
//! Product principle: we spawn PLAIN `claude` under the user's standard HOME so
//! their own config / permission mode / allowlist apply. We never inject
//! `--dangerously-skip-permissions`; trust + permission popups render in the
//! embedded TUI and the user answers them there.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Claude encodes the *canonical* cwd into a projects-dir name by replacing
/// both '/' and '.' with '-'. Symlinks MUST be resolved first (macOS /tmp ->
/// /private/tmp) or the encoded dir won't match. Verified empirically.
pub fn encode_cwd(canonical: &Path) -> String {
    canonical
        .to_string_lossy()
        .chars()
        .map(|c| if c == '/' || c == '.' { '-' } else { c })
        .collect()
}

/// `~/.claude/projects/<encoded-canonical-cwd>` for a (possibly symlinked) cwd.
pub fn projects_dir_for(cwd: &Path) -> std::io::Result<PathBuf> {
    let canon = std::fs::canonicalize(cwd)?;
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME unset"))?;
    Ok(PathBuf::from(home)
        .join(".claude")
        .join("projects")
        .join(encode_cwd(&canon)))
}

fn mtime_secs(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Newest `*.jsonl` in `projects_dir` whose mtime is at/after `since`.
/// Returns the session id (== file stem) only after cross-checking the file
/// actually contains a matching `"sessionId":"<stem>"` — the stem and the
/// recorded id must agree, or we don't trust the capture.
pub fn capture_session_id(projects_dir: &Path, since: u64) -> Option<String> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(projects_dir).ok()?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let mt = mtime_secs(&p);
        if mt + 2 < since {
            continue; // 2s slack for clock granularity
        }
        if best.as_ref().map_or(true, |(bm, _)| mt >= *bm) {
            best = Some((mt, p));
        }
    }
    let (_, path) = best?;
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let content = std::fs::read_to_string(&path).ok()?;
    if content.contains(&format!("\"sessionId\":\"{}\"", stem)) {
        Some(stem)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_slashes_and_dots() {
        let p = Path::new("/private/tmp/weft/.claude-worktrees/x");
        assert_eq!(encode_cwd(p), "-private-tmp-weft--claude-worktrees-x");
    }

    #[test]
    fn no_special_chars_pass_through() {
        assert_eq!(encode_cwd(Path::new("/a/b/c")), "-a-b-c");
    }

    #[test]
    fn capture_requires_stem_id_agreement() {
        let dir = std::env::temp_dir().join(format!("weft-claude-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // matching stem == sessionId -> captured
        let sid = "11111111-2222-3333-4444-555555555555";
        std::fs::write(
            dir.join(format!("{sid}.jsonl")),
            format!("{{\"sessionId\":\"{sid}\",\"x\":1}}\n"),
        )
        .unwrap();
        assert_eq!(capture_session_id(&dir, 0).as_deref(), Some(sid));

        // a jsonl whose stem does NOT appear inside -> not trusted
        let dir2 = dir.join("mismatch");
        std::fs::create_dir_all(&dir2).unwrap();
        std::fs::write(
            dir2.join("aaaa.jsonl"),
            "{\"sessionId\":\"bbbb\"}\n".to_string(),
        )
        .unwrap();
        assert_eq!(capture_session_id(&dir2, 0), None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
