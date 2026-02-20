use thiserror::Error;

#[derive(Debug, Error)]
#[error("Repository operation failed")]
pub struct RepositoryError;
