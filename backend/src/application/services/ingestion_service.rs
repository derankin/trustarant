use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::{
    application::{
        dto::SourceFacilityInput,
        services::{ScoreSignals, TrustScoreService},
    },
    domain::{
        entities::{Facility, Inspection},
        repositories::FacilityRepository,
    },
    infrastructure::connectors::HealthDataConnector,
};

#[derive(Clone)]
pub struct IngestionService {
    repository: Arc<dyn FacilityRepository>,
    trust_score_service: Arc<TrustScoreService>,
    connectors: Vec<Arc<dyn HealthDataConnector>>,
    stats: Arc<RwLock<IngestionStats>>,
}

#[derive(Clone, Debug, serde::Serialize, Default)]
pub struct ConnectorIngestionStats {
    pub source: String,
    pub fetched_records: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, Default)]
pub struct IngestionStats {
    pub last_refresh_at: Option<chrono::DateTime<Utc>>,
    pub unique_facilities: usize,
    pub connector_stats: Vec<ConnectorIngestionStats>,
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
            stats: Arc::new(RwLock::new(IngestionStats::default())),
        }
    }

    pub async fn stats(&self) -> IngestionStats {
        self.stats.read().await.clone()
    }

    pub async fn refresh(&self) -> anyhow::Result<()> {
        let mut stitched: HashMap<String, SourceFacilityInput> = HashMap::new();
        let mut connector_stats = Vec::new();

        for connector in &self.connectors {
            match connector.fetch_facilities().await {
                Ok(records) => {
                    info!(
                        source = connector.source_name(),
                        records = records.len(),
                        "Fetched inspection records"
                    );
                    connector_stats.push(ConnectorIngestionStats {
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
                    connector_stats.push(ConnectorIngestionStats {
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

        let facilities = stitched
            .into_values()
            .map(|record| self.normalize(record))
            .collect::<Vec<_>>();

        self.repository
            .replace_all(facilities)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        let snapshot = IngestionStats {
            last_refresh_at: Some(Utc::now()),
            unique_facilities: self.repository.list().await?.len(),
            connector_stats,
        };

        *self.stats.write().await = snapshot.clone();

        info!(
            unique_facilities = snapshot.unique_facilities,
            "Ingestion + normalization finished"
        );

        Ok(())
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
