use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0}")]
    Message(String),
}

impl RepositoryError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
