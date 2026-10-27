//! Wizard "Project baru": daftar template dan pembuatan project dengan log langsung.

use devlock_core::launcher;
use devlock_core::scaffold::{self, Created, Options, Template};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::commands::{blocking, context, err, CmdResult};

#[derive(Serialize)]
pub struct TemplateInfo {
    id: &'static str,
    label: &'static str,
    uses_php: bool,
    uses_node: bool,
    needs_database: bool,
}

#[tauri::command]
pub fn list_templates() -> Vec<TemplateInfo> {
    Template::ALL
        .iter()
        .map(|t| TemplateInfo {
            id: t.id(),
            label: t.label(),
            uses_php: t.uses_php(),
            uses_node: t.uses_node(),
            needs_database: t.needs_database(),
        })
        .collect()
}

/// Buat project; setiap baris output dikirim sebagai event `scaffold-log`.
#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    template: String,
    name: String,
    php: Option<String>,
    node: Option<String>,
    database: Option<String>,
) -> CmdResult<Created> {
    blocking(move || {
        let (paths, config) = context()?;
        let template: Template = template.parse().map_err(err)?;
        let opts = Options {
            name,
            php: php.filter(|v| !v.is_empty()),
            node: node.filter(|v| !v.is_empty()),
            database: database.filter(|v| !v.is_empty()),
        };
        let created = scaffold::create(&paths, &config, template, &opts, &mut |line| {
            let _ = app.emit("scaffold-log", line.to_string());
        })
        .map_err(err)?;
        if let Err(e) = launcher::sync_hosts(&paths, &config, false) {
            let _ = app.emit(
                "scaffold-log",
                format!("File hosts tidak diperbarui: {e:#}"),
            );
        }
        Ok(created)
    })
    .await
}
