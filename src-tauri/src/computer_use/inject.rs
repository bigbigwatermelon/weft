#![allow(dead_code)]

use crate::bus::inject::Injection;
use std::path::{Path, PathBuf};

const SERVER: &str = "open_computer_use";
const STEM: &str = "computer-use";
const CLAUDE_CONFIG: &str = ".atlas-computer-use.mcp.json";
const OPENCODE_SCHEMA: &str = "https://opencode.ai/config.json";
const OPENCODE_OWNER_ENV: &str = "ATLAS_COMPUTER_USE_OWNER";
const OPENCODE_OWNER_VALUE: &str = "atlas";

pub fn build_stdio_injection(tool: &str, cwd: &Path, helper: &Path) -> Injection {
    match tool {
        "claude" => inject_claude(cwd, helper),
        "codex" => inject_codex(helper),
        "opencode" => {
            merge_opencode_config(cwd, helper);
            Injection { args: vec![] }
        }
        _ => Injection { args: vec![] },
    }
}

pub fn empty_injection() -> Injection {
    Injection { args: vec![] }
}

pub async fn maybe_inject(
    app: &tauri::AppHandle,
    db: &crate::store::Db,
    tool: &str,
    cwd: &Path,
) -> Injection {
    let enabled = crate::computer_use::settings::enabled(db)
        .await
        .unwrap_or(false);
    inject_for_enabled(app, tool, cwd, enabled)
}

pub fn inject_for_enabled(
    app: &tauri::AppHandle,
    tool: &str,
    cwd: &Path,
    enabled: bool,
) -> Injection {
    if !enabled {
        let info = crate::computer_use::helper::resolve_helper_path(Some(app));
        cleanup_managed_config(tool, cwd, info.path.as_deref().map(Path::new));
        return empty_injection();
    }
    if !cfg!(target_os = "macos") {
        return empty_injection();
    }
    let info = crate::computer_use::helper::resolve_helper_path(Some(app));
    inject_for_helper_info(tool, cwd, info)
}

fn inject_for_helper_info(
    tool: &str,
    cwd: &Path,
    info: crate::computer_use::helper::HelperInfo,
) -> Injection {
    if info.state != crate::computer_use::helper::HelperState::Found {
        cleanup_managed_config(tool, cwd, info.path.as_deref().map(Path::new));
        return empty_injection();
    }
    match info.path {
        Some(path) => build_stdio_injection(tool, cwd, &PathBuf::from(path)),
        None => empty_injection(),
    }
}

fn inject_claude(cwd: &Path, helper: &Path) -> Injection {
    let cfg = claude_config_path(cwd);
    let json = serde_json::json!({
        "mcpServers": {
            SERVER: {
                "command": helper.to_string_lossy(),
                "args": ["mcp"],
            }
        }
    });
    let Ok(bytes) = serde_json::to_vec_pretty(&json) else {
        return Injection { args: vec![] };
    };
    if std::fs::write(&cfg, bytes).is_err() {
        return Injection { args: vec![] };
    }
    crate::git::git_exclude(cwd, CLAUDE_CONFIG);
    Injection {
        args: vec!["--mcp-config".into(), cfg.to_string_lossy().to_string()],
    }
}

fn inject_codex(helper: &Path) -> Injection {
    Injection {
        args: vec![
            "-c".into(),
            format!(
                "mcp_servers.{SERVER}.command={}",
                toml_quote(&helper.to_string_lossy())
            ),
            "-c".into(),
            format!("mcp_servers.{SERVER}.args=[\"mcp\"]"),
        ],
    }
}

