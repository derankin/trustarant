use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

pub struct SanDiegoConnector;

#[async_trait]
impl HealthDataConnector for SanDiegoConnector {
    fn source_name(&self) -> &'static str {
        "san_diego_socrata"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        Ok(vec![SourceFacilityInput {
            source_id: "SD-5562".to_owned(),
            name: "Harbor Greens Kitchen".to_owned(),
            address: "442 Harbor Dr".to_owned(),
            city: "San Diego".to_owned(),
            state: "CA".to_owned(),
            postal_code: "92101".to_owned(),
            latitude: 32.7112,
            longitude: -117.1681,
            jurisdiction: Jurisdiction::SanDiegoCounty,
            inspected_at: Utc::now() - Duration::days(2),
            raw_score: Some(89.0),
            letter_grade: Some("B".to_owned()),
            placard_status: None,
            violations: vec![Violation {
                code: "MRF-12".to_owned(),
                description: "Improper cold holding".to_owned(),
                points: 4,
                critical: true,
            }],
        }])
    }
}
