use std::sync::Arc;

use crate::{
    application::dto::{FacilityDetail, FacilitySearchQuery, FacilitySummary},
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
    ) -> Result<Vec<FacilitySummary>, crate::domain::errors::RepositoryError> {
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

        facilities.sort_by(|left, right| {
            right
                .trust_score
                .cmp(&left.trust_score)
                .then(right.updated_at.cmp(&left.updated_at))
        });

        let limit = query.limit.unwrap_or(50).clamp(1, 2_000);

        Ok(facilities
            .into_iter()
            .take(limit)
            .map(to_summary)
            .collect::<Vec<_>>())
    }

    pub async fn get(
        &self,
        id: &str,
    ) -> Result<Option<FacilityDetail>, crate::domain::errors::RepositoryError> {
        let facility = self.repository.get_by_id(id).await?;

        Ok(facility.map(|facility| FacilityDetail {
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
            inspections_count: facility.inspections.len(),
            latest_inspection_at: facility
                .inspections
                .first()
                .map(|inspection| inspection.inspected_at),
        }))
    }
}

fn to_summary(facility: Facility) -> FacilitySummary {
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
        latest_inspection_at: facility
            .inspections
            .first()
            .map(|inspection| inspection.inspected_at),
    }
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
