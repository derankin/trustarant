use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    application::dto::{
        FacilityDetail, FacilitySearchQuery, FacilitySearchResult, FacilitySummary, ScoreSliceCounts,
    },
    domain::{
        entities::{Facility, FacilityVoteSummary},
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
        let mut facilities = self.repository.list().await?;
        let has_search_term = query
            .q
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        if let Some(term) = query.q.as_ref().map(|value| normalize_for_search(value)) {
            if !term.is_empty() {
                let search_tokens = term.split_whitespace().collect::<Vec<_>>();
                facilities.retain(|facility| {
                    let candidate = normalize_for_search(&format!(
                        "{} {} {} {}",
                        facility.name, facility.address, facility.city, facility.postal_code
                    ));
                    let candidate_tokens = candidate.split_whitespace().collect::<Vec<_>>();

                    // Match each token independently so re-ordered terms and name variants
                    // (e.g. "Mastro's" vs "mastros" and singular/plural drift) resolve
                    // predictably.
                    search_tokens.iter().all(|query_token| {
                        candidate_tokens
                            .iter()
                            .any(|candidate_token| tokens_match(candidate_token, query_token))
                    })
                });
            }
        }

        if let Some(jurisdiction) = query
            .jurisdiction
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
        {
            if !jurisdiction.is_empty() && jurisdiction != "all" {
                facilities.retain(|facility| {
                    facility
                        .jurisdiction
                        .label()
                        .eq_ignore_ascii_case(jurisdiction.as_str())
                });
            }
        }

        // Search terms (name/address/ZIP) should not be constrained by the default
        // "near downtown LA" radius used for discovery mode.
        if !has_search_term {
            if let (Some(latitude), Some(longitude), Some(radius_miles)) =
                (query.latitude, query.longitude, query.radius_miles)
            {
                facilities.retain(|facility| {
                    haversine_miles(latitude, longitude, facility.latitude, facility.longitude)
                        <= radius_miles.max(0.1)
                });
            }
        }

        if query.recent_only.unwrap_or(false) {
            let now = Utc::now();
            facilities.retain(|facility| {
                latest_inspection_at(facility)
                    .map(|inspected_at| now.signed_duration_since(inspected_at).num_days() <= 90)
                    .unwrap_or(false)
            });
        }

        let slice_counts = ScoreSliceCounts {
            all: facilities.len(),
            elite: facilities
                .iter()
                .filter(|facility| facility.trust_score >= 90)
                .count(),
            solid: facilities
                .iter()
                .filter(|facility| facility.trust_score >= 80 && facility.trust_score < 90)
                .count(),
            watch: facilities
                .iter()
                .filter(|facility| facility.trust_score < 80)
                .count(),
        };

        if let Some(score_slice) = query
            .score_slice
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
        {
            match score_slice.as_str() {
                "elite" => facilities.retain(|facility| facility.trust_score >= 90),
                "solid" => {
                    facilities
                        .retain(|facility| facility.trust_score >= 80 && facility.trust_score < 90)
                }
                "watch" => facilities.retain(|facility| facility.trust_score < 80),
                _ => {}
            }
        }

        match query
            .sort
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("recent_desc") => {
                facilities.sort_by(|left, right| {
                    latest_inspection_at(right)
                        .cmp(&latest_inspection_at(left))
                        .then(right.trust_score.cmp(&left.trust_score))
                });
            }
            Some("name_asc") => {
                facilities.sort_by(|left, right| left.name.cmp(&right.name));
            }
            _ => {
                facilities.sort_by(|left, right| {
                    right
                        .trust_score
                        .cmp(&left.trust_score)
                        .then(right.updated_at.cmp(&left.updated_at))
                });
            }
        }

        let page_size = query
            .page_size
            .or(query.limit)
            .unwrap_or(50)
            .clamp(1, 200);
        let page = query.page.unwrap_or(1).max(1);
        let total_count = facilities.len();
        let offset = (page - 1).saturating_mul(page_size);
        let page_facilities = facilities
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect::<Vec<_>>();
        let page_ids = page_facilities
            .iter()
            .map(|facility| facility.id.clone())
            .collect::<Vec<_>>();
        let vote_summaries = self.repository.get_facility_vote_summaries(&page_ids).await?;
        let data = page_facilities
            .into_iter()
            .map(|facility| {
                let summary = vote_summaries.get(&facility.id).cloned().unwrap_or_default();
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
        let vote_summary = vote_summaries.get(&facility.id).cloned().unwrap_or_default();

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
                let vote_summary = vote_summaries.get(&facility.id).cloned().unwrap_or_default();
                (facility, vote_summary)
            })
            .collect::<Vec<_>>();

        // Top picks represent active community preference. Exclude entries with no likes.
        ranked.retain(|(_, votes)| votes.likes > 0);
        if ranked.is_empty() {
            return Ok(Vec::new());
        }

        ranked.sort_by(|(left_facility, left_votes), (right_facility, right_votes)| {
            right_votes
                .likes
                .cmp(&left_votes.likes)
                .then(right_votes.score().cmp(&left_votes.score()))
                .then(right_facility.trust_score.cmp(&left_facility.trust_score))
                .then(right_facility.updated_at.cmp(&left_facility.updated_at))
        });

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

fn normalize_for_search(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut last_was_space = false;

    for ch in value.chars() {
        if ch == '\'' || ch == '’' || ch == '`' {
            // Drop apostrophes to normalize "Mastro's" -> "mastros".
            continue;
        }

        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.trim().to_owned()
}

fn singularize_token(token: &str) -> &str {
    if token.len() > 4 && token.ends_with('s') {
        &token[..token.len() - 1]
    } else {
        token
    }
}

fn tokens_match(candidate_token: &str, query_token: &str) -> bool {
    if candidate_token == query_token {
        return true;
    }

    let candidate_singular = singularize_token(candidate_token);
    let query_singular = singularize_token(query_token);
    if candidate_singular == query_singular {
        return true;
    }

    // Allow partial matches for meaningful tokens so "mastros" and "mastro"
    // still match in either direction, while avoiding noise on tiny terms.
    let min_partial_len = 4;
    (candidate_token.len() >= min_partial_len && query_token.len() >= min_partial_len)
        && (candidate_token.contains(query_token) || query_token.contains(candidate_token))
}

fn haversine_miles(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius_miles = 3_958.8_f64;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();

    let a =
        (dlat / 2.0).sin().powi(2) + lat1_rad.cos() * lat2_rad.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    radius_miles * c
}

#[cfg(test)]
mod tests {
    use super::{normalize_for_search, tokens_match};

    #[test]
    fn normalizes_apostrophes_and_punctuation() {
        assert_eq!(normalize_for_search("Mastro's Steakhouse"), "mastros steakhouse");
        assert_eq!(normalize_for_search("Mastro’s Steakhouse"), "mastros steakhouse");
    }

    #[test]
    fn supports_out_of_order_multi_token_matching() {
        let candidate = normalize_for_search("Mastro's Steakhouse Beverly Hills");
        let query = normalize_for_search("hills mastros");
        let candidate_tokens = candidate.split_whitespace().collect::<Vec<_>>();

        assert!(query.split_whitespace().all(|query_token| candidate_tokens
            .iter()
            .any(|candidate_token| tokens_match(candidate_token, query_token))));
    }

    #[test]
    fn supports_singular_plural_name_variants() {
        assert!(tokens_match("mastros", "mastro"));
        assert!(tokens_match("mastro", "mastros"));
    }
}

