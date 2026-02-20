use std::{collections::HashMap, env, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};
use tracing::warn;

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

const DEFAULT_INVENTORY_URL: &str = "https://services.arcgis.com/RmCCgQtiZLDCtblq/arcgis/rest/services/Environmental_Health_Restaurant_and_Market_Inventory_12312025/FeatureServer";
const DEFAULT_INSPECTIONS_URL: &str = "https://services.arcgis.com/RmCCgQtiZLDCtblq/arcgis/rest/services/Environmental_Health_Restaurant_and_Market_Inspections_01012023_to_123120025/FeatureServer";
const DEFAULT_VIOLATIONS_URL: &str = "https://services.arcgis.com/RmCCgQtiZLDCtblq/arcgis/rest/services/Environmental_Health_Restaurant_and_Market_Violations_01012023_to_123120025/FeatureServer";
const DEFAULT_PAGE_SIZE: usize = 2_000;
const QUERY_CHUNK_SIZE: usize = 50;

#[derive(Clone)]
pub struct LaCountyConnector {
    client: Client,
    inventory_url: String,
    inspections_url: String,
    violations_url: String,
    page_size: usize,
    max_records: Option<usize>,
}

impl Default for LaCountyConnector {
    fn default() -> Self {
        Self::from_env()
    }
}

impl LaCountyConnector {
    pub fn from_env() -> Self {
        let inventory_url = env::var("TRUSTARANT_LA_INVENTORY_URL")
            .unwrap_or_else(|_| DEFAULT_INVENTORY_URL.to_owned());
        let inspections_url = env::var("TRUSTARANT_LA_INSPECTIONS_URL")
            .unwrap_or_else(|_| DEFAULT_INSPECTIONS_URL.to_owned());
        let violations_url = env::var("TRUSTARANT_LA_VIOLATIONS_URL")
            .unwrap_or_else(|_| DEFAULT_VIOLATIONS_URL.to_owned());
        let page_size = env::var("TRUSTARANT_LA_PAGE_SIZE")
            .ok()
            .or_else(|| env::var("TRUSTARANT_LA_LIMIT").ok())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .max(1);
        let max_records = env::var("TRUSTARANT_LA_MAX_RECORDS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0);
        let timeout_secs = env::var("TRUSTARANT_LA_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(20);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            inventory_url,
            inspections_url,
            violations_url,
            page_size,
            max_records,
        }
    }

    async fn query_features<T>(
        &self,
        source_url: &str,
        where_clause: &str,
        out_fields: &str,
        order_by_fields: Option<&str>,
    ) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let mut offset = 0usize;
        let mut rows = Vec::<T>::new();

        loop {
            let mut query = vec![
                ("where".to_owned(), where_clause.to_owned()),
                ("outFields".to_owned(), out_fields.to_owned()),
                ("resultOffset".to_owned(), offset.to_string()),
                (
                    "resultRecordCount".to_owned(),
                    self.page_size.to_string(),
                ),
                ("f".to_owned(), "json".to_owned()),
            ];
            if let Some(order_by) = order_by_fields {
                query.push(("orderByFields".to_owned(), order_by.to_owned()));
            }

            let response: ArcGisResponse<T> = self
                .client
                .get(format!("{}/0/query", source_url.trim_end_matches('/')))
                .query(&query)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let page_count = response.features.len();
            rows.extend(response.features.into_iter().map(|f| f.attributes));

            if let Some(max_records) = self.max_records {
                if rows.len() >= max_records {
                    rows.truncate(max_records);
                    break;
                }
            }

            if page_count == 0 {
                break;
            }

            offset = offset.saturating_add(page_count);
            let exceeded = response.exceeded_transfer_limit.unwrap_or(false);
            if !exceeded && page_count < self.page_size {
                break;
            }
        }

        Ok(rows)
    }

    async fn fetch_inspections(&self) -> Result<Vec<LaInspectionAttrs>> {
        self.query_features(
            &self.inspections_url,
            "FACILITY_ID IS NOT NULL",
            "ACTIVITY_DATE,FACILITY_ID,FACILITY_NAME,FACILITY_ADDRESS,FACILITY_CITY,FACILITY_STATE,FACILITY_ZIP,SCORE,GRADE,SERIAL_NUMBER",
            Some("ACTIVITY_DATE DESC"),
        )
        .await
        .context("LA inspections request failed")
    }