fn merge_opencode_config(cwd: &Path, helper: &Path) {
    let path = cwd.join("opencode.json");
    let mut root: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => return,
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(_) => return,
    };
    if !root.is_object() {
        return;
    }
    let Some(obj) = root.as_object_mut() else {
        return;
    };
    obj.entry("$schema".to_string())
        .or_insert_with(|| serde_json::json!(OPENCODE_SCHEMA));

    let mcp = obj
        .entry("mcp".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !mcp.is_object() {
        *mcp = serde_json::json!({});
    }
    if let Some(mcp_obj) = mcp.as_object_mut() {
        if mcp_obj
            .get(SERVER)
            .is_some_and(|server| !is_atlas_opencode_server(server, Some(helper)))
        {
            return;
        }
        let mut server = serde_json::json!({
            "type": "local",
            "command": [helper.to_string_lossy(), "mcp"],
            "enabled": true,
        });
        if let Some(server_obj) = server.as_object_mut() {
            let mut environment = serde_json::Map::new();
            environment.insert(
                OPENCODE_OWNER_ENV.to_string(),
                serde_json::json!(OPENCODE_OWNER_VALUE),
            );
            server_obj.insert(
                "environment".to_string(),
                serde_json::Value::Object(environment),
            );
        }
        mcp_obj.insert(SERVER.to_string(), server);
    }

    if let Ok(bytes) = serde_json::to_vec_pretty(&root) {
        let _ = std::fs::write(&path, bytes);
    }
    crate::git::git_exclude(cwd, "opencode.json");
}

fn cleanup_managed_config(tool: &str, cwd: &Path, configured_helper: Option<&Path>) {
    if tool == "opencode" {
        remove_opencode_server(cwd, configured_helper);
    }
}

fn remove_opencode_server(cwd: &Path, configured_helper: Option<&Path>) {
    let path = cwd.join("opencode.json");
    let mut root: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => return,
        },
        Err(_) => return,
    };
    let Some(obj) = root.as_object_mut() else {
        return;
    };
    let Some(mcp) = obj.get_mut("mcp").and_then(|v| v.as_object_mut()) else {
        return;
    };
    let should_remove = mcp
        .get(SERVER)
        .map(|server| is_atlas_opencode_server(server, configured_helper))
        .unwrap_or(false);
    if !should_remove {
        return;
    }
    mcp.remove(SERVER);
    if let Ok(bytes) = serde_json::to_vec_pretty(&root) {
        let _ = std::fs::write(&path, bytes);
    }
}

fn is_atlas_opencode_server(value: &serde_json::Value, configured_helper: Option<&Path>) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    if obj.get("type").and_then(|v| v.as_str()) != Some("local") {
        return false;
    }
    let Some(command) = obj.get("command").and_then(|v| v.as_array()) else {
        return false;
    };
    let helper = command.first().and_then(|v| v.as_str()).unwrap_or("");
    if !command_is_mcp(command) {
        return false;
    }
    if has_atlas_opencode_marker(obj) {
        return true;
    }
    if obj.get("enabled").and_then(|v| v.as_bool()) != Some(true) {
        return false;
    }
    if !is_legacy_atlas_opencode_shape(obj) {
        return false;
    }
    configured_helper.is_some_and(|path| helper_matches_path(helper, path))
        || is_atlas_bundled_helper_command(helper)
}

fn command_is_mcp(command: &[serde_json::Value]) -> bool {
    command.len() == 2
        && command
            .get(1)
            .and_then(|v| v.as_str())
            .is_some_and(|arg| arg == "mcp")
}

fn has_atlas_opencode_marker(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    obj.get("environment")
        .and_then(|v| v.as_object())
        .and_then(|env| env.get(OPENCODE_OWNER_ENV))
        .and_then(|v| v.as_str())
        == Some(OPENCODE_OWNER_VALUE)
}

fn is_legacy_atlas_opencode_shape(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    obj.len() == 3
        && obj.contains_key("type")
        && obj.contains_key("command")
        && obj.contains_key("enabled")
}

fn helper_matches_path(command: &str, helper: &Path) -> bool {
    Path::new(command) == helper
}

fn is_atlas_bundled_helper_command(command: &str) -> bool {
    let path = Path::new(command);
    if path.file_name().and_then(|v| v.to_str()) != Some(crate::computer_use::helper::HELPER_NAME) {
        return false;
    }
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|v| v.to_str())
        == Some("sidecars")
}

