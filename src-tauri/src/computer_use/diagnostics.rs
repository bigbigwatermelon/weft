use super::helper::{self, HelperInfo, HelperState};
use super::settings;
use crate::store::Db;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tauri::AppHandle;
use tokio::process::Command;

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

struct HelperCommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl HelperCommandOutput {
    fn combined_text(&self) -> String {
        combine_text(&self.stdout, &self.stderr)
    }
}

pub async fn status(app: &AppHandle, db: &Db) -> Result<ComputerUseStatus> {
    let enabled = settings::enabled(db).await?;
    let supported = cfg!(target_os = "macos");
    if !enabled {
        return Ok(ComputerUseStatus {
            enabled,
            supported,
            status: ComputerUseStatusKind::Disabled,
            helper_path: None,
            helper_version: None,
            doctor_summary: "Computer Use is disabled.".to_string(),
            error: None,
        });
    }

    if !supported {
        return Ok(ComputerUseStatus {
            enabled,
            supported,
            status: ComputerUseStatusKind::UnsupportedPlatform,
            helper_path: None,
            helper_version: None,
            doctor_summary: "Computer Use is only supported on macOS.".to_string(),
            error: None,
        });
    }

    let helper = helper::resolve_helper_path(Some(app));
    let helper_path = helper.path.clone();
    match helper.state {
        HelperState::Missing => {
            return Ok(ComputerUseStatus {
                enabled,
                supported,
                status: ComputerUseStatusKind::Missing,
                helper_path,
                helper_version: None,
                doctor_summary: "Computer Use helper is missing.".to_string(),
                error: helper.error,
            });
        }
        HelperState::NotExecutable => {
            return Ok(ComputerUseStatus {
                enabled,
                supported,
                status: ComputerUseStatusKind::NotExecutable,
                helper_path,
                helper_version: None,
                doctor_summary: "Computer Use helper is not executable.".to_string(),
                error: helper.error,
            });
        }
        HelperState::Found => {}
    }

    let helper_bin = executable_path(&helper)?;
    let helper_version = match run_helper(&helper_bin, &["--version"], Duration::from_secs(2)).await
    {
        Ok(output) if output.status.success() => optional_text(output.combined_text()),
        _ => None,
    };

    let doctor = run_helper(&helper_bin, &["doctor"], Duration::from_secs(5)).await;
    let (status, doctor_summary, error) = match doctor {
        Ok(output) if output.status.success() => {
            let summary = output.combined_text();
            let status = classify_doctor(&summary);
            (status, summary_or_default(summary), None)
        }
        Ok(output) => {
            let summary = summary_or_default(output.combined_text());
            let error = Some(format!("doctor exited with status {}", output.status));
            (ComputerUseStatusKind::DoctorFailed, summary, error)
        }
        Err(err) => (
            ComputerUseStatusKind::DoctorFailed,
            "Computer Use doctor failed.".to_string(),
            Some(err.to_string()),
        ),
    };

    Ok(ComputerUseStatus {
        enabled,
        supported,
        status,
        helper_path,
        helper_version,
        doctor_summary,
        error,
    })
}

pub async fn run_doctor_text(app: &AppHandle) -> Result<String> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!("Computer Use is only supported on macOS"));
    }

    let helper = helper::resolve_helper_path(Some(app));
    let helper_bin = executable_path(&helper)?;
    let output = run_helper(&helper_bin, &["doctor"], Duration::from_secs(10)).await?;
    let text = output.combined_text();
    if output.status.success() {
        Ok(summary_or_default(text))
    } else {
        Err(anyhow!(
            "doctor exited with status {}: {}",
            output.status,
            summary_or_default(text)
        ))
    }
}

pub(crate) fn classify_doctor(_text: &str) -> ComputerUseStatusKind {
    let text = _text.to_lowercase();
    if text.contains("missing") || text.contains("denied") {
        return ComputerUseStatusKind::PermissionMissing;
    }

    for line in text.lines() {
        let normalized = line.replace(['_', '-'], " ");
        if normalized.contains("false")
            && (normalized.contains("accessibility")
                || normalized.contains("screen recording")
                || normalized.contains("screen capture"))
        {
            return ComputerUseStatusKind::PermissionMissing;
        }
    }

    ComputerUseStatusKind::Ready
}

fn executable_path(helper: &HelperInfo) -> Result<PathBuf> {
    if helper.state != HelperState::Found {
        return Err(anyhow!(
            "{}",
            helper
                .error
                .clone()
                .unwrap_or_else(|| "Computer Use helper is not executable".to_string())
        ));
    }

    helper
        .path
        .clone()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("Computer Use helper path is unavailable"))
}

async fn run_helper(
    helper: &PathBuf,
    args: &[&str],
    timeout: Duration,
) -> Result<HelperCommandOutput> {
    let mut command = Command::new(helper);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = command.spawn().map_err(|err| {
        anyhow!(
            "failed to start Computer Use helper {}: {err}",
            helper.display()
        )
    })?;

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("Computer Use helper timed out after {}s", timeout.as_secs()))?
        .map_err(|err| anyhow!("Computer Use helper failed: {err}"))?;

    Ok(HelperCommandOutput {
        status: output.status,
        stdout: clean_output_bytes(&output.stdout),
        stderr: clean_output_bytes(&output.stderr),
    })
}

fn clean_output_bytes(bytes: &[u8]) -> String {
    clean_line_endings(&String::from_utf8_lossy(bytes))
}

fn clean_line_endings(text: &str) -> String {
    text.trim_end_matches(['\r', '\n']).to_string()
}

fn combine_text(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn optional_text(text: String) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn summary_or_default(text: String) -> String {
    if text.is_empty() {
        "Computer Use doctor completed with no output.".to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_doctor_detects_missing_permission_words() {
        let text = "accessibility: true\nscreen recording: denied\n";

        assert!(matches!(
            classify_doctor(text),
            ComputerUseStatusKind::PermissionMissing
        ));
    }

    #[test]
    fn classify_doctor_detects_false_permission_flags() {
        let text = "accessibility: false\nscreen recording: true\n";

        assert!(matches!(
            classify_doctor(text),
            ComputerUseStatusKind::PermissionMissing
        ));
    }

    #[test]
    fn classify_doctor_accepts_ready_text() {
        let text = "accessibility: true\nscreen recording: true\nall checks passed\n";

        assert!(matches!(
            classify_doctor(text),
            ComputerUseStatusKind::Ready
        ));
    }
}
