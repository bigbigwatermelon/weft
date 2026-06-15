use super::settings;
use crate::store::Db;
use anyhow::Result;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ComputerUseStatusKind {
    Disabled,
    UnsupportedPlatform,
    Missing,
    NotExecutable,
    Found,
    DoctorFailed,
    PermissionMissing,
    Ready,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputerUseStatus {
    pub enabled: bool,
    pub supported: bool,
    pub status: ComputerUseStatusKind,
    pub helper_path: Option<String>,
    pub helper_version: Option<String>,
    pub doctor_summary: String,
    pub error: Option<String>,
}

pub async fn status(_app: &AppHandle, db: &Db) -> Result<ComputerUseStatus> {
    let enabled = settings::enabled(db).await?;
    let supported = cfg!(target_os = "macos");
    let status = if enabled {
        ComputerUseStatusKind::Unknown
    } else {
        ComputerUseStatusKind::Disabled
    };
    let doctor_summary = if enabled {
        "Computer Use diagnostics are not available yet.".to_string()
    } else {
        "Computer Use is disabled.".to_string()
    };

    Ok(ComputerUseStatus {
        enabled,
        supported,
        status,
        helper_path: None,
        helper_version: None,
        doctor_summary,
        error: None,
    })
}

pub async fn run_doctor_text(_app: &AppHandle) -> Result<String> {
    Ok("Computer Use diagnostics are not available yet.".to_string())
}
