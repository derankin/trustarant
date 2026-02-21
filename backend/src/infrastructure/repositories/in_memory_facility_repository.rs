use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::{
    entities::{Facility, FacilityVoteSummary, SystemIngestionStatus, VoteValue},
    errors::RepositoryError,
    repositories::FacilityRepository,
};

#[derive(Default)]
pub struct InMemoryFacilityRepository {
    facilities: RwLock<Vec<Facility>>,
    ingestion_status: RwLock<Option<SystemIngestionStatus>>,
    votes: RwLock<HashMap<(String, String), VoteValue>>,
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

    async fn set_system_ingestion_status(
        &self,
        status: SystemIngestionStatus,
    ) -> Result<(), RepositoryError> {
        let mut write_guard = self.ingestion_status.write().await;
        *write_guard = Some(status);
        Ok(())
    }

    async fn get_system_ingestion_status(
        &self,
    ) -> Result<Option<SystemIngestionStatus>, RepositoryError> {
        Ok(self.ingestion_status.read().await.clone())
    }

    async fn upsert_facility_vote(
        &self,
        facility_id: &str,
        voter_key: &str,
        vote: VoteValue,
    ) -> Result<FacilityVoteSummary, RepositoryError> {
        {
            let mut write_guard = self.votes.write().await;
            write_guard.insert((facility_id.to_owned(), voter_key.to_owned()), vote);
        }

        let all_votes = self.votes.read().await;
        let mut likes = 0_u64;
        let mut dislikes = 0_u64;
        for ((current_facility_id, _), current_vote) in all_votes.iter() {
            if current_facility_id != facility_id {
                continue;
            }
            match current_vote {
                VoteValue::Like => likes += 1,
                VoteValue::Dislike => dislikes += 1,
            }
        }

        Ok(FacilityVoteSummary { likes, dislikes })
    }

    async fn get_facility_vote_summaries(
        &self,
        facility_ids: &[String],
    ) -> Result<HashMap<String, FacilityVoteSummary>, RepositoryError> {
        if facility_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let id_set = facility_ids.iter().cloned().collect::<std::collections::HashSet<_>>();
        let all_votes = self.votes.read().await;
        let mut summaries: HashMap<String, FacilityVoteSummary> = HashMap::new();

        for ((facility_id, _), vote) in all_votes.iter() {
            if !id_set.contains(facility_id) {
                continue;
            }

            let summary = summaries.entry(facility_id.clone()).or_default();
            match vote {
                VoteValue::Like => summary.likes += 1,
                VoteValue::Dislike => summary.dislikes += 1,
            }
        }

        Ok(summaries)
    }
}
