use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

pub struct LivesBatchConnector;

#[async_trait]
impl HealthDataConnector for LivesBatchConnector {
    fn source_name(&self) -> &'static str {
        "lives_batch_riv_sbc"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        Ok(vec![
            SourceFacilityInput {
                source_id: "RIV-3231".to_owned(),
                name: "Date Palm Cafe".to_owned(),
                address: "412 E Palm Canyon Dr".to_owned(),
                city: "Palm Springs".to_owned(),
                state: "CA".to_owned(),
                postal_code: "92262".to_owned(),
                latitude: 33.8301,
                longitude: -116.5453,
                jurisdiction: Jurisdiction::RiversideCounty,
                inspected_at: Utc::now() - Duration::days(6),
                raw_score: Some(92.0),
                letter_grade: Some("A".to_owned()),
                placard_status: None,
                violations: vec![Violation {
                    code: "11".to_owned(),
                    description: "Insufficient hand-washing signage".to_owned(),
                    points: 1,
                    critical: false,
                }],
            },
            SourceFacilityInput {
                source_id: "SBC-772".to_owned(),
                name: "Foothill Bento".to_owned(),
                address: "655 N Euclid Ave".to_owned(),
                city: "Ontario".to_owned(),
                state: "CA".to_owned(),
                postal_code: "91762".to_owned(),
                latitude: 34.0728,
                longitude: -117.6491,
                jurisdiction: Jurisdiction::SanBernardinoCounty,
                inspected_at: Utc::now() - Duration::days(1),
                raw_score: Some(86.0),
                letter_grade: Some("B".to_owned()),
                placard_status: None,
                violations: vec![Violation {
                    code: "25".to_owned(),
                    description: "Food storage separation issue".to_owned(),
                    points: 2,
                    critical: false,
                }],
            },
        ])
    }
}
