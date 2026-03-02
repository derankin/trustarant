use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    application::dto::{FacilitySearchQuery, ScoreSliceCounts},
    domain::{
        entities::{
            AutocompleteSuggestion, Facility, FacilityVoteSummary, SystemIngestionStatus, VoteValue,
        },
        errors::RepositoryError,
        repositories::FacilityRepository,
    },
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

    async fn search_facilities(
        &self,
        query: &FacilitySearchQuery,
    ) -> Result<(Vec<Facility>, usize, ScoreSliceCounts), RepositoryError> {
        let mut facilities = self.facilities.read().await.clone();

        // Basic text filtering
        if let Some(term) = query.q.as_ref().map(|v| v.trim().to_ascii_lowercase()) {
            if !term.is_empty() {
                facilities.retain(|f| {
                    f.name.to_ascii_lowercase().contains(&term)
                        || f.address.to_ascii_lowercase().contains(&term)
                        || f.city.to_ascii_lowercase().contains(&term)
                        || f.postal_code.contains(&term)
                });
            }
        }

        // Jurisdiction filter
        if let Some(jurisdiction) = query
            .jurisdiction
            .as_ref()
            .map(|v| v.trim().to_ascii_lowercase())
        {
            if !jurisdiction.is_empty() && jurisdiction != "all" {
                facilities.retain(|f| {
                    f.jurisdiction.code() == jurisdiction
                        || f.jurisdiction.label().eq_ignore_ascii_case(&jurisdiction)
                });
            }
        }

        let slice_counts = ScoreSliceCounts {
            all: facilities.len(),
            elite: facilities.iter().filter(|f| f.trust_score >= 90).count(),
            solid: facilities
                .iter()
                .filter(|f| f.trust_score >= 80 && f.trust_score < 90)
                .count(),
            watch: facilities.iter().filter(|f| f.trust_score < 80).count(),
        };

        if let Some(slice) = query
            .score_slice
            .as_ref()
            .map(|v| v.trim().to_ascii_lowercase())
        {
            match slice.as_str() {
                "elite" => facilities.retain(|f| f.trust_score >= 90),
                "solid" => facilities.retain(|f| f.trust_score >= 80 && f.trust_score < 90),
                "watch" => facilities.retain(|f| f.trust_score < 80),
                _ => {}
            }
        }

        facilities.sort_by(|a, b| b.trust_score.cmp(&a.trust_score));

        // total_count reflects post-slice-filter count (matches number of paginated results)
        let total_count = facilities.len();
        let page_size = query.page_size.or(query.limit).unwrap_or(50).clamp(1, 200);
        let page = query.page.unwrap_or(1).max(1);
        let offset = (page - 1).saturating_mul(page_size);
        let page_facilities = facilities.into_iter().skip(offset).take(page_size).collect();

        Ok((page_facilities, total_count, slice_counts))
    }

    async fn autocomplete(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<AutocompleteSuggestion>, RepositoryError> {
        let lower = prefix.to_ascii_lowercase();
        let capped = limit.clamp(1, 20);
        let facilities = self.facilities.read().await;

        let suggestions = facilities
            .iter()
            .filter(|f| {
                f.name.to_ascii_lowercase().contains(&lower)
                    || f.city.to_ascii_lowercase().contains(&lower)
                    || f.postal_code.starts_with(prefix)
            })
            .take(capped)
            .map(|f| AutocompleteSuggestion {
                id: f.id.clone(),
                name: f.name.clone(),
                city: f.city.clone(),
                postal_code: f.postal_code.clone(),
                trust_score: f.trust_score,
            })
            .collect();

        Ok(suggestions)
    }
}
