use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::{
    entities::Facility, errors::RepositoryError, repositories::FacilityRepository,
};

#[derive(Default)]
pub struct InMemoryFacilityRepository {
    facilities: RwLock<Vec<Facility>>,
}

impl InMemoryFacilityRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl FacilityRepository for InMemoryFacilityRepository {
    async fn replace_all(&self, facilities: Vec<Facility>) -> Result<(), RepositoryError> {
        let mut write_guard = self.facilities.write().await;
        *write_guard = facilities;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Facility>, RepositoryError> {
        Ok(self.facilities.read().await.clone())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Facility>, RepositoryError> {
        let item = self
            .facilities
            .read()
            .await
            .iter()
            .find(|facility| facility.id == id)
            .cloned();

        Ok(item)
    }
}
