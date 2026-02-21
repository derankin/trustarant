use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

use crate::{
    application::{
        dto::SourceFacilityInput,
        services::{ScoreSignals, TrustScoreService},
    },
    domain::{
        entities::{ConnectorIngestionStatus, Facility, Inspection, SystemIngestionStatus},
        repositories::FacilityRepository,
    },
    infrastructure::connectors::HealthDataConnector,
};

#[derive(Clone)]
pub struct IngestionService {
    repository: Arc<dyn FacilityRepository>,
    trust_score_service: Arc<TrustScoreService>,
    connectors: Vec<Arc<dyn HealthDataConnector>>,
    refresh_lock: Arc<Mutex<()>>,
}

const CONNECTOR_MAX_ATTEMPTS: usize = 3;

#[derive(Clone, Debug, serde::Serialize, Default)]
pub struct IngestionStats {
    pub last_refresh_at: Option<chrono::DateTime<Utc>>,
    pub unique_facilities: usize,
    pub connector_stats: Vec<ConnectorIngestionStatus>,
}

impl IngestionService {
    pub fn new(
        repository: Arc<dyn FacilityRepository>,
        trust_score_service: Arc<TrustScoreService>,
        connectors: Vec<Arc<dyn HealthDataConnector>>,
    ) -> Self {
        Self {
            repository,
            trust_score_service,
            connectors,
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn stats(&self) -> IngestionStats {
        match self.repository.get_system_ingestion_status().await {
            Ok(Some(status)) => IngestionStats {
                last_refresh_at: Some(status.last_refresh_at),
                unique_facilities: status.unique_facilities,
                connector_stats: status.connector_stats,
            },
            Ok(None) | Err(_) => IngestionStats::default(),
        }
    }

    pub async fn refresh(&self) -> anyhow::Result<()> {
        let _guard = self.refresh_lock.lock().await;
        let mut stitched: HashMap<String, SourceFacilityInput> = HashMap::new();
        let mut connector_stats = Vec::new();
        let mut successful_connectors = 0usize;
        let previous_status = self
            .repository
            .get_system_ingestion_status()
            .await
            .ok()
            .flatten();

        for connector in &self.connectors {
            match self.fetch_with_retry(connector.as_ref()).await {
                Ok(records) => {
                    successful_connectors += 1;
                    info!(
                        source = connector.source_name(),
                        records = records.len(),
                        "Fetched inspection records"
                    );
                    connector_stats.push(ConnectorIngestionStatus {
                        source: connector.source_name().to_owned(),
                        fetched_records: records.len(),
                        error: None,
                    });

                    for record in records {
                        let key = dedupe_key(&record);
                        stitched
                            .entry(key)
                            .and_modify(|current| {
                                if current.inspected_at < record.inspected_at {
                                    *current = record.clone();
                                }
                            })
                            .or_insert(record);
                    }
                }
                Err(error) => {
                    connector_stats.push(ConnectorIngestionStatus {
                        source: connector.source_name().to_owned(),
                        fetched_records: 0,
                        error: Some(error.to_string()),
                    });
                    warn!(
                        source = connector.source_name(),
                        error = %error,
                        "Connector fetch failed"
                    );
                }
            }
        }

        if successful_connectors == 0 {
            anyhow::bail!("no connectors succeeded; keeping previous dataset untouched");
        }

        if stitched.is_empty() {
            anyhow::bail!("ingestion produced zero facilities; keeping previous dataset untouched");
        }

        if let Some(previous) = previous_status {
            let minimum_safe_count = (previous.unique_facilities / 2).max(1);
            if stitched.len() < minimum_safe_count {
                anyhow::bail!(
                    "ingestion result too small ({} < {}), keeping previous dataset untouched",
                    stitched.len(),
                    minimum_safe_count
                );
            }
        }

        let facilities = stitched
            .into_values()
            .map(|record| self.normalize(record))
            .collect::<Vec<_>>();
        let unique_facilities = facilities.len();

        self.repository
            .replace_all(facilities)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        let snapshot = SystemIngestionStatus {
            last_refresh_at: Utc::now(),
            unique_facilities,
            connector_stats,
        };

        self.repository
            .set_system_ingestion_status(snapshot.clone())
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        info!(
            unique_facilities = snapshot.unique_facilities,
            "Ingestion + normalization finished"
        );

        Ok(())
    }

    async fn fetch_with_retry(
        &self,
        connector: &dyn HealthDataConnector,
    ) -> anyhow::Result<Vec<SourceFacilityInput>> {
        let mut attempt = 0usize;
        loop {
            attempt += 1;

            match connector.fetch_facilities().await {
                Ok(records) => return Ok(records),
                Err(error) if attempt < CONNECTOR_MAX_ATTEMPTS => {
                    let backoff_seconds = (attempt as u64) * 2;
                    warn!(
                        source = connector.source_name(),
                        attempt,
                        error = %error,
                        "Connector fetch failed, retrying"
                    );
                    sleep(Duration::from_secs(backoff_seconds)).await;
                }
                Err(error) => {
                    return Err(error);
                }
            }
        }
    }

    fn normalize(&self, record: SourceFacilityInput) -> Facility {
        let trust_score = self.trust_score_service.score(&ScoreSignals {
            raw_score: record.raw_score,
            letter_grade: record.letter_grade.clone(),
            placard_status: record.placard_status.clone(),
        });

        let inspection = Inspection {
            inspection_id: format!("{}-{}", record.jurisdiction.code(), record.source_id),
            inspected_at: record.inspected_at,
            raw_score: record.raw_score,
            letter_grade: record.letter_grade,
            placard_status: record.placard_status,
            violations: record.violations,
        };

        Facility {
            id: format!("{}::{}", record.jurisdiction.code(), record.source_id),
            source_id: record.source_id,
            name: record.name,
            address: record.address,
            city: record.city,
            state: record.state,
            postal_code: record.postal_code,
            latitude: record.latitude,
            longitude: record.longitude,
            jurisdiction: record.jurisdiction,
            trust_score,
            inspections: vec![inspection],
            updated_at: Utc::now(),
        }
    }
}

fn dedupe_key(record: &SourceFacilityInput) -> String {
    format!(
        "{}|{}|{}|{}",
        record.name.trim().to_ascii_lowercase(),
        record.address.trim().to_ascii_lowercase(),
        record.city.trim().to_ascii_lowercase(),
        record.postal_code.trim().to_ascii_lowercase()
    )
}
