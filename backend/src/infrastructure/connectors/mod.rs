mod cpra_connector;
mod la_county_connector;
mod lives_batch_connector;
mod long_beach_connector;
mod san_diego_connector;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::application::dto::SourceFacilityInput;

pub use cpra_connector::CpraConnector;
pub use la_county_connector::LaCountyConnector;
pub use lives_batch_connector::LivesBatchConnector;
pub use long_beach_connector::LongBeachConnector;
pub use san_diego_connector::SanDiegoConnector;

#[async_trait]
pub trait HealthDataConnector: Send + Sync {
    fn source_name(&self) -> &'static str;
    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>>;
}

pub fn default_connectors() -> Vec<Arc<dyn HealthDataConnector>> {
    vec![
        Arc::new(LaCountyConnector::from_env()),
        Arc::new(SanDiegoConnector::from_env()),
        Arc::new(LongBeachConnector::from_env()),
        Arc::new(LivesBatchConnector::from_env()),
        Arc::new(CpraConnector::from_env()),
    ]
}
