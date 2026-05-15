use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid skill: {0}")]
    InvalidSkill(String),

    #[error("Conflicting skill: {0}")]
    Conflict(String),

    #[error("Installation failed: {0}")]
    InstallFailed(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Zip error: {0}")]
    Zip(String),

    #[error("Link error: {0}")]
    Link(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Cancelled")]
    Cancelled,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::Zip(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Config(format!("Database error: {}", err))
    }
}