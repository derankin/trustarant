use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    application::dto::{
        FacilityDetail, FacilitySearchQuery, FacilitySearchResult, FacilitySummary, ScoreSliceCounts,
    },
    domain::{entities::Facility, repositories::FacilityRepository},
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

        if let Some(term) = query
            .q
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
        {
            if !term.is_empty() {
                facilities.retain(|facility| {
                    let candidate = format!(
                        "{} {} {} {}",
                        facility.name, facility.address, facility.city, facility.postal_code
                    )
                    .to_ascii_lowercase();
                    candidate.contains(&term)
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
        let data = facilities
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(to_summary)
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

        Ok(facility.map(|facility| {
            let latest_inspection_at = latest_inspection_at(&facility);
            let inspections_count = facility.inspections.len();

            FacilityDetail {
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
            }
        }))
    }
}

fn to_summary(facility: Facility) -> FacilitySummary {
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
    }
}

fn latest_inspection_at(facility: &Facility) -> Option<DateTime<Utc>> {
    facility
        .inspections
        .iter()
        .map(|inspection| inspection.inspected_at)
        .max()
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
