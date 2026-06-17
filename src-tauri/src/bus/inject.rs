//! Spawn-time, ADDITIVE injection of the thread bus as an MCP server for each
//! tool. Claude/Codex use launch flags; OpenCode deep-merges into a local
//! opencode.json in the target cwd.

use std::path::Path;

/// Extra args to PREPEND to the tool's own args (global flags must precede any
/// subcommand, e.g. `codex -c k=v resume <id>`).
pub struct Injection {
    pub args: Vec<String>,
}

fn mcp_url(base: &str, thread: i32, dir: &str) -> String {
    format!("{base}/bus/{thread}/{dir}/mcp")
}

fn planner_url(base: &str, thread: i32) -> String {
    format!("{base}/planner/{thread}/mcp")
}

fn global_url(base: &str) -> String {
    format!("{base}/global/mcp")
}

fn ask_url(base: &str, thread: i32, dir: &str, tool: &str) -> String {
    format!("{base}/ask/{thread}/{dir}?tool={tool}")
}

/// Install the Ask Bridge for a session: a PreToolUse hook that POSTs each tool
/// action to atlas's /ask endpoint and blocks on the returned allow/deny. Both
/// claude and codex use the IDENTICAL hookSpecificOutput contract, so the hook
/// script is shared; only the per-tool wiring differs (claude `--settings`,
/// codex `-c hooks.PreToolUse`). Additive — stacks with the user's own hooks.
/// Best-effort: empty args if files can't be written. (OpenCode bridges via its
/// server `/event` channel, not a hook — handled elsewhere.)
pub fn inject_ask_hook(base: &str, thread: i32, dir: &str, tool: &str, cwd: &Path) -> Injection {
    if tool == "opencode" {
        return inject_opencode_ask_plugin(base, thread, dir, cwd);
    }
    if tool != "claude" && tool != "codex" {
        return Injection { args: vec![] };
    }
    let url = ask_url(base, thread, dir, tool);
    let script = cwd.join(".atlas-ask-hook.sh");
    // Reads the PreToolUse JSON on stdin, asks atlas, echoes atlas's decision JSON
    // (empty on failure/timeout → the tool falls back to its own prompt).
    // -m matches the server's ASK_WAIT: hold the call until the human answers in
    // Needs-you rather than timing out into the tool's own hidden prompt.
    let body = format!(
        "#!/usr/bin/env bash\n\
         resp=$(curl -s -m 3600 -X POST '{url}' -H 'Content-Type: application/json' --data-binary @- 2>/dev/null)\n\
         [ -n \"$resp\" ] && printf '%s' \"$resp\"\n\
         exit 0\n"
    );
    if std::fs::write(&script, body).is_err() {
        return Injection { args: vec![] };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
    }
    crate::git::git_exclude(cwd, ".atlas-ask-hook.sh");

    match tool {
        "claude" => {
            let settings = cwd.join(".atlas-ask.settings.json");
            let json = serde_json::json!({
                "hooks": { "PreToolUse": [
                    { "matcher": "*", "hooks": [
                        { "type": "command",
                          "command": format!("bash {}", script.to_string_lossy()),
                          "timeout": 3650 }
                    ] }
                ] }
            });
            if std::fs::write(
                &settings,
                serde_json::to_vec_pretty(&json).unwrap_or_default(),
            )
            .is_err()
            {
                return Injection { args: vec![] };
            }
            crate::git::git_exclude(cwd, ".atlas-ask.settings.json");
            Injection {
                args: vec!["--settings".into(), settings.to_string_lossy().to_string()],
            }
        }
        // Codex defines the same hook in config.toml. We pass it inline via `-c`
        // and add --dangerously-bypass-hook-trust: that flag only waives
        // SOURCE-trust for our own generated hook so it runs unattended — it does
        // NOT skip approvals or the sandbox (the hook IS the approval surface).
        "codex" => {
            let hooks = format!(
                "hooks.PreToolUse=[{{ matcher = \".*\", hooks = [{{ type = \"command\", command = \"bash {}\" }}] }}]",
                script.to_string_lossy()
            );
            Injection {
                args: vec!["--dangerously-bypass-hook-trust".into(), "-c".into(), hooks],
            }
        }
        _ => Injection { args: vec![] },
    }
}

