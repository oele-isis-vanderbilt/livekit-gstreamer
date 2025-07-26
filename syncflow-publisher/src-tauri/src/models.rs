use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use syncflow_client::ProjectClient;

use crate::register::RegistrationResponse;

pub struct AppState {
    pub client: Arc<Mutex<Option<ProjectClient>>>,
    pub app_dir: PathBuf,
    pub registration: Option<RegistrationResponse>,
}
