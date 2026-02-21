use async_trait::async_trait;

use crate::domain::{
    entities::{Facility, SystemIngestionStatus},
    errors::RepositoryError,
};

#[async_trait]
pub trait FacilityRepository: Send + Sync {
    async fn replace_all(&self, facilities: Vec<Facility>) -> Result<(), RepositoryError>;
    async fn list(&self) -> Result<Vec<Facility>, RepositoryError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Facility>, RepositoryError>;
    async fn set_system_ingestion_status(
        &self,
        status: SystemIngestionStatus,
    ) -> Result<(), RepositoryError>;
    async fn get_system_ingestion_status(
        &self,
    ) -> Result<Option<SystemIngestionStatus>, RepositoryError>;
}
