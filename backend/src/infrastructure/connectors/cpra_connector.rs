use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

pub struct CpraConnector;

#[async_trait]
impl HealthDataConnector for CpraConnector {
    fn source_name(&self) -> &'static str {
        "cpra_import_orange_pasadena"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        Ok(vec![
            SourceFacilityInput {
                source_id: "OC-2209".to_owned(),
                name: "Laguna Coastal Grill".to_owned(),
                address: "145 Ocean Ave".to_owned(),
                city: "Laguna Beach".to_owned(),
                state: "CA".to_owned(),
                postal_code: "92651".to_owned(),
                latitude: 33.5423,
                longitude: -117.7834,
                jurisdiction: Jurisdiction::OrangeCounty,
                inspected_at: Utc::now() - Duration::days(4),
                raw_score: None,
                letter_grade: None,
                placard_status: Some("Green".to_owned()),
                violations: vec![],
            },
            SourceFacilityInput {
                source_id: "PAS-1142".to_owned(),
                name: "Arroyo Bistro".to_owned(),
                address: "301 N Lake Ave".to_owned(),
                city: "Pasadena".to_owned(),
                state: "CA".to_owned(),
                postal_code: "91101".to_owned(),
                latitude: 34.1501,
                longitude: -118.1321,
                jurisdiction: Jurisdiction::Pasadena,
                inspected_at: Utc::now() - Duration::days(8),
                raw_score: None,
                letter_grade: None,
                placard_status: Some("Yellow".to_owned()),
                violations: vec![Violation {
                    code: "P-14".to_owned(),
                    description: "Sanitizer concentration out of range".to_owned(),
                    points: 0,
                    critical: false,
                }],
            },
        ])
    }
}
