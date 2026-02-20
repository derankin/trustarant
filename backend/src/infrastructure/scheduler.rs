use std::{sync::Arc, time::Duration};

use tokio::time;
use tracing::{error, info};

use crate::application::services::IngestionService;

pub async fn run(ingestion_service: Arc<IngestionService>, interval_hours: u64) {
    let mut interval = time::interval(Duration::from_secs(interval_hours.max(1) * 60 * 60));
    // Consume the immediate first tick so the periodic scheduler waits for the
    // configured interval after startup.
    interval.tick().await;

    loop {
        interval.tick().await;

        if let Err(error) = ingestion_service.refresh().await {
            error!(%error, "Scheduled ingestion failed");
            continue;
        }

        info!("Scheduled ingestion completed");
    }
}
