use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

pub const ENV_HELPER: &str = "ATLAS_COMPUTER_USE_HELPER";
pub const HELPER_NAME: &str = "open-computer-use";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperState {
    Missing,
    NotExecutable,
    Found,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelperInfo {
    pub state: HelperState,
    pub path: Option<String>,
    pub error: Option<String>,
}

pub fn resolve_helper_path(app: Option<&AppHandle>) -> HelperInfo {
    let env_value = std::env::var(ENV_HELPER).ok();
    let resource_dir = app.and_then(|app| app.path().resource_dir().ok());

    resolve_helper_path_from(
        env_value.as_deref(),
        resource_dir.as_deref(),
        &manifest_dir(),
    )
}

fn resolve_helper_path_from(
    env_value: Option<&str>,
    resource_dir: Option<&Path>,
    manifest_dir: &Path,
) -> HelperInfo {
    if let Some(path) = env_value {
        if !path.trim().is_empty() {
            return validate_helper_path(PathBuf::from(path));
        }
    }

    if let Some(dir) = resource_dir {
        let info = validate_helper_path(dir.join("sidecars").join(HELPER_NAME));
        if info.state == HelperState::Found {
            return info;
        }
    }

    validate_helper_path(manifest_dir.join("sidecars").join(HELPER_NAME))
}

pub fn validate_helper_path(path: impl AsRef<Path>) -> HelperInfo {
    let path = path.as_ref();
    let path_text = path.to_string_lossy().into_owned();

    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return HelperInfo {
                state: HelperState::Missing,
                path: Some(path_text),
                error: Some(format!("helper not found: {}", path.display())),
            };
        }
        Err(err) => {
            return HelperInfo {
                state: HelperState::NotExecutable,
                path: Some(path_text),
                error: Some(format!(
                    "could not inspect helper {}: {err}",
                    path.display()
                )),
            };
        }
    };

    if !metadata.is_file() {
        return HelperInfo {
            state: HelperState::NotExecutable,
            path: Some(path_text),
            error: Some(format!("helper is not a file: {}", path.display())),
        };
    }

    if !is_executable(&metadata) {
        return HelperInfo {
            state: HelperState::NotExecutable,
            path: Some(path_text),
            error: Some(format!("helper is not executable: {}", path.display())),
        };
    }

    HelperInfo {
        state: HelperState::Found,
        path: Some(path_text),
        error: None,
    }
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper_path(root: &Path) -> PathBuf {
        root.join("sidecars").join(HELPER_NAME)
    }

    fn write_executable_helper(path: &Path) {
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    fn missing_helper_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("missing-helper");

        let info = validate_helper_path(&helper);

        assert_eq!(info.state, HelperState::Missing);
        assert_eq!(
            info.path.as_deref(),
            Some(helper.to_string_lossy().as_ref())
        );
        assert!(info.error.unwrap().contains("helper not found"));
    }

    #[test]
    fn env_helper_wins_over_resource_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let env_helper = tmp.path().join("env-helper");
        let resource_dir = tmp.path().join("resource");
        let manifest_dir = tmp.path().join("manifest");
        write_executable_helper(&env_helper);
        write_executable_helper(&helper_path(&resource_dir));
        write_executable_helper(&helper_path(&manifest_dir));

        let info = resolve_helper_path_from(
            Some(env_helper.to_string_lossy().as_ref()),
            Some(&resource_dir),
            &manifest_dir,
        );

        assert_eq!(info.state, HelperState::Found);
        assert_eq!(
            info.path.as_deref(),
            Some(env_helper.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn resource_helper_wins_over_manifest_when_usable() {
        let tmp = tempfile::tempdir().unwrap();
        let resource_dir = tmp.path().join("resource");
        let manifest_dir = tmp.path().join("manifest");
        write_executable_helper(&helper_path(&resource_dir));
        write_executable_helper(&helper_path(&manifest_dir));

        let info = resolve_helper_path_from(None, Some(&resource_dir), &manifest_dir);

        assert_eq!(info.state, HelperState::Found);
        assert_eq!(
            info.path.as_deref(),
            Some(helper_path(&resource_dir).to_string_lossy().as_ref())
        );
    }

    #[test]
    fn manifest_helper_is_used_when_resource_helper_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let resource_dir = tmp.path().join("resource");
        let manifest_dir = tmp.path().join("manifest");
        write_executable_helper(&helper_path(&manifest_dir));

        let info = resolve_helper_path_from(None, Some(&resource_dir), &manifest_dir);

        assert_eq!(info.state, HelperState::Found);
        assert_eq!(
            info.path.as_deref(),
            Some(helper_path(&manifest_dir).to_string_lossy().as_ref())
        );
    }

    #[cfg(unix)]
    #[test]
    fn manifest_helper_is_used_when_resource_helper_is_not_executable() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let resource_dir = tmp.path().join("resource");
        let manifest_dir = tmp.path().join("manifest");
        let resource_helper = helper_path(&resource_dir);
        let parent = resource_helper.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();
        std::fs::write(&resource_helper, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&resource_helper, std::fs::Permissions::from_mode(0o600)).unwrap();
        write_executable_helper(&helper_path(&manifest_dir));

        let info = resolve_helper_path_from(None, Some(&resource_dir), &manifest_dir);

        assert_eq!(info.state, HelperState::Found);
        assert_eq!(
            info.path.as_deref(),
            Some(helper_path(&manifest_dir).to_string_lossy().as_ref())
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_helper_is_reported() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");
        std::fs::write(&helper, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o600)).unwrap();

        let info = validate_helper_path(&helper);

        assert_eq!(info.state, HelperState::NotExecutable);
        assert_eq!(
            info.path.as_deref(),
            Some(helper.to_string_lossy().as_ref())
        );
        assert!(info.error.unwrap().contains("not executable"));
    }

    #[cfg(unix)]
    #[test]
    fn executable_helper_is_found() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("open-computer-use");
        std::fs::write(&helper, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();

        let info = validate_helper_path(&helper);

        assert_eq!(info.state, HelperState::Found);
        assert_eq!(
            info.path.as_deref(),
            Some(helper.to_string_lossy().as_ref())
        );
        assert_eq!(info.error, None);
    }
}
