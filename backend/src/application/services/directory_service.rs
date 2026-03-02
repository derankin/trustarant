use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    application::dto::{FacilityDetail, FacilitySearchQuery, FacilitySearchResult, FacilitySummary},
    domain::{
        entities::{AutocompleteSuggestion, Facility, FacilityVoteSummary},
        repositories::FacilityRepository,
    },
};

#[derive(Clone)]
pub struct DirectoryService {
    repository: Arc<dyn FacilityRepository>,
}

impl DirectoryService {
    pub fn new(repository: Arc<dyn FacilityRepository>) -> Self {
        Self { repository }
    }

    pub async fn search(
        &self,
        query: FacilitySearchQuery,
    ) -> Result<FacilitySearchResult, crate::domain::errors::RepositoryError> {
        let page_size = query.page_size.or(query.limit).unwrap_or(50).clamp(1, 200);
        let page = query.page.unwrap_or(1).max(1);

        let (facilities, total_count, slice_counts) =
            self.repository.search_facilities(&query).await?;

        let page_ids = facilities
            .iter()
            .map(|facility| facility.id.clone())
            .collect::<Vec<_>>();
        let vote_summaries = self
            .repository
            .get_facility_vote_summaries(&page_ids)
            .await?;
        let data = facilities
            .into_iter()
            .map(|facility| {
                let summary = vote_summaries
                    .get(&facility.id)
                    .cloned()
                    .unwrap_or_default();
                to_summary(facility, summary)
            })
            .collect::<Vec<_>>();

        Ok(FacilitySearchResult {
            count: data.len(),
            total_count,
            data,
            page,
            page_size,
            slice_counts,
        })
    }

    pub async fn autocomplete(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<AutocompleteSuggestion>, crate::domain::errors::RepositoryError> {
        self.repository.autocomplete(prefix, limit).await
    }

    pub async fn get(
        &self,
        id: &str,
    ) -> Result<Option<FacilityDetail>, crate::domain::errors::RepositoryError> {
        let facility = self.repository.get_by_id(id).await?;
        let Some(facility) = facility else {
            return Ok(None);
        };

        let latest_inspection_at = latest_inspection_at(&facility);
        let inspections_count = facility.inspections.len();
        let vote_summaries = self
            .repository
            .get_facility_vote_summaries(&[facility.id.clone()])
            .await?;
        let vote_summary = vote_summaries
            .get(&facility.id)
            .cloned()
            .unwrap_or_default();

        Ok(Some(FacilityDetail {
            id: facility.id,
            source_id: facility.source_id,
            name: facility.name,
            address: facility.address,
            city: facility.city,
            state: facility.state,
            postal_code: facility.postal_code,
            latitude: facility.latitude,
            longitude: facility.longitude,
            jurisdiction: facility.jurisdiction.label().to_string(),
            trust_score: facility.trust_score,
            inspections_count,
            latest_inspection_at,
            likes: vote_summary.likes,
            dislikes: vote_summary.dislikes,
            vote_score: vote_summary.score(),
        }))
    }

    pub async fn top_picks(
        &self,
        limit: usize,
    ) -> Result<Vec<FacilitySummary>, crate::domain::errors::RepositoryError> {
        let facilities = self.repository.list().await?;
        if facilities.is_empty() {
            return Ok(Vec::new());
        }

        let capped_limit = limit.clamp(1, 50);
        let facility_ids = facilities
            .iter()
            .map(|facility| facility.id.clone())
            .collect::<Vec<_>>();
        let vote_summaries = self
            .repository
            .get_facility_vote_summaries(&facility_ids)
            .await?;

        let mut ranked = facilities
            .into_iter()
            .map(|facility| {
                let vote_summary = vote_summaries
                    .get(&facility.id)
                    .cloned()
                    .unwrap_or_default();
                (facility, vote_summary)
            })
            .collect::<Vec<_>>();

        // Top picks represent active community preference. Exclude entries with no likes.
        ranked.retain(|(_, votes)| votes.likes > 0);
        if ranked.is_empty() {
            return Ok(Vec::new());
        }

        ranked.sort_by(
            |(left_facility, left_votes), (right_facility, right_votes)| {
                right_votes
                    .likes
                    .cmp(&left_votes.likes)
                    .then(right_votes.score().cmp(&left_votes.score()))
                    .then(right_facility.trust_score.cmp(&left_facility.trust_score))
                    .then(right_facility.updated_at.cmp(&left_facility.updated_at))
            },
        );

        Ok(ranked
            .into_iter()
            .take(capped_limit)
            .map(|(facility, votes)| to_summary(facility, votes))
            .collect::<Vec<_>>())
    }
}

fn to_summary(facility: Facility, vote_summary: FacilityVoteSummary) -> FacilitySummary {
    let latest_inspection_at = latest_inspection_at(&facility);

    FacilitySummary {
        id: facility.id,
        name: facility.name,
        address: facility.address,
        city: facility.city,
        state: facility.state,
        postal_code: facility.postal_code,
        latitude: facility.latitude,
        longitude: facility.longitude,
        jurisdiction: facility.jurisdiction.label().to_string(),
        trust_score: facility.trust_score,
        latest_inspection_at,
        likes: vote_summary.likes,
        dislikes: vote_summary.dislikes,
        vote_score: vote_summary.score(),
    }
}

fn latest_inspection_at(facility: &Facility) -> Option<DateTime<Utc>> {
    facility
        .inspections
        .iter()
        .map(|inspection| inspection.inspected_at)
        .max()
}
