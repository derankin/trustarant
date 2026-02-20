use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::Jurisdiction,
    infrastructure::connectors::HealthDataConnector,
};

pub struct LongBeachConnector;

#[async_trait]
impl HealthDataConnector for LongBeachConnector {
    fn source_name(&self) -> &'static str {
        "long_beach_rest_api"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        Ok(vec![SourceFacilityInput {
            source_id: "LB-901".to_owned(),
            name: "Pier Market Tacos".to_owned(),
            address: "75 Aquarium Way".to_owned(),
            city: "Long Beach".to_owned(),
            state: "CA".to_owned(),
            postal_code: "90802".to_owned(),
            latitude: 33.7626,
            longitude: -118.1967,
            jurisdiction: Jurisdiction::LongBeach,
            inspected_at: Utc::now() - Duration::days(5),
            raw_score: Some(97.0),
            letter_grade: Some("A".to_owned()),
            placard_status: None,
            violations: vec![],
        }])
    }
}
