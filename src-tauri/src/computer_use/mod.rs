pub mod diagnostics;
pub mod helper;
pub mod inject;
pub mod settings;

pub mod commands {
    use super::diagnostics::{self, ComputerUseStatus};
    use super::settings;
    use crate::store::Db;
    use tauri::{AppHandle, State};

    type R<T> = Result<T, String>;

    #[tauri::command]
    pub async fn computer_use_get_status(
        app: AppHandle,
        db: State<'_, Db>,
    ) -> R<ComputerUseStatus> {
        diagnostics::status(&app, &db)
            .await
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn computer_use_set_enabled(db: State<'_, Db>, enabled: bool) -> R<()> {
        settings::set_enabled(&db, enabled)
            .await
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn computer_use_run_doctor(app: AppHandle) -> R<String> {
        diagnostics::run_doctor_text(&app)
            .await
            .map_err(|e| e.to_string())
    }
}
