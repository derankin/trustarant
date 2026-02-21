use std::sync::Arc;

use crate::domain::{
    entities::{FacilityVoteSummary, VoteValue},
    repositories::FacilityRepository,
};

#[derive(Clone)]
pub struct VoteService {
    repository: Arc<dyn FacilityRepository>,
}

impl VoteService {
    pub fn new(repository: Arc<dyn FacilityRepository>) -> Self {
        Self { repository }
    }

    pub async fn record_vote(
        &self,
        facility_id: &str,
        voter_key: &str,
        vote: VoteValue,
    ) -> Result<FacilityVoteSummary, crate::domain::errors::RepositoryError> {
        self.repository
            .upsert_facility_vote(facility_id, voter_key, vote)
            .await
    }
}
