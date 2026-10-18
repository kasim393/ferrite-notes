use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Settings error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Note not found")]
    NotFound,
    #[error("That line is not a task")]
    NotATask,
    #[error(
        "This notes database was created by a newer version of Ferrite Notes \
         (schema {0}). Please update the app."
    )]
    NewerSchema(i64),
    #[error("{0}")]
    Other(String),
}

// Commands send errors to the UI as a friendly string.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