    async fn fetch_inventory(
        &self,
        facility_ids: &[String],
    ) -> Result<HashMap<String, LaInventoryAttrs>> {
        if facility_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut inventory_map = HashMap::new();
        for chunk in facility_ids.chunks(QUERY_CHUNK_SIZE) {
            let where_clause = format!(
                "FACILITY_ID IN ({})",
                chunk
                    .iter()
                    .map(|id| format!("'{}'", id.trim().replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(",")
            );

            let rows: Vec<LaInventoryAttrs> = self
                .query_features(
                    &self.inventory_url,
                    where_clause.as_str(),
                    "FACILITY_ID,FACILITY_NAME,FACILITY_ADDRESS,FACILITY_CITY,FACILITY__STATE,FACILITY_ZIP,FACILITY_LATITUDE,FACILITY_LONGITUDE",
                    None,
                )
                .await
                .context("LA inventory request failed")?;

            for attrs in rows {
                if let Some(id) = attrs.facility_id.clone() {
                    inventory_map.insert(id.trim().to_owned(), attrs);
                }
            }
        }

        Ok(inventory_map)
    }

    async fn fetch_violations(
        &self,
        serial_numbers: &[String],
    ) -> Result<HashMap<String, Vec<Violation>>> {
        if serial_numbers.is_empty() {
            return Ok(HashMap::new());
        }

        let mut grouped: HashMap<String, Vec<Violation>> = HashMap::new();
        for chunk in serial_numbers.chunks(QUERY_CHUNK_SIZE) {
            let where_clause = format!(
                "SERIAL_NUMBER IN ({})",
                chunk
                    .iter()
                    .map(|sn| format!("'{}'", sn.trim().replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(",")
            );

            let rows: Vec<LaViolationAttrs> = self
                .query_features(
                    &self.violations_url,
                    where_clause.as_str(),
                    "SERIAL_NUMBER,VIOLATION_CODE,VIOLATION_DESCRIPTION,POINTS",
                    None,
                )
                .await
                .context("LA violations request failed")?;

            for attrs in rows {
                let Some(serial_number) = attrs.serial_number else {
                    continue;
                };

                let points = attrs.points.unwrap_or(0) as i16;
                grouped
                    .entry(serial_number.trim().to_owned())
                    .or_default()
                    .push(Violation {
                        code: attrs
                            .violation_code
                            .unwrap_or_else(|| "LA-UNKNOWN".to_owned()),
                        description: attrs.violation_description.unwrap_or_default(),
                        points,
                        critical: points >= 4,
                    });
            }
        }

        Ok(grouped)
    }
}

#[async_trait]
impl HealthDataConnector for LaCountyConnector {
    fn source_name(&self) -> &'static str {
        "la_county_open_data"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        let inspections = self.fetch_inspections().await?;

        let serial_numbers = inspections
            .iter()
            .filter_map(|inspection| inspection.serial_number.clone())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let facility_ids = inspections
            .iter()
            .filter_map(|inspection| inspection.facility_id.clone())
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        let inventory = match self.fetch_inventory(&facility_ids).await {
            Ok(records) => records,
            Err(error) => {
                warn!(error = %error, "LA inventory enrichment failed; proceeding without inventory join");
                HashMap::new()
            }
        };
        let violations = match self.fetch_violations(&serial_numbers).await {
            Ok(records) => records,
            Err(error) => {
                warn!(error = %error, "LA violations enrichment failed; proceeding without violation join");
                HashMap::new()
            }
        };

        let facilities = inspections
            .into_iter()
            .filter_map(|inspection| {
                let facility_id = inspection.facility_id?.trim().to_owned();
                let serial_number = inspection.serial_number.clone().unwrap_or_default();
                let inv = inventory.get(&facility_id);

                let (latitude, longitude) = inv
                    .map(|record| {
                        (
                            record.facility_latitude.unwrap_or(34.0522),
                            record.facility_longitude.unwrap_or(-118.2437),
                        )
                    })
                    .unwrap_or((34.0522, -118.2437));

                let inspected_at = inspection
                    .activity_date
                    .and_then(DateTime::from_timestamp_millis)
                    .unwrap_or_else(Utc::now);

                Some(SourceFacilityInput {
                    source_id: facility_id,
                    name: inspection
                        .facility_name
                        .or_else(|| inv.and_then(|record| record.facility_name.clone()))
                        .unwrap_or_else(|| "Unknown Facility".to_owned()),
                    address: inspection
                        .facility_address
                        .or_else(|| inv.and_then(|record| record.facility_address.clone()))
                        .unwrap_or_default(),
                    city: inspection
                        .facility_city
                        .or_else(|| inv.and_then(|record| record.facility_city.clone()))
                        .unwrap_or_else(|| "Los Angeles".to_owned()),
                    state: inspection
                        .facility_state
                        .or_else(|| inv.and_then(|record| record.facility_state.clone()))
                        .unwrap_or_else(|| "CA".to_owned()),
                    postal_code: inspection
                        .facility_zip
                        .or_else(|| inv.and_then(|record| record.facility_zip.clone()))
                        .unwrap_or_default(),
                    latitude,
                    longitude,
                    jurisdiction: Jurisdiction::LosAngelesCounty,
                    inspected_at,
                    raw_score: inspection.score.map(|score| score as f32),
                    letter_grade: inspection.grade,
                    placard_status: None,
                    violations: violations
                        .get(serial_number.trim())
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();

        Ok(facilities)
    }
}

#[derive(Debug, Deserialize)]
struct ArcGisResponse<T> {
    #[serde(rename = "exceededTransferLimit")]
    exceeded_transfer_limit: Option<bool>,
    features: Vec<ArcGisFeature<T>>,
}

#[derive(Debug, Deserialize)]
struct ArcGisFeature<T> {
    attributes: T,
}

#[derive(Debug, Deserialize)]
struct LaInspectionAttrs {
    #[serde(rename = "ACTIVITY_DATE")]
    activity_date: Option<i64>,
    #[serde(rename = "FACILITY_ID")]
    facility_id: Option<String>,
    #[serde(rename = "FACILITY_NAME")]
    facility_name: Option<String>,
    #[serde(rename = "FACILITY_ADDRESS")]
    facility_address: Option<String>,
    #[serde(rename = "FACILITY_CITY")]
    facility_city: Option<String>,
    #[serde(rename = "FACILITY_STATE")]
    facility_state: Option<String>,
    #[serde(rename = "FACILITY_ZIP")]
    facility_zip: Option<String>,
    #[serde(rename = "SCORE")]
    score: Option<f64>,
    #[serde(rename = "GRADE")]
    grade: Option<String>,
    #[serde(rename = "SERIAL_NUMBER")]
    serial_number: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LaInventoryAttrs {
    #[serde(rename = "FACILITY_ID")]
    facility_id: Option<String>,
    #[serde(rename = "FACILITY_NAME")]
    facility_name: Option<String>,
    #[serde(rename = "FACILITY_ADDRESS")]
    facility_address: Option<String>,
    #[serde(rename = "FACILITY_CITY")]
    facility_city: Option<String>,
    #[serde(rename = "FACILITY__STATE")]
    facility_state: Option<String>,
    #[serde(rename = "FACILITY_ZIP")]
    facility_zip: Option<String>,
    #[serde(rename = "FACILITY_LATITUDE")]
    facility_latitude: Option<f64>,
    #[serde(rename = "FACILITY_LONGITUDE")]
    facility_longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct LaViolationAttrs {
    #[serde(rename = "SERIAL_NUMBER")]
    serial_number: Option<String>,
    #[serde(rename = "VIOLATION_CODE")]
    violation_code: Option<String>,
    #[serde(rename = "VIOLATION_DESCRIPTION")]
    violation_description: Option<String>,
    #[serde(rename = "POINTS")]
    points: Option<i64>,
}
