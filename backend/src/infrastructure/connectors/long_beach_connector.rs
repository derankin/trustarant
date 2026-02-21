use std::{env, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
use reqwest::Client;
use scraper::{Html, Selector};

use crate::{
    application::dto::SourceFacilityInput, domain::entities::Jurisdiction,
    infrastructure::connectors::HealthDataConnector,
};

const DEFAULT_CLOSURES_URL: &str =
    "https://www.longbeach.gov/health/inspections-and-reporting/inspections/restaurant-closures/";
const DEFAULT_LIMIT: usize = 200;

#[derive(Clone)]
pub struct LongBeachConnector {
    client: Client,
    closures_url: String,
    limit: usize,
}

impl Default for LongBeachConnector {
    fn default() -> Self {
        Self::from_env()
    }
}

impl LongBeachConnector {
    pub fn from_env() -> Self {
        let closures_url = env::var("TRUSTARANT_LONG_BEACH_CLOSURES_URL")
            .unwrap_or_else(|_| DEFAULT_CLOSURES_URL.to_owned());
        let limit = env::var("TRUSTARANT_LONG_BEACH_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_LIMIT);
        let timeout_secs = env::var("TRUSTARANT_LONG_BEACH_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(20);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .http1_only()
            .danger_accept_invalid_certs(true)
            .user_agent("TrustarauntBot/1.0 (+https://trustaraunt.com)")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            closures_url,
            limit,
        }
    }
}

#[async_trait]
impl HealthDataConnector for LongBeachConnector {
    fn source_name(&self) -> &'static str {
        "long_beach_closures_page"
    }

    async fn fetch_facilities(&self) -> Result<Vec<SourceFacilityInput>> {
        let html = self
            .client
            .get(&self.closures_url)
            .header(
                reqwest::header::ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .send()
            .await
            .context("Long Beach closures page request failed")?
            .error_for_status()
            .context("Long Beach closures page returned non-success status")?
            .text()
            .await
            .context("Long Beach closures page body read failed")?;

        let document = Html::parse_document(&html);
        let row_selector = Selector::parse("table tr").expect("valid row selector");
        let cell_selector = Selector::parse("td").expect("valid cell selector");

        let mut facilities = Vec::new();
        for row in document.select(&row_selector) {
            let cells = row
                .select(&cell_selector)
                .map(|cell| {
                    cell.text()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();

            if cells.len() != 4 {
                continue;
            }

            let restaurant_lines = &cells[0];
            if restaurant_lines.is_empty() {
                continue;
            }

            let name = restaurant_lines
                .first()
                .map(|value| value.trim().to_owned())
                .unwrap_or_default();

            if name.eq_ignore_ascii_case("restaurant") {
                continue;
            }

            let address = restaurant_lines
                .iter()
                .skip(1)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");

            let date_closed = cells[1].join(" ");
            let date_reopened = cells[2].join(" ");
            let reason = cells[3].join(" ");

            let inspected_at = parse_long_beach_date(&date_closed).unwrap_or_else(Utc::now);
            let is_currently_closed = date_reopened.trim().is_empty();

            let (raw_score, letter_grade, placard_status) = if is_currently_closed {
                (Some(40.0), Some("C".to_owned()), Some("Red".to_owned()))
            } else {
                (Some(74.0), Some("C".to_owned()), Some("Yellow".to_owned()))
            };

            facilities.push(SourceFacilityInput {
                source_id: format!(
                    "lb-closure-{}-{}",
                    inspected_at.date_naive(),
                    slugify(&name)
                ),
                name,
                address,
                city: "Long Beach".to_owned(),
                state: "CA".to_owned(),
                postal_code: String::new(),
                latitude: 33.7701,
                longitude: -118.1937,
                jurisdiction: Jurisdiction::LongBeach,
                inspected_at,
                raw_score,
                letter_grade,
                placard_status,
                violations: vec![crate::domain::entities::Violation {
                    code: "LB-CLOSURE".to_owned(),
                    description: reason,
                    points: 0,
                    critical: true,
                }],
            });

            if facilities.len() >= self.limit {
                break;
            }
        }

        Ok(facilities)
    }
}

fn parse_long_beach_date(value: &str) -> Option<chrono::DateTime<Utc>> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return None;
    }

    let date = NaiveDate::parse_from_str(cleaned, "%m/%d/%Y")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(cleaned, "%m-%d-%Y").ok())?;
    let naive = date.and_hms_opt(0, 0, 0)?;
    Some(Utc.from_utc_datetime(&naive))
}

fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }

    out.trim_matches('-').to_owned()
}