/// OpenCode has no PreToolUse hook; its analog is a local plugin's
/// `tool.execute.before`, which is async and throws to deny. Drop a plugin in
/// the cwd's `.opencode/plugins/` that POSTs each tool action to atlas's
/// /ask endpoint and throws on a deny verdict — same Ask Bridge, same endpoint,
/// same allow/deny contract as claude/codex. Auto-loaded (no launch flag).
fn inject_opencode_ask_plugin(base: &str, thread: i32, dir: &str, cwd: &Path) -> Injection {
    let url = ask_url(base, thread, dir, "opencode");
    let plugins = cwd.join(".opencode").join("plugins");
    if std::fs::create_dir_all(&plugins).is_err() {
        return Injection { args: vec![] };
    }
    let template = r#"// atlas Ask Bridge — surfaces tool approvals to atlas, blocks on deny.
export const AtlasAsk = async () => ({
  "tool.execute.before": async (input, output) => {
    let decision;
    try {
      const res = await fetch("__URL__", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ tool_name: input.tool, tool_input: output.args }),
      });
      decision = (await res.json())?.hookSpecificOutput?.permissionDecision;
    } catch (e) { /* atlas unreachable → fall back to opencode's own flow */ }
    if (decision === "deny") throw new Error("Denied in atlas");
  },
});
"#;
    let body = template.replace("__URL__", &url);
    let _ = std::fs::write(plugins.join("atlas-ask.js"), body);
    crate::git::git_exclude(cwd, ".opencode/plugins/atlas-ask.js");
    Injection { args: vec![] }
}

/// Build the thread-bus injection. `cwd` is the run directory used for generated
/// config files. `dir` is the direction id as a string.
pub fn inject(base: &str, thread: i32, dir: &str, tool: &str, cwd: &Path) -> Injection {
    inject_mcp("atlas_bus", "bus", &mcp_url(base, thread, dir), tool, cwd)
}

/// Build the planner-MCP injection for a lead session (read-only planning).
/// Same additive mechanism as the bus, a different server keyed to the thread.
pub fn inject_planner(base: &str, thread: i32, tool: &str, cwd: &Path) -> Injection {
    inject_mcp(
        "atlas_planner",
        "planner",
        &planner_url(base, thread),
        tool,
        cwd,
    )
}

/// Build the global-MCP injection for the Concierge engine (M3-2). Not
/// per-thread — the URL has no thread/dir in path; identity is "the global
/// helper running in IM single-chat". Same additive shape as planner.
pub fn inject_global(base: &str, tool: &str, cwd: &Path) -> Injection {
    inject_mcp("atlas_global", "global", &global_url(base), tool, cwd)
}

/// Additively register one HTTP MCP `server` at `url` for `tool`, never
/// overriding local config. `stem` names the claude temp config file
/// (`.atlas-<stem>.mcp.json`).
fn inject_mcp(server: &str, stem: &str, url: &str, tool: &str, cwd: &Path) -> Injection {
    match tool {
        "claude" => {
            // ephemeral --mcp-config file inside the cwd. It's an injected,
            // untracked file, so we add it to git exclude (see git_exclude) to
            // keep it out of `git status` / diffs / commits.
            let file = format!(".atlas-{stem}.mcp.json");
            let cfg = cwd.join(&file);
            let json = serde_json::json!({
                "mcpServers": { server: { "type": "http", "url": url } }
            });
            let _ = std::fs::write(&cfg, serde_json::to_vec_pretty(&json).unwrap_or_default());
            crate::git::git_exclude(cwd, &file);
            Injection {
                args: vec!["--mcp-config".into(), cfg.to_string_lossy().to_string()],
            }
        }
        "codex" => Injection {
            args: vec!["-c".into(), format!("mcp_servers.{server}.url={url}")],
        },
        "opencode" => {
            merge_opencode_config(cwd, server, url);
            Injection { args: vec![] }
        }
        _ => Injection { args: vec![] },
    }
}

