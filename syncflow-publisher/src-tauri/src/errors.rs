use crate::models;

#[derive(Debug, thiserror::Error)]
pub enum SyncFlowPublisherError {
    #[error("{0}")]
    ProjectClientError(#[from] syncflow_client::ProjectClientError),

    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Failed to read file: {0}")]
    NotIntialized(String),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Io(String),
    JSON(String),
    ProjectClient(String),
}

impl serde::Serialize for SyncFlowPublisherError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let error_message = self.to_string();
        let error_kind = match self {
            Self::IoError(_) => ErrorKind::Io(error_message),
            Self::JsonError(_) => ErrorKind::JSON(error_message), // Treat JSON errors as IO for serialization
            Self::ProjectClientError(_) => ErrorKind::ProjectClient(error_message),
            Self::NotIntialized(_) => ErrorKind::Io(error_message), // Treat NotIntialized as IO for serialization
        };
        error_kind.serialize(serializer)
    }
}
