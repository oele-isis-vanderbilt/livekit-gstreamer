// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod devices;
mod errors;
mod models;
mod register;
mod utils;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use devices::get_devices;
use register::{delete_registration, register_to_syncflow};
use tauri::Manager;

use crate::{errors::SyncFlowPublisherError, register::RegistrationResponse};

fn create_app_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    let app_dir = home_dir.join(".syncflow-publisher");
    std::fs::create_dir_all(&app_dir)?;
    Ok(app_dir)
}

#[tauri::command]
fn get_registration(
    app_state: tauri::State<'_, models::AppState>,
) -> Result<RegistrationResponse, SyncFlowPublisherError> {
    if let Some(registration) = &app_state.registration {
        Ok(registration.clone())
    } else {
        Err(SyncFlowPublisherError::NotIntialized(
            "Registration not found".to_string(),
        ))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            livekit_gstreamer::initialize_gstreamer();
            let app_dir = create_app_dir().expect("Failed to create app directory");

            tauri::async_runtime::block_on(async {
                let client = register::intialize_client(&app_dir).await;
                let registration = if let Some(c) = client.as_ref() {
                    register::register_if_needed(c, &app_dir).await
                } else {
                    None
                };
                let app_state = models::AppState {
                    client: Arc::new(Mutex::new(client)),
                    app_dir,
                    registration: registration.clone(),
                };
                app.manage(app_state);
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_devices,
            get_registration,
            register_to_syncflow,
            delete_registration,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                tauri::async_runtime::block_on(async {
                    let app_state = app_handle.state::<models::AppState>();
                    let _ = register::deregister_from_syncflow(&app_state).await;
                });
            }
            _ => {}
        })
}
