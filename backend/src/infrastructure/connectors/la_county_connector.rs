use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

pub struct LaCountyConnector;

#[async_trait]
impl HealthDataConnector for LaCountyConnector {
    fn source_name(&self) -> &'static str {
        "la_county_open_data"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        Ok(vec![SourceFacilityInput {
            source_id: "FAC-10021".to_owned(),
            name: "Sunset Noodle House".to_owned(),
            address: "1825 W Sunset Blvd".to_owned(),
            city: "Los Angeles".to_owned(),
            state: "CA".to_owned(),
            postal_code: "90026".to_owned(),
            latitude: 34.0789,
            longitude: -118.2636,
            jurisdiction: Jurisdiction::LosAngelesCounty,
            inspected_at: Utc::now() - Duration::days(3),
            raw_score: Some(94.0),
            letter_grade: Some("A".to_owned()),
            placard_status: None,
            violations: vec![Violation {
                code: "31A".to_owned(),
                description: "Food contact surfaces not clean".to_owned(),
                points: 2,
                critical: false,
            }],
        }])
    }
}
