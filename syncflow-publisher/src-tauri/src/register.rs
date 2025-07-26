use std::f32::consts::E;
use std::path::PathBuf;

use crate::errors::SyncFlowPublisherError;
use crate::{models, utils as app_utils};
use serde::{de, Deserialize, Serialize};
use syncflow_client::{ProjectClient, ProjectClientError};
use syncflow_shared::device_models::{DeviceRegisterRequest, DeviceResponse};
use syncflow_shared::user_models::ProjectInfo;

const DEFAULT_COMMENTS: &str = "Registered via SyncFlow Publisher";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterCredentials {
    pub syncflow_project_id: String,
    pub syncflow_api_key: String,
    pub syncflow_server_url: String,
    pub syncflow_api_secret: String,
    pub device_name: Option<String>,
    pub device_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationResponse {
    pub device_id: String,
    pub device_name: String,
    pub device_group: String,
    pub project_name: String,
    pub project_id: String,
    pub project_comments: String,
    pub lk_server_url: String,
    pub s3_bucket_name: String,
    pub s3_endpoint: String,
}

impl RegistrationResponse {
    pub fn compose(device_response: &DeviceResponse, project_details: &ProjectInfo) -> Self {
        Self {
            device_id: device_response.id.clone(),
            device_name: device_response.name.clone(),
            device_group: device_response.group.clone(),
            project_name: project_details.name.clone(),
            project_id: project_details.id.clone(),
            project_comments: device_response.comments.clone().unwrap_or_default(),
            lk_server_url: project_details.livekit_server_url.clone(),
            s3_bucket_name: project_details.bucket_name.clone(),
            s3_endpoint: project_details.endpoint.clone(),
        }
    }
}

#[tauri::command(async)]
pub async fn register_to_syncflow(
    credentials: RegisterCredentials,
    app_state: tauri::State<'_, models::AppState>,
) -> Result<RegistrationResponse, SyncFlowPublisherError> {
    let project_client = ProjectClient::new(
        &credentials.syncflow_server_url,
        &credentials.syncflow_project_id,
        &credentials.syncflow_api_key,
        &credentials.syncflow_api_secret,
    );

    let device_request = DeviceRegisterRequest {
        name: credentials
            .device_name
            .clone()
            .unwrap_or(match app_utils::get_ip_address() {
                Some(ip) => format!("{} ({})", app_utils::host_name(), ip),
                None => app_utils::host_name(),
            }),
        group: credentials.device_group.clone(),
        comments: Some(DEFAULT_COMMENTS.to_string()),
    };

    let device_response = project_client.register_device(&device_request).await?;
    let project_details = project_client.get_project_details().await?;

    let mut client: std::sync::MutexGuard<'_, Option<ProjectClient>> =
        app_state.client.lock().unwrap();

    *client = Some(project_client);

    let credentials_file = app_state.app_dir.join("credentials.json");
    std::fs::write(credentials_file, serde_json::to_string(&credentials)?)?;
    let registration_file = app_state.app_dir.join("registration.json");
    std::fs::write(registration_file, serde_json::to_string(&device_response)?)?;
    let registration_response = RegistrationResponse::compose(&device_response, &project_details);

    Ok(registration_response)
}

pub async fn intialize_client(app_dir: &PathBuf) -> Option<ProjectClient> {
    let credentials_path = app_dir.join("credentials.json");
    if credentials_path.exists() {
        let credentials_str = std::fs::read_to_string(&credentials_path).ok()?;
        let credentials: RegisterCredentials = serde_json::from_str(&credentials_str).ok()?;
        Some(ProjectClient::new(
            &credentials.syncflow_server_url,
            &credentials.syncflow_project_id,
            &credentials.syncflow_api_key,
            &credentials.syncflow_api_secret,
        ))
    } else {
        None
    }
}

pub async fn register_if_needed(
    client: &ProjectClient,
    app_dir: &PathBuf,
) -> Option<RegistrationResponse> {
    let registration_file = app_dir.join("registration.json");
    if registration_file.exists() {
        let registration_str = std::fs::read_to_string(&registration_file).ok()?;
        let mut registration: DeviceResponse = serde_json::from_str(&registration_str).ok()?;
        let project_details = client.get_project_details().await.ok()?;

        let registered_device_res = client.get_device(&registration.id).await;
        if let Err(ProjectClientError::ReqwestError(_)) = registered_device_res {
            // Device not found, proceed to register
            // Attempt to register the device again
            if let Ok(new_device) = client
                .register_device(&DeviceRegisterRequest {
                    name: registration.name.clone(),
                    group: registration.group.clone(),
                    comments: Some(DEFAULT_COMMENTS.to_string()),
                })
                .await
            {
                registration = new_device.into();
                let registration_file = app_dir.join("registration.json");
                let _ = std::fs::write(
                    registration_file,
                    serde_json::to_string(&registration).ok()?,
                );
            }
        }
        Some(RegistrationResponse::compose(
            &registration,
            &project_details,
        ))
    } else {
        None
    }
}

#[tauri::command(async)]
pub async fn delete_registration(
    app_state: tauri::State<'_, models::AppState>,
) -> Result<(), SyncFlowPublisherError> {
    let client = {
        let guard = app_state.client.lock().unwrap();
        guard.clone().ok_or_else(|| {
            SyncFlowPublisherError::NotIntialized("Client is not initialized".into())
        })?
    };

    let registration_file = app_state.app_dir.join("registration.json");
    if registration_file.exists() {
        let registration_str = std::fs::read_to_string(&registration_file)?;
        let registration: DeviceResponse = serde_json::from_str(&registration_str)?;
        let _ = client.delete_device(&registration.id).await?;
    }

    let app_dir = &app_state.app_dir;
    let credentials_file = app_dir.join("credentials.json");
    if credentials_file.exists() {
        std::fs::remove_file(credentials_file)?;
    }
    let registration_file = app_dir.join("registration.json");
    if registration_file.exists() {
        std::fs::remove_file(registration_file)?;
    }

    Ok(())
}

pub async fn deregister_from_syncflow(
    app_state: &tauri::State<'_, models::AppState>,
) -> Result<(), SyncFlowPublisherError> {
    let client = app_state.client.lock().unwrap();
    let client = client
        .as_ref()
        .ok_or(SyncFlowPublisherError::NotIntialized(
            "Client is not initialized".to_string(),
        ))?;

    let registration_file = app_state.app_dir.join("registration.json");
    if registration_file.exists() {
        let registration_str = std::fs::read_to_string(&registration_file)?;
        let registration: DeviceResponse = serde_json::from_str(&registration_str)?;

        let _ = client.delete_device(&registration.id).await?;
    }

    Ok(())
}