fn claude_config_path(cwd: &Path) -> PathBuf {
    cwd.join(format!(".atlas-{STEM}.mcp.json"))
}

fn toml_quote(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::computer_use::helper::{HelperInfo, HelperState};
    use serde_json::Value;

    use super::*;

    #[test]
    fn claude_injection_writes_stdio_mcp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");

        let injection = build_stdio_injection("claude", tmp.path(), &helper);

        assert_eq!(injection.args[0], "--mcp-config");
        assert_eq!(
            injection.args[1],
            tmp.path()
                .join(".atlas-computer-use.mcp.json")
                .to_string_lossy()
        );

        let raw = std::fs::read_to_string(tmp.path().join(".atlas-computer-use.mcp.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        let server = &config["mcpServers"]["open_computer_use"];
        assert_eq!(server["command"], helper.to_string_lossy().as_ref());
        assert_eq!(server["args"], serde_json::json!(["mcp"]));
    }

    #[test]
    fn claude_config_write_failure_returns_no_usable_injection() {
        let tmp = tempfile::tempdir().unwrap();
        let missing_cwd = tmp.path().join("missing-cwd");

        let injection =
            build_stdio_injection("claude", &missing_cwd, Path::new("/tmp/open-computer-use"));

        assert!(injection.args.is_empty());
        assert!(!missing_cwd.exists());
    }

    #[test]
    fn codex_injection_uses_inline_stdio_config() {
        let helper = Path::new("/tmp/open-computer-use");

        let injection = build_stdio_injection("codex", Path::new("/unused"), helper);

        assert_eq!(
            injection.args,
            vec![
                "-c".to_string(),
                "mcp_servers.open_computer_use.command=\"/tmp/open-computer-use\"".to_string(),
                "-c".to_string(),
                "mcp_servers.open_computer_use.args=[\"mcp\"]".to_string(),
            ]
        );
    }

    #[test]
    fn codex_injection_toml_quotes_helper_paths_with_spaces_quotes_and_backslashes() {
        let helper = r#"/tmp/open computer "use" \bin/open-computer-use"#;

        let injection = build_stdio_injection("codex", Path::new("/unused"), Path::new(helper));

        let command_value = injection.args[1]
            .strip_prefix("mcp_servers.open_computer_use.command=")
            .unwrap();
        let parsed: toml::Value = toml::from_str(&format!("value = {command_value}")).unwrap();
        assert_eq!(parsed["value"].as_str().unwrap(), helper);
    }

    #[test]
    fn opencode_injection_preserves_existing_config_and_adds_local_mcp() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "theme": "system",
                "mcp": {
                    "existing": {
                        "type": "remote",
                        "url": "http://127.0.0.1:9/mcp",
                        "enabled": false
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        let injection = build_stdio_injection("opencode", tmp.path(), &helper);

        assert!(injection.args.is_empty());
        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(config["$schema"], "https://opencode.ai/config.json");
        assert_eq!(config["theme"], "system");
        assert_eq!(config["mcp"]["existing"]["url"], "http://127.0.0.1:9/mcp");
        assert_eq!(config["mcp"]["existing"]["enabled"], false);

        let server = &config["mcp"]["open_computer_use"];
        assert_eq!(server["type"], "local");
        assert_eq!(
            server["command"],
            serde_json::json!([helper.to_string_lossy(), "mcp"])
        );
        assert_eq!(server["enabled"], true);
        assert_eq!(
            server["environment"][OPENCODE_OWNER_ENV],
            OPENCODE_OWNER_VALUE
        );
    }

    #[test]
    fn opencode_invalid_existing_config_is_preserved_and_not_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let invalid_config = "{ invalid json";
        let config_path = tmp.path().join("opencode.json");
        std::fs::write(&config_path, invalid_config).unwrap();

        let injection =
            build_stdio_injection("opencode", tmp.path(), Path::new("/tmp/open-computer-use"));

        assert!(injection.args.is_empty());
        assert_eq!(
            std::fs::read_to_string(config_path).unwrap(),
            invalid_config
        );
    }

    #[test]
    fn opencode_injection_preserves_user_managed_same_name_server() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "mcp": {
                    "open_computer_use": {
                        "type": "remote",
                        "url": "http://127.0.0.1:7/mcp",
                        "enabled": true
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        let injection = build_stdio_injection("opencode", tmp.path(), &helper);

        assert!(injection.args.is_empty());
        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            config["mcp"]["open_computer_use"]["url"],
            "http://127.0.0.1:7/mcp"
        );
        assert!(config["mcp"]["open_computer_use"]
            .get("environment")
            .is_none());
    }

    #[test]
    fn opencode_missing_helper_cleans_stale_managed_config() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "mcp": {
                    "open_computer_use": {
                        "type": "local",
                        "command": [helper.to_string_lossy(), "mcp"],
                        "enabled": true,
                        "environment": {
                            "ATLAS_COMPUTER_USE_OWNER": OPENCODE_OWNER_VALUE
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        let injection = inject_for_helper_info(
            "opencode",
            tmp.path(),
            HelperInfo {
                state: HelperState::Missing,
                path: Some(helper.to_string_lossy().into_owned()),
                error: Some("helper not found".to_string()),
            },
        );

        assert!(injection.args.is_empty());
        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert!(config["mcp"].get("open_computer_use").is_none());
    }

    #[test]
    fn opencode_cleanup_removes_only_atlas_managed_server() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("sidecars").join("open-computer-use");
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "$schema": OPENCODE_SCHEMA,
                "theme": "system",
                "mcp": {
                    "open_computer_use": {
                        "type": "local",
                        "command": [helper.to_string_lossy(), "mcp"],
                        "enabled": true
                    },
                    "existing": {
                        "type": "remote",
                        "url": "http://127.0.0.1:9/mcp",
                        "enabled": false
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        cleanup_managed_config("opencode", tmp.path(), None);

        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(config["$schema"], OPENCODE_SCHEMA);
        assert_eq!(config["theme"], "system");
        assert!(config["mcp"].get("open_computer_use").is_none());
        assert_eq!(config["mcp"]["existing"]["url"], "http://127.0.0.1:9/mcp");
    }

    #[test]
    fn opencode_cleanup_removes_configured_custom_helper() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("bin").join("open-computer-use");
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "mcp": {
                    "open_computer_use": {
                        "type": "local",
                        "command": [helper.to_string_lossy(), "mcp"],
                        "enabled": true
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        cleanup_managed_config("opencode", tmp.path(), Some(&helper));

        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert!(config["mcp"].get("open_computer_use").is_none());
    }

    #[test]
    fn opencode_cleanup_preserves_user_managed_same_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "mcp": {
                    "open_computer_use": {
                        "type": "remote",
                        "url": "http://127.0.0.1:7/mcp",
                        "enabled": true
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        cleanup_managed_config("opencode", tmp.path(), None);

        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            config["mcp"]["open_computer_use"]["url"],
            "http://127.0.0.1:7/mcp"
        );
    }

    #[test]
    fn opencode_cleanup_preserves_user_managed_same_name_local_server() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::json!({
                "mcp": {
                    "open_computer_use": {
                        "type": "local",
                        "command": ["/Users/me/bin/my-computer-use", "mcp"],
                        "enabled": true
                    }
                }
            })
            .to_string(),
        )
        .unwrap();

        cleanup_managed_config("opencode", tmp.path(), None);

        let raw = std::fs::read_to_string(tmp.path().join("opencode.json")).unwrap();
        let config: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            config["mcp"]["open_computer_use"]["command"],
            serde_json::json!(["/Users/me/bin/my-computer-use", "mcp"])
        );
    }

    #[test]
    fn unknown_tool_gets_no_injection() {
        let tmp = tempfile::tempdir().unwrap();
        let injection =
            build_stdio_injection("unknown", tmp.path(), Path::new("/tmp/open-computer-use"));

        assert!(injection.args.is_empty());
        assert!(!tmp.path().join(".atlas-computer-use.mcp.json").exists());
        assert!(!tmp.path().join("opencode.json").exists());
    }
}
