//! The planner: capturing the lead's proposed decomposition of a Task into
//! directions + per-repo scope (ARCHITECTURE §4.10, §5.1), and confirming it
//! into real directions. The lead (a native CLI session) calls the planner MCP
//! to read the repo map and `propose_directions`; the human reviews/edits in the
//! scope-confirm step, then confirms — which materializes worktrees.
//!
//! Repos are addressed by NAME across the MCP boundary (the lead reasons over
//! names from the repo map); resolution to ids happens here against the
//! workspace, so an unknown name is surfaced, never silently dropped.

use crate::materialize;
use crate::store::{repo, Db};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// One proposed work line: a tool plus the repos it writes / reads, by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedDirection {
    pub name: String,
    pub tool: String,
    #[serde(default)]
    pub writes: Vec<String>,
    #[serde(default)]
    pub reads: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub directions: Vec<ProposedDirection>,
}

/// A repo in a resolved direction's scope: id (-1 if the name is unknown), the
/// name as written, the role, and whether it matched a workspace repo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScopeEntry {
    pub repo_id: i32,
    pub repo_name: String,
    pub role: String,
    pub known: bool,
}

/// A direction resolved against the workspace's repos, ready for the UI / confirm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedDirection {
    pub name: String,
    pub tool: String,
    pub scope: Vec<ScopeEntry>,
}

/// Resolve one proposed direction's repo names to workspace repo ids + roles.
/// `repos` is (id, name) for the workspace. Writes take precedence if a name
/// appears in both lists; unknown names are kept with `known = false`.
pub fn resolve(dir: &ProposedDirection, repos: &[(i32, String)]) -> ResolvedDirection {
    let id_of = |name: &str| repos.iter().find(|(_, n)| n == name).map(|(id, _)| *id);
    let mut scope = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (names, role) in [(&dir.writes, "write"), (&dir.reads, "read")] {
        for name in names {
            if !seen.insert(name.clone()) {
                continue; // a write already claimed this name; don't add as read
            }
            match id_of(name) {
                Some(id) => scope.push(ScopeEntry {
                    repo_id: id,
                    repo_name: name.clone(),
                    role: role.to_string(),
                    known: true,
                }),
                None => scope.push(ScopeEntry {
                    repo_id: -1,
                    repo_name: name.clone(),
                    role: role.to_string(),
                    known: false,
                }),
            }
        }
    }
    ResolvedDirection {
        name: dir.name.clone(),
        tool: dir.tool.clone(),
        scope,
    }
}

// ---- DB orchestration ----

fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Store (replace) the proposal for a thread, status = "proposed".
pub async fn save_proposal(db: &Db, thread_id: i32, proposal: &Proposal) -> Result<()> {
    let json = serde_json::to_string(proposal)?;
    repo::upsert_plan(db, thread_id, &json, "proposed", &now()).await?;
    Ok(())
}

/// The stored proposal for a thread, resolved against its workspace repos.
pub async fn get_resolved(db: &Db, thread_id: i32) -> Result<Option<ResolvedProposal>> {
    let Some(p) = repo::get_plan(db, thread_id).await? else {
        return Ok(None);
    };
    let proposal: Proposal = serde_json::from_str(&p.proposal).unwrap_or_default();
    let repos = workspace_repos(db, thread_id).await?;
    let directions = proposal.directions.iter().map(|d| resolve(d, &repos)).collect();
    Ok(Some(ResolvedProposal {
        thread_id,
        rationale: proposal.rationale,
        status: p.status,
        directions,
    }))
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedProposal {
    pub thread_id: i32,
    pub rationale: String,
    pub status: String,
    pub directions: Vec<ResolvedDirection>,
}

/// Confirm the stored proposal: create each direction with its known-repo scope
/// and materialize its worktrees. Marks the plan confirmed. Unknown repo names
/// are skipped (they never resolved to a worktree-able repo).
pub async fn confirm(db: &Db, thread_id: i32) -> Result<Vec<i32>> {
    let resolved = get_resolved(db, thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no proposal to confirm for thread {thread_id}"))?;
    let mut created = Vec::new();
    for d in &resolved.directions {
        let scope: Vec<(i32, String)> = d
            .scope
            .iter()
            .filter(|s| s.known)
            .map(|s| (s.repo_id, s.role.clone()))
            .collect();
        let dir = repo::create_direction(db, thread_id, &d.name, &d.tool, &scope).await?;
        materialize::materialize_direction(db, dir.id).await?;
        created.push(dir.id);
    }
    if let Some(p) = repo::get_plan(db, thread_id).await? {
        repo::upsert_plan(db, thread_id, &p.proposal, "confirmed", &p.created_at).await?;
    }
    Ok(created)
}

async fn workspace_repos(db: &Db, thread_id: i32) -> Result<Vec<(i32, String)>> {
    let t = repo::get_thread(db, thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread {thread_id} not found"))?;
    let repos = repo::list_repos(db, t.workspace_id).await?;
    Ok(repos.into_iter().map(|r| (r.id, r.name)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repos() -> Vec<(i32, String)> {
        vec![
            (1, "web-app".into()),
            (2, "api".into()),
            (3, "shared-lib".into()),
        ]
    }

    #[test]
    fn resolves_writes_and_reads_to_ids() {
        let d = ProposedDirection {
            name: "Payments".into(),
            tool: "claude".into(),
            writes: vec!["web-app".into(), "api".into()],
            reads: vec!["shared-lib".into()],
        };
        let r = resolve(&d, &repos());
        assert_eq!(r.name, "Payments");
        assert_eq!(r.scope.len(), 3);
        assert_eq!(r.scope[0], ScopeEntry { repo_id: 1, repo_name: "web-app".into(), role: "write".into(), known: true });
        assert_eq!(r.scope[2], ScopeEntry { repo_id: 3, repo_name: "shared-lib".into(), role: "read".into(), known: true });
    }

    #[test]
    fn unknown_repo_name_is_flagged_not_dropped() {
        let d = ProposedDirection {
            name: "X".into(),
            tool: "codex".into(),
            writes: vec!["ghost-repo".into()],
            reads: vec![],
        };
        let r = resolve(&d, &repos());
        assert_eq!(r.scope.len(), 1);
        assert!(!r.scope[0].known);
        assert_eq!(r.scope[0].repo_id, -1);
    }

    #[test]
    fn write_wins_when_name_in_both_lists() {
        let d = ProposedDirection {
            name: "X".into(),
            tool: "claude".into(),
            writes: vec!["api".into()],
            reads: vec!["api".into()],
        };
        let r = resolve(&d, &repos());
        assert_eq!(r.scope.len(), 1);
        assert_eq!(r.scope[0].role, "write");
    }

    #[test]
    fn proposal_parses_with_missing_optional_fields() {
        let p: Proposal = serde_json::from_str(
            r#"{ "directions": [ { "name": "only writes", "tool": "claude", "writes": ["api"] } ] }"#,
        )
        .unwrap();
        assert_eq!(p.rationale, "");
        assert_eq!(p.directions.len(), 1);
        assert!(p.directions[0].reads.is_empty());
    }
}
