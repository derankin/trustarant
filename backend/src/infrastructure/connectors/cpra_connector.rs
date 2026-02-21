use std::{
    collections::hash_map::DefaultHasher,
    env,
    hash::{Hash, Hasher},
    time::Duration,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use reqwest::Client;
use serde_json::{Map, Value};

use crate::{
    application::dto::SourceFacilityInput,
    domain::entities::{Jurisdiction, Violation},
    infrastructure::connectors::HealthDataConnector,
};

const DEFAULT_TIMEOUT_SECS: u64 = 20;

#[derive(Clone)]
pub struct CpraConnector {
    client: Client,
    orange_county_url: Option<String>,
    pasadena_url: Option<String>,
}

impl Default for CpraConnector {
    fn default() -> Self {
        Self::from_env()
    }
}

impl CpraConnector {
    pub fn from_env() -> Self {
        let orange_county_url = env::var("TRUSTARANT_OC_CPRA_EXPORT_URL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let pasadena_url = env::var("TRUSTARANT_PASADENA_CPRA_EXPORT_URL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let timeout_secs = env::var("TRUSTARANT_CPRA_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            orange_county_url,
            pasadena_url,
        }
    }

    async fn fetch_export(
        &self,
        source_url: &str,
        jurisdiction: Jurisdiction,
        id_prefix: &str,
    ) -> Result<Vec<SourceFacilityInput>> {
        let response = self
            .client
            .get(source_url)
            .send()
            .await
            .with_context(|| format!("{} CPRA export request failed", jurisdiction.label()))?
            .error_for_status()
            .with_context(|| {
                format!(
                    "{} CPRA export returned non-success status",
                    jurisdiction.label()
                )
            })?;

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let body = response
            .text()
            .await
            .with_context(|| format!("{} CPRA export body read failed", jurisdiction.label()))?;

        if body.trim().is_empty() {
            return Ok(Vec::new());
        }

        let records = if content_type.contains("json") || body.trim_start().starts_with(['{', '['])
        {
            parse_json_records(&body)?
        } else {
            parse_csv_records(&body)?
        };

        Ok(records
            .into_iter()
            .enumerate()
            .filter_map(|(idx, record)| map_record(record, jurisdiction.clone(), id_prefix, idx))
            .collect::<Vec<_>>())
    }
}

#[async_trait]
impl HealthDataConnector for CpraConnector {
    fn source_name(&self) -> &'static str {
        "cpra_import_orange_pasadena"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        if self.orange_county_url.is_none() && self.pasadena_url.is_none() {
            anyhow::bail!(
                "CPRA connector not configured: set TRUSTARANT_OC_CPRA_EXPORT_URL and/or TRUSTARANT_PASADENA_CPRA_EXPORT_URL"
            );
        }

        let mut facilities = Vec::new();

        if let Some(url) = &self.orange_county_url {
            facilities.extend(
                self.fetch_export(url, Jurisdiction::OrangeCounty, "oc")
                    .await?,
            );
        }

        if let Some(url) = &self.pasadena_url {
            facilities.extend(
                self.fetch_export(url, Jurisdiction::Pasadena, "pas")
                    .await?,
            );
        }

        if facilities.is_empty() {
            anyhow::bail!("CPRA connector fetched zero records from configured export URLs");
        }

        Ok(facilities)
    }
}

fn parse_json_records(body: &str) -> Result<Vec<Map<String, Value>>> {
    let value: Value = serde_json::from_str(body).context("JSON export parse failed")?;

    let map_list = if let Some(array) = value.as_array() {
        array
            .iter()
            .filter_map(|item| item.as_object().cloned())
            .collect::<Vec<_>>()
    } else if let Some(object) = value.as_object() {
        if let Some(features) = object.get("features").and_then(Value::as_array) {
            features
                .iter()
                .filter_map(|item| item.get("attributes"))
                .filter_map(Value::as_object)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            ["data", "results", "records", "value"]
                .iter()
                .find_map(|key| object.get(*key))
                .and_then(Value::as_array)
                .map(|array| {
                    array
                        .iter()
                        .filter_map(Value::as_object)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    Ok(map_list)
}

fn parse_csv_records(body: &str) -> Result<Vec<Map<String, Value>>> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(body.as_bytes());

    let headers = reader
        .headers()
        .context("CSV headers parse failed")?
        .iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    let mut records = Vec::new();
    for row in reader.records() {
        let row = row.context("CSV row parse failed")?;
        let mut map = Map::new();

        for (idx, value) in row.iter().enumerate() {
            if let Some(header) = headers.get(idx) {
                map.insert(header.clone(), Value::String(value.trim().to_owned()));
            }
        }

        records.push(map);
    }

    Ok(records)
}

fn map_record(
    record: Map<String, Value>,
    jurisdiction: Jurisdiction,
    id_prefix: &str,
    row_index: usize,
) -> Option<SourceFacilityInput> {
    let name = rec_string(
        &record,
        &[
            "facility_name",
            "Facility_Name",
            "name",
            "record_name",
            "business_name",
        ],
    )?;
    let address = rec_string(
        &record,
        &[
            "address",
            "Address",
            "facility_address",
            "FACILITY_ADDRESS",
            "street_address",
        ],
    )
    .unwrap_or_default();
    let city = rec_string(&record, &["city", "City", "facility_city", "FACILITY_CITY"])
        .unwrap_or_else(|| jurisdiction.label().to_owned());
    let postal_code = rec_string(
        &record,
        &["postal_code", "Zip", "zip", "FACILITY_ZIP", "zipcode"],
    )
    .unwrap_or_default();
    let state = rec_string(&record, &["state", "State", "FACILITY_STATE"])
        .unwrap_or_else(|| "CA".to_owned());

    let source_id = rec_string(
        &record,
        &[
            "source_id",
            "facility_id",
            "Facility_ID",
            "record_id",
            "SERIAL_NUMBER",
            "id",
        ],
    )
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| stable_id(id_prefix, &name, &address, &city, row_index));

    let raw_score = rec_f64(&record, &["raw_score", "score", "Score", "SCORE"]).map(|v| v as f32);
    let letter_grade = rec_string(&record, &["letter_grade", "grade", "Grade", "GRADE"]);
    let placard_status = rec_string(
        &record,
        &[
            "placard_status",
            "status",
            "Placard_Status",
            "inspection_status",
        ],
    );

    let default_coordinates = match jurisdiction {
        Jurisdiction::OrangeCounty => (33.7175, -117.8311),
        Jurisdiction::Pasadena => (34.1478, -118.1445),
        _ => (34.0522, -118.2437),
    };
    let latitude = rec_f64(&record, &["latitude", "Latitude", "FACILITY_LATITUDE"])
        .unwrap_or(default_coordinates.0);
    let longitude = rec_f64(&record, &["longitude", "Longitude", "FACILITY_LONGITUDE"])
        .unwrap_or(default_coordinates.1);

    let inspected_at = rec_datetime(
        &record,
        &[
            "inspected_at",
            "inspection_date",
            "Inspection_Date",
            "ACTIVITY_DATE",
            "date",
            "Date",
        ],
    )
    .unwrap_or_else(Utc::now);

    let violations = rec_string(
        &record,
        &[
            "violation_description",
            "VIOLATION_DESCRIPTION",
            "closure_reason",
            "reason",
        ],
    )
    .map(|description| {
        vec![Violation {
            code: rec_string(
                &record,
                &["violation_code", "VIOLATION_CODE", "code", "closure_code"],
            )
            .unwrap_or_else(|| "CPRA".to_owned()),
            points: rec_f64(&record, &["violation_points", "POINTS"])
                .map(|value| value as i16)
                .unwrap_or(0),
            critical: rec_bool(&record, &["critical", "is_critical"]).unwrap_or(false),
            description,
        }]
    })
    .unwrap_or_default();

    Some(SourceFacilityInput {
        source_id,
        name,
        address,
        city,
        state,
        postal_code,
        latitude,
        longitude,
        jurisdiction,
        inspected_at,
        raw_score,
        letter_grade,
        placard_status,
        violations,
    })
}

fn stable_id(prefix: &str, name: &str, address: &str, city: &str, row_index: usize) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{name}|{address}|{city}|{row_index}").hash(&mut hasher);
    format!("{prefix}-{:016x}", hasher.finish())
}

fn rec_string(record: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| record.get(*key))
        .and_then(|value| {
            if let Some(text) = value.as_str() {
                return Some(text.trim().to_owned());
            }
            if value.is_number() {
                return Some(value.to_string());
            }
            None
        })
        .filter(|text| !text.is_empty())
}

fn rec_f64(record: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        let value = record.get(*key)?;
        if let Some(number) = value.as_f64() {
            return Some(number);
        }
        value
            .as_str()
            .and_then(|text| text.trim().parse::<f64>().ok())
    })
}