/// Deep-merge `mcp.<server> = {type:remote, url, enabled:true}` into the cwd's
/// opencode.json, preserving any existing local config.
fn merge_opencode_config(cwd: &Path, server: &str, url: &str) {
    let path = cwd.join("opencode.json");
    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    // root is guaranteed an object here; guard instead of unwrap so a panic is
    // impossible even if the invariant ever changes.
    let Some(obj) = root.as_object_mut() else {
        return;
    };
    obj.entry("$schema".to_string())
        .or_insert_with(|| serde_json::json!("https://opencode.ai/config.json"));
    let mcp = obj
        .entry("mcp".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(mcp_obj) = mcp.as_object_mut() {
        mcp_obj.insert(
            server.to_string(),
            serde_json::json!({ "type": "remote", "url": url, "enabled": true }),
        );
    }
    let _ = std::fs::write(&path, serde_json::to_vec_pretty(&root).unwrap_or_default());
    // Best-effort: only hides opencode.json from git when the current directory
    // does not already track it. If it is tracked, the merge still shows as a
    // modification.
    crate::git::git_exclude(cwd, "opencode.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_writes_mcp_config_and_flags() {
        let dir = std::env::temp_dir().join(format!("atlas-inj-claude-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inj = inject("http://127.0.0.1:9", 1, "10", "claude", &dir);
        assert_eq!(inj.args[0], "--mcp-config");
        let cfg = std::fs::read_to_string(dir.join(".atlas-bus.mcp.json")).unwrap();
        assert!(cfg.contains("atlas_bus") && cfg.contains("/bus/1/10/mcp"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_uses_config_override() {
        let inj = inject("http://127.0.0.1:9", 2, "30", "codex", Path::new("/tmp"));
        assert_eq!(
            inj.args,
            vec![
                "-c".to_string(),
                "mcp_servers.atlas_bus.url=http://127.0.0.1:9/bus/2/30/mcp".to_string()
            ]
        );
    }

    #[test]
    fn planner_claude_writes_its_own_config() {
        let dir = std::env::temp_dir().join(format!("atlas-inj-plan-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inj = inject_planner("http://127.0.0.1:9", 7, "claude", &dir);
        assert_eq!(inj.args[0], "--mcp-config");
        let cfg = std::fs::read_to_string(dir.join(".atlas-planner.mcp.json")).unwrap();
        assert!(cfg.contains("atlas_planner") && cfg.contains("/planner/7/mcp"));
        // the bus config is a SEPARATE file — planner doesn't clobber it
        assert_ne!(
            inj.args[1],
            dir.join(".atlas-bus.mcp.json").to_string_lossy()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn claude_ask_hook_wires_pretooluse_settings() {
        let dir = std::env::temp_dir().join(format!("atlas-askh-c-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inj = inject_ask_hook("http://127.0.0.1:9", 1, "10", "claude", &dir);
        assert_eq!(inj.args[0], "--settings");
        let script = std::fs::read_to_string(dir.join(".atlas-ask-hook.sh")).unwrap();
        assert!(script.contains("/ask/1/10?tool=claude"));
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(".atlas-ask.settings.json")).unwrap(),
        )
        .unwrap();
        assert!(settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains(".atlas-ask-hook.sh"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_ask_hook_injects_pretooluse_via_config() {
        let dir = std::env::temp_dir().join(format!("atlas-askh-x-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inj = inject_ask_hook("http://127.0.0.1:9", 2, "30", "codex", &dir);
        assert_eq!(inj.args[0], "--dangerously-bypass-hook-trust");
        assert_eq!(inj.args[1], "-c");
        assert!(inj.args[2].starts_with("hooks.PreToolUse=["));
        assert!(inj.args[2].contains(".atlas-ask-hook.sh"));
        let script = std::fs::read_to_string(dir.join(".atlas-ask-hook.sh")).unwrap();
        assert!(script.contains("/ask/2/30?tool=codex"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opencode_ask_plugin_written_and_excluded() {
        let dir = std::env::temp_dir().join(format!("atlas-inj-oask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let inj = inject_ask_hook("http://127.0.0.1:9", 1, "10", "opencode", &dir);
        assert!(
            inj.args.is_empty(),
            "opencode plugin auto-loads, no launch flag"
        );
        let plugin = std::fs::read_to_string(dir.join(".opencode/plugins/atlas-ask.js")).unwrap();
        assert!(plugin.contains("tool.execute.before"));
        assert!(plugin.contains("/ask/1/10?tool=opencode"));
        assert!(plugin.contains("Denied in atlas"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn planner_codex_override_targets_planner_server() {
        let inj = inject_planner("http://127.0.0.1:9", 3, "codex", Path::new("/tmp"));
        assert_eq!(
            inj.args,
            vec![
                "-c".to_string(),
                "mcp_servers.atlas_planner.url=http://127.0.0.1:9/planner/3/mcp".to_string()
            ]
        );
    }

    #[test]
    fn opencode_merges_preserving_existing() {
        let dir = std::env::temp_dir().join(format!("atlas-inj-oc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // sub-repo already ships an opencode.json with its own mcp server
        std::fs::write(
            dir.join("opencode.json"),
            r#"{"mcp":{"repo_own":{"type":"local","command":["x"]}}}"#,
        )
        .unwrap();
        let inj = inject("http://127.0.0.1:9", 1, "10", "opencode", &dir);
        assert!(inj.args.is_empty());
        let merged: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("opencode.json")).unwrap())
                .unwrap();
        // both the repo's server AND atlas_bus must be present
        assert!(
            merged["mcp"]["repo_own"].is_object(),
            "repo's own server preserved"
        );
        assert_eq!(merged["mcp"]["atlas_bus"]["type"], "remote");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn injected_file_is_git_excluded() {
        let root = std::env::temp_dir().join(format!("atlas-inj-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("dir");
        std::fs::create_dir_all(&dir).unwrap();
        let sh = |dir: &Path, args: &[&str]| {
            assert!(crate::git::command()
                .args(&args[1..])
                .current_dir(dir)
                .status()
                .unwrap()
                .success());
        };
        sh(&dir, &["git", "init", "-q"]);

        let _ = inject("http://127.0.0.1:9", 1, "1", "claude", &dir);
        assert!(dir.join(".atlas-bus.mcp.json").exists(), "file written");
        let status = crate::git::command()
            .args(["status", "--porcelain"])
            .current_dir(&dir)
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&status.stdout);
        assert!(
            !s.contains(".atlas-bus.mcp.json"),
            "injected file must be git-excluded, got: {s}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