fn rec_bool(record: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        let value = record.get(*key)?;
        if let Some(boolean) = value.as_bool() {
            return Some(boolean);
        }
        value
            .as_str()
            .and_then(|text| match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            })
    })
}

fn rec_datetime(record: &Map<String, Value>, keys: &[&str]) -> Option<DateTime<Utc>> {
    let value = keys.iter().find_map(|key| record.get(*key))?;

    if let Some(number) = value.as_i64() {
        return timestamp_to_datetime(number);
    }

    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }

    if let Ok(number) = text.parse::<i64>() {
        return timestamp_to_datetime(number);
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Some(parsed.with_timezone(&Utc));
    }

    if let Ok(parsed) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return parsed
            .and_hms_opt(0, 0, 0)
            .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc));
    }

    if let Ok(parsed) = NaiveDate::parse_from_str(text, "%m/%d/%Y") {
        return parsed
            .and_hms_opt(0, 0, 0)
            .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(parsed, Utc));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(text, "%m/%d/%Y %H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(parsed, Utc));
    }

    None
}

fn timestamp_to_datetime(raw: i64) -> Option<DateTime<Utc>> {
    if raw > 10_000_000_000 {
        DateTime::from_timestamp_millis(raw)
    } else {
        DateTime::from_timestamp(raw, 0)
    }
}
